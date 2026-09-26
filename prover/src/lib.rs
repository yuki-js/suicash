//! FeliCa DES oracle prover, IDi-level statement.
//!
//! Groth16 over BN254 (`ark-groth16`). The circuit proves, in R1CS:
//!
//! * **D1–D3** — the FeliCa DES key schedule `l = gsk ⊕ idm`,
//!   `alpha = DES_l(usk)`, `beta = DES_alpha(l)`, computed *inside* the
//!   circuit from the master keys. This is the security-relevant part: a
//!   prover that does not know `gsk`/`usk`/`idm` cannot produce a witness.
//! * **C1–C6** — transcript consistency: `3DES(l,beta,r1) == c1b`,
//!   `3DES(l,beta,r2) == c2a`, `auth2` CBC-decrypts under `r2` to a plaintext
//!   carrying a valid 8-byte MAC under opcode `0x13`, TID `== tail6(r1)`, and
//!   IDi `== idi`.
//!
//! Public inputs are just `(idi, r1, attested_at)` — see [`abi`], the single
//! canonical bytes ↔ field mapping shared by the prover, the circuit, this
//! crate's `Attestation` type and external verifiers.
//!
//! # What this does and does not establish
//!
//! The statement is **transcript consistency plus key-schedule binding**, not
//! card presence. `l` is derivable from `gsk ⊕ idm` and `idm` is an ordinary
//! input, so an oracle holding the master keys can still attest to *any*
//! `idi`. What the circuit guarantees is that whoever produced the proof knew
//! the system master keys, which is a non-repudiation property: a holder
//! cannot fabricate an attestation attributed to the key holder, and cannot
//! swap `idi` after the fact. `tests/forgery_authority.rs` pins both halves —
//! that a public proving key is no longer sufficient, and that the residual
//! master-key trust still stands.
//!
//! Setup is external: the proving key is generated once (ceremony output)
//! and loaded via [`load_proving_key`]; this crate never generates keys.
//! The `vk` is embedded into verifier contracts out of band.

use ark_bn254::{Bn254, Fr};
use ark_serialize::CanonicalSerialize;
use ark_snark::SNARK;
use thiserror::Error;

pub mod abi;
pub mod circuit;
pub mod des;
pub mod setup;
pub use abi::{PublicInputs, PUBLIC_INPUT_COUNT, PUBLIC_INPUT_ORDER, SUPPORTED_ALG};
pub use setup::{
    encode_proving_key, encode_verifying_key, load_proving_key, load_verifying_key,
    FelicaProvingKey, FelicaVerifyingKey, KeyLoadError,
};

use circuit::FelicaCircuit;

/// Prover failure modes (spec §8.4 mapping: `MAC_MISMATCH`, `TID_MISMATCH`,
/// `C1B_MISMATCH`, `PROVE_FAILED`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ProverError {
    #[error("AUTH2 MAC verification failed")]
    MacMismatch,
    #[error("transaction identifier mismatch")]
    TidMismatch,
    #[error("challenge response mismatch")]
    C1bMismatch,
    #[error("proof generation failed")]
    ProveFailed,
}

/// Everything `prove` needs, as raw bytes.
///
/// The master keys (`gsk` = GSK, `usk` = USK) and the card's `idm` are the
/// only private inputs, and they are private *witness* values: the caller
/// passes them and the circuit constrains the key schedule from them. `r1` is
/// intentionally absent — the prover recovers it as `3DES⁻¹(L,β,c1b)`,
/// mirroring the oracle's native session verifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProveRequest {
    /// Card IDm (8B, private witness).
    pub idm: [u8; 8],
    /// Card auth response (8B, private witness).
    pub c1b: [u8; 8],
    /// Card challenge (8B, private witness).
    pub c2a: [u8; 8],
    /// AUTH2 ciphertext (32B, private witness).
    pub auth2: [u8; 32],
    /// Resolved GSK from environment (8B, private witness).
    pub gsk: [u8; 8],
    /// Resolved USK from environment (8B, private witness).
    pub usk: [u8; 8],
    /// Oracle Unix timestamp, seconds (echoed public input).
    pub attested_at: u64,
}

/// Groth16 proof over BN254, hex-encoded like the RPC surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Groth16Proof {
    pub alg: String,
    pub a: (String, String),
    pub b: ((String, String), (String, String)),
    pub c: (String, String),
    pub public_inputs: Vec<String>,
}

/// Certified session: the circuit's public claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attestation {
    /// Extracted card identifier (8B) — the sole identity statement.
    pub idi: [u8; 8],
    /// Echoed timestamp.
    pub attested_at: u64,
    /// Groth16 proof binding the above.
    pub proof: Groth16Proof,
}

fn fr_to_hex(f: &Fr) -> String {
    let mut buf = Vec::new();
    f.serialize_compressed(&mut buf).expect("Fr serializes");
    hex::encode(buf)
}

fn fq_to_hex<F: CanonicalSerialize>(f: &F) -> String {
    let mut buf = Vec::new();
    f.serialize_compressed(&mut buf).expect("Fq serializes");
    hex::encode(buf)
}

/// Constraint/variable counts of the blank circuit (perf-regression guard).
pub fn blank_constraint_counts() -> (usize, usize, usize) {
    use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystem};
    let cs = ConstraintSystem::<Fr>::new_ref();
    FelicaCircuit::blank()
        .generate_constraints(cs.clone())
        .expect("blank synthesizes");
    (
        cs.num_constraints(),
        cs.num_instance_variables(),
        cs.num_witness_variables(),
    )
}

/// Check raw circuit satisfiability for the given witnesses (no proof).
///
/// Unlike [`prove`], this bypasses the native pre-checks and feeds the
/// witnesses straight into the constraint system. Robustness tests use it
/// to assert that every §7 constraint actually constrains: a mutation that
/// violates exactly one constraint must come back unsatisfiable, which a
/// `prove`-only test could never distinguish from a native rejection.
pub fn check_satisfiable(c: FelicaCircuit) -> bool {
    use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystem};
    let cs = ConstraintSystem::<Fr>::new_ref();
    if c.generate_constraints(cs.clone()).is_err() {
        return false;
    }
    cs.is_satisfied().unwrap_or(false)
}

/// Verify a proof against an explicit verifying key.
pub fn verify_proof(
    vk: &FelicaVerifyingKey,
    public_inputs: &[Fr],
    proof: &ark_groth16::Proof<Bn254>,
) -> bool {
    let pvk = ark_groth16::prepare_verifying_key(vk);
    ark_groth16::Groth16::<Bn254>::verify_with_processed_vk(&pvk, public_inputs, proof)
        .unwrap_or(false)
}

fn parse_fq(s: &str) -> Option<ark_bn254::Fq> {
    use ark_serialize::CanonicalDeserialize;
    let raw = hex::decode(s).ok()?;
    ark_bn254::Fq::deserialize_compressed(&raw[..]).ok()
}

fn parse_fr(s: &str) -> Option<Fr> {
    use ark_serialize::CanonicalDeserialize;
    let raw = hex::decode(s).ok()?;
    Fr::deserialize_compressed(&raw[..]).ok()
}

/// Why an [`Attestation`] was rejected.
///
/// The three variants are deliberately distinct, because conflating them is
/// how callers end up trusting an envelope they never checked:
///
/// * [`VerifyError::ProofInvalid`] -- the Groth16 statement itself is false.
///   Nothing in the envelope matters; the proof does not verify.
/// * [`VerifyError::EnvelopeInconsistent`] -- the proof is *valid*, but the
///   top-level metadata disagrees with the public inputs it commits to. A
///   caller reading `att.idi` would be misled about which statement was
///   proved. This is not a soundness failure of the proof system; it is an
///   integrity failure of the helper's contract.
/// * [`VerifyError::Malformed`] -- undecodable encoding: bad hex, wrong
///   length, off-curve coordinate, or an unsupported `alg` label.
///
/// Note that `attested_at` freshness is deliberately *not* covered. It is a
/// prover-supplied public scalar, not authoritative time; ordering and
/// replay policy belong to the verifier (spec §5, §11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum VerifyError {
    #[error("proof verification failed")]
    ProofInvalid,
    #[error("attestation envelope inconsistent with proof public inputs")]
    EnvelopeInconsistent,
    #[error("malformed attestation: {0}")]
    Malformed(&'static str),
}

/// Verify a hex-encoded [`Attestation`] against an explicit verifying key.
///
/// Checks the complete envelope, not just the pairing equation:
///
/// 1. `alg` is the supported label.
/// 2. coordinates decode, lie on the curve and in the correct subgroup;
/// 3. public inputs decode canonically (see [`abi`]) and the count matches;
/// 4. `idi` and `attested_at` equal the values the public inputs commit to;
/// 5. the pairing equation holds.
///
/// Never panics. Step 4 is what makes it safe to read `att.idi` after a
/// `true` result: previously a caller could mutate the top-level metadata,
/// keep a valid proof, and still be told the attestation verified.
///
/// Independent verifiers that decode public inputs themselves are unaffected
/// by step 4 -- the proof was always bound to those inputs; only the
/// convenience helper lied about having checked the rest.
pub fn verify_attestation_detailed(
    vk: &FelicaVerifyingKey,
    att: &Attestation,
) -> Result<(), VerifyError> {
    let proof = parse_proof_points(&att.proof)?;
    let pis = parse_public_inputs(&att.proof)?;

    if !verify_proof(vk, &pis, &proof) {
        return Err(VerifyError::ProofInvalid);
    }

    // The proof is valid. Now the envelope must agree with what it commits to.
    let decoded =
        PublicInputs::from_fr(&pis).map_err(|_| VerifyError::Malformed("non-canonical inputs"))?;
    if decoded.idi != att.idi {
        return Err(VerifyError::EnvelopeInconsistent);
    }
    if decoded.attested_at != att.attested_at {
        return Err(VerifyError::EnvelopeInconsistent);
    }
    Ok(())
}

/// Decode the hex-coordinate wire proof back into an Arkworks proof object.
///
/// Rejects an unsupported `alg`, undecodable hex, and points off the curve or
/// outside the prime-order subgroup — the same checks
/// [`verify_attestation_detailed`] has always run; they now live here so every
/// consumer of the wire form (local verification, Sui serialization) applies
/// them identically.
pub fn parse_proof_points(proof: &Groth16Proof) -> Result<ark_groth16::Proof<Bn254>, VerifyError> {
    if proof.alg != SUPPORTED_ALG {
        return Err(VerifyError::Malformed("unsupported alg label"));
    }
    let (ax, ay) = (&proof.a.0, &proof.a.1);
    let (cx, cy) = (&proof.c.0, &proof.c.1);
    let ((bx0, bx1), (by0, by1)) = (&proof.b.0, &proof.b.1);
    let (ax, ay, cx, cy) = match (parse_fq(ax), parse_fq(ay), parse_fq(cx), parse_fq(cy)) {
        (Some(a), Some(b), Some(c), Some(d)) => (a, b, c, d),
        _ => return Err(VerifyError::Malformed("undecodable G1 coordinate")),
    };
    let (bx0, bx1, by0, by1) = match (parse_fq(bx0), parse_fq(bx1), parse_fq(by0), parse_fq(by1)) {
        (Some(a), Some(b), Some(c), Some(d)) => (a, b, c, d),
        _ => return Err(VerifyError::Malformed("undecodable G2 coordinate")),
    };
    let a = ark_bn254::G1Affine::new_unchecked(ax, ay);
    let c = ark_bn254::G1Affine::new_unchecked(cx, cy);
    let b = ark_bn254::G2Affine::new_unchecked(
        ark_bn254::Fq2::new(bx0, bx1),
        ark_bn254::Fq2::new(by0, by1),
    );
    for p in [&a, &c] {
        if !p.is_on_curve() || !p.is_in_correct_subgroup_assuming_on_curve() {
            return Err(VerifyError::Malformed("G1 point off curve or subgroup"));
        }
    }
    if !b.is_on_curve() || !b.is_in_correct_subgroup_assuming_on_curve() {
        return Err(VerifyError::Malformed("G2 point off curve or subgroup"));
    }
    Ok(ark_groth16::Proof { a, b, c })
}

/// Decode the wire public inputs into field elements, count-checked.
pub fn parse_public_inputs(proof: &Groth16Proof) -> Result<Vec<Fr>, VerifyError> {
    if proof.public_inputs.len() != PUBLIC_INPUT_COUNT {
        return Err(VerifyError::Malformed("wrong public-input count"));
    }
    let mut pis = Vec::with_capacity(PUBLIC_INPUT_COUNT);
    for s in &proof.public_inputs {
        match parse_fr(s) {
            Some(f) => pis.push(f),
            None => return Err(VerifyError::Malformed("undecodable public input")),
        }
    }
    Ok(pis)
}

/// The Arkworks canonical *compressed* serialization of a wire proof — the
/// single blob `sui::groth16::proof_points_from_bytes` consumes.
///
/// This is a re-serialization of the same proof object the JSON coordinates
/// describe, not a second proof: [`prove_compressed`] emits the identical
/// bytes at proving time, and this function recovers them from the RPC form
/// for callers that only ever saw the JSON.
pub fn proof_compressed_bytes(proof: &Groth16Proof) -> Result<Vec<u8>, VerifyError> {
    let parsed = parse_proof_points(proof)?;
    let mut out = Vec::new();
    parsed
        .serialize_compressed(&mut out)
        .map_err(|_| VerifyError::Malformed("proof does not serialize"))?;
    Ok(out)
}

/// The packed 96-byte public-input blob (`3 × 32B` little-endian scalars) the
/// Move verifier's `felica_auth::new` expects, recovered from the wire form.
///
/// Round-trips through `Fr` rather than concatenating the hex directly, so a
/// non-canonical wire encoding is rejected here exactly as the verifier would
/// reject it on chain.
pub fn public_inputs_bytes(proof: &Groth16Proof) -> Result<Vec<u8>, VerifyError> {
    let pis = parse_public_inputs(proof)?;
    let mut out = Vec::with_capacity(PUBLIC_INPUT_COUNT * 32);
    for f in &pis {
        f.serialize_compressed(&mut out)
            .map_err(|_| VerifyError::Malformed("public input does not serialize"))?;
    }
    Ok(out)
}

/// Boolean form of [`verify_attestation_detailed`].
///
/// Kept for callers that only need a yes/no answer. Prefer the detailed form
/// where the distinction between a bad proof and a lying envelope matters,
/// because both collapse to `false` here.
pub fn verify_attestation(vk: &FelicaVerifyingKey, att: &Attestation) -> bool {
    verify_attestation_detailed(vk, att).is_ok()
}

/// Generate the attestation proof for one verified session.
///
/// Native pre-checks mirror the circuit so failures map to typed errors before
/// proving; the circuit then re-enforces the same relations in zero knowledge
/// *and* constrains the key schedule D1–D3 from `gsk`/`usk`/`idm`. `pk` is the
/// externally generated proving key — this function never generates keys.
pub fn prove(pk: &FelicaProvingKey, req: &ProveRequest) -> Result<Attestation, ProverError> {
    prove_compressed(pk, req).map(|(att, _)| att)
}

/// [`prove`], plus the Arkworks canonical compressed proof bytes.
///
/// The RPC surface hex-encodes each coordinate separately, which is what the
/// JSON consumers parse. `sui::groth16` instead consumes the Arkworks
/// *compressed* serialization as a single blob, so a Move verifier needs this
/// second form. Emitting it here keeps one proving path: the bytes are a
/// serialization of the same proof object, not a second proof system.
pub fn prove_compressed(
    pk: &FelicaProvingKey,
    req: &ProveRequest,
) -> Result<(Attestation, Vec<u8>), ProverError> {
    use des::{cbc_decrypt, command_mac, des_encrypt, tdes_decrypt, tdes_encrypt};

    // Native mirror of D1-D3, for typed pre-checks and hints only. The
    // circuit re-derives l/alpha/beta itself; nothing here is trusted.
    let l: [u8; 8] = std::array::from_fn(|i| req.gsk[i] ^ req.idm[i]);
    let alpha = des_encrypt(&req.usk, &l);
    let beta = des_encrypt(&l, &alpha);

    // Recover r1/r2; re-encrypt checks are tautological natively but pin the
    // C1B mapping for callers.
    let r1 = tdes_decrypt(&req.c1b, &l, &beta);
    if tdes_encrypt(&r1, &l, &beta) != req.c1b {
        return Err(ProverError::C1bMismatch);
    }
    let r2 = tdes_decrypt(&req.c2a, &l, &beta);
    if tdes_encrypt(&r2, &l, &beta) != req.c2a {
        return Err(ProverError::C1bMismatch);
    }

    // Constraint 3+4: CBC decrypt + MAC (opcode 0x13, payload 24B).
    let pt = cbc_decrypt(&req.auth2, &r2).ok_or(ProverError::MacMismatch)?;
    if pt.len() != 32 {
        return Err(ProverError::MacMismatch);
    }
    let mac = command_mac(0x13, &pt[..24]);
    if mac != pt[24..32] {
        return Err(ProverError::MacMismatch);
    }
    // Constraint 5: TID binding.
    if pt[2..8] != r1[2..8] {
        return Err(ProverError::TidMismatch);
    }
    // Constraint 6: IDi extraction.
    let mut idi = [0u8; 8];
    idi.copy_from_slice(&pt[8..16]);

    let circuit = FelicaCircuit {
        idi,
        r1,
        attested_at: req.attested_at,
        gsk: req.gsk,
        usk: req.usk,
        idm: req.idm,
        c1b: req.c1b,
        c2a: req.c2a,
        r2,
        auth2: req.auth2,
    };
    let mut rng = rand::thread_rng();
    let proof = ark_groth16::Groth16::<Bn254>::prove(pk, circuit, &mut rng)
        .map_err(|_| ProverError::ProveFailed)?;

    let pis = PublicInputs {
        idi,
        r1,
        attested_at: req.attested_at,
    }
    .to_fr()
    .map_err(|_| ProverError::ProveFailed)?;
    if !verify_proof(&pk.vk, &pis, &proof) {
        return Err(ProverError::ProveFailed);
    }

    let mut compressed = Vec::new();
    proof
        .serialize_compressed(&mut compressed)
        .map_err(|_| ProverError::ProveFailed)?;

    Ok((
        Attestation {
            idi,
            attested_at: req.attested_at,
            proof: Groth16Proof {
                alg: SUPPORTED_ALG.to_string(),
                a: (fq_to_hex(&proof.a.x), fq_to_hex(&proof.a.y)),
                b: (
                    (fq_to_hex(&proof.b.x.c0), fq_to_hex(&proof.b.x.c1)),
                    (fq_to_hex(&proof.b.y.c0), fq_to_hex(&proof.b.y.c1)),
                ),
                c: (fq_to_hex(&proof.c.x), fq_to_hex(&proof.c.y)),
                public_inputs: pis.iter().map(fr_to_hex).collect(),
            },
        },
        compressed,
    ))
}
