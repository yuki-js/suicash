//! Adversarial authority-binding regression (issue #1, P0).
//!
//! # What this file asserts
//!
//! After the IDi-level revision, the circuit constrains the FeliCa key
//! schedule (D1–D3) from `gsk`/`usk`/`idm` instead of taking `l`/`beta` as
//! free witnesses. The question this file answers empirically is whether that
//! actually stops a forger.
//!
//! It does not — and these tests are written to *demonstrate* that rather than
//! to assert a fix. The reason is structural: `l` is defined as
//! `gsk ⊕ idm`, so an attacker who cannot produce the real master keys can
//! simply *invent* `gsk`, `usk` and `idm`. D1–D3 then derive `l`, `alpha`,
//! `beta` consistently, and the witness set is isomorphic to the old one.
//! The set of satisfying assignments is unchanged; only the parameterisation
//! moved from `{l, beta}` to `{gsk, usk, idm}`.
//!
//! The gain from D1–D3 is therefore real but narrow, and it is *not*
//! anti-forgery. It is:
//!
//! 1. The oracle can no longer pass a `l`/`beta` that disagrees with its own
//!    key custody — the two derivations are now pinned together in-circuit.
//! 2. The statement is self-documenting: it says which inputs the session
//!    keys come from, instead of leaving them as unexplained witnesses.
//!
//! What it does **not** do is bind the keys to anything the verifier knows.
//! Closing issue #1 requires a value the attacker cannot choose, which means
//! pinning a commitment to the key material as a public input (spec §10.2
//! option 1/2). That is not implemented here; see
//! `keys_are_not_pinned_to_a_verifier_known_value`.
//!
//! # Constraints on the forgery
//!
//! * Only the **public** proving/verifying keys are used. No card, no
//!   emulator, no oracle.
//! * The **normal prover path is not called**. `prover::prove` runs
//!   native pre-checks first, which would mask the circuit hole entirely.
//!   `ark_groth16::Groth16::prove` is called directly, exactly as an attacker
//!   would.
//! * The forged transcript is *self-consistent*: valid 3DES relations, a
//!   correct MAC over the fabricated AUTH2 payload, and a TID matching the
//!   fabricated R1. Nothing here relies on a cryptographic break.

// Each test binary uses a subset of `common`, so unused items are expected.
#![allow(dead_code)]

mod common;

use ark_bn254::{Bn254, Fr};
use ark_groth16::Groth16;
use ark_snark::SNARK;
use ark_std::rand::SeedableRng;
use common::*;
use prover::circuit::FelicaCircuit;
use prover::des::{cbc_encrypt_zero_iv, command_mac, tdes_encrypt};

/// A complete, self-consistent forged session produced with no card and no
/// oracle. The attacker invents the key material.
struct Forgery {
    circuit: FelicaCircuit,
    /// The invented key material, recorded for the independence assertions.
    gsk: [u8; 8],
    usk: [u8; 8],
    idm: [u8; 8],
}

/// Build a forgery: invent `gsk`/`usk`/`idm`, then derive every other value so
/// D1–D3 and C1–C6 all hold.
///
/// The attacker no longer chooses `l`/`beta` — that was the point of the
/// revision. They choose the *inputs* to the derivation instead, which is
/// just as unconstrained.
fn forge() -> Forgery {
    // Invented arbitrarily. None of these is the real card's key material.
    let gsk: [u8; 8] = [0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77];
    let usk: [u8; 8] = [0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11, 0x00];
    let idm: [u8; 8] = [0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x11, 0x22, 0x33];

    // D1–D3, derived natively exactly as the circuit derives them. The
    // attacker is doing arithmetic here, not breaking anything.
    let l: [u8; 8] = std::array::from_fn(|i| gsk[i] ^ idm[i]);
    let alpha = prover::des::des_encrypt(&usk, &l);
    let beta = prover::des::des_encrypt(&l, &alpha);

    // Public inputs, chosen freely.
    let r1: [u8; 8] = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x00, 0x11];
    let r2: [u8; 8] = [0x0F, 0x1E, 0x2D, 0x3C, 0x4B, 0x5A, 0x69, 0x78];

    // Constraint 1: c1b = 3DES(l, beta, r1)
    let c1b = tdes_encrypt(&r1, &l, &beta);
    // Constraint 2: c2a = 3DES(l, beta, r2)
    let c2a = tdes_encrypt(&r2, &l, &beta);

    // AUTH2 plaintext: TN(2) TID(6) IDi(8) PMi(8) MAC(8).
    // TID must equal tail_6(r1) for constraint 5.
    let mut p = Vec::with_capacity(32);
    p.extend_from_slice(&0u16.to_le_bytes());
    p.extend_from_slice(&r1[2..8]);
    // IDi is a *public input*: the attacker asserts any identity it likes.
    let idi: [u8; 8] = [0xC0, 0xFF, 0xEE, 0x00, 0x11, 0x22, 0x33, 0x44];
    p.extend_from_slice(&idi);
    p.extend_from_slice(&[0x50; 8]); // PMi, unconstrained
    debug_assert_eq!(p.len(), 24);
    // Constraint 4: the MAC is a correct MAC over the payload.
    let mac = command_mac(0x13, &p);
    p.extend_from_slice(&mac);
    debug_assert_eq!(p.len(), 32);

    // Constraint 3: auth2 = CBC-encrypt under r2, zero IV.
    let auth2 = cbc_encrypt_zero_iv(&p, &r2);

    Forgery {
        circuit: FelicaCircuit {
            idi,
            r1,
            attested_at: 0xDEAD_BEEF, // arbitrary
            gsk,
            usk,
            idm,
            c1b,
            c2a,
            r2,
            auth2: auth2.try_into().expect("32B ciphertext"),
        },
        gsk,
        usk,
        idm,
    }
}

/// The fabricated transcript satisfies the circuit's raw R1CS, D1–D3
/// included. Asserted first so that a later verifier-acceptance cannot be
/// mistaken for a witness that was never valid in the first place.
#[test]
fn forged_witness_satisfies_the_circuit() {
    let f = forge();
    assert!(
        prover::check_satisfiable(f.circuit.clone()),
        "the fabricated transcript must satisfy D1-D3 and C1-C6, \
         otherwise the rest of this file proves nothing"
    );
}

/// THE regression test for issue #1 — and it documents that the IDi-level
/// revision did **not** fix it.
///
/// The attacker uses only the public proving key, invents the key material,
/// fabricates the whole transcript, and the attestation verifies. Marked
/// `#[ignore]` so the suite stays usable, but it is the release gate: it must
/// be made to pass by pinning the key material to a verifier-known value, and
/// nothing else counts as a fix. Regenerating the proving key does not help —
/// the key is public material. Restating D1–D3 does not help — the attacker
/// satisfies them by construction, which is what this test demonstrates.
///
/// The only legitimate resolution is spec §10.2: constrain the derivation to a
/// key configuration the verifier knows, i.e. publish a commitment to `gsk`
/// as a public input and prove knowledge of its preimage. That changes the
/// circuit and requires regenerating both keys.
#[test]
#[ignore = "issue #1 (P0) REMAINS OPEN: D1-D3 pin l to invented gsk/usk/idm, \
            so a forgery with invented key material still verifies. Release gate."]
fn forged_attestation_from_public_proving_key_is_rejected() {
    let pk = test_key();
    let f = forge();

    // Prove with the public key and a fabricated witness, bypassing
    // `prover::prove` and its native pre-checks entirely.
    let mut rng = ark_std::rand::rngs::StdRng::from_seed([7u8; 32]);
    let proof = Groth16::<Bn254>::prove(pk, f.circuit.clone(), &mut rng)
        .expect("a Groth16 prover accepts any satisfying witness");

    // Build the envelope an attacker would present, with the forged public
    // inputs derived through the same canonical ABI decoder the oracle uses.
    let pis = f
        .circuit
        .public_inputs()
        .to_fr()
        .expect("forged public inputs are canonical");
    let att = prover::Attestation {
        idi: f.circuit.idi,
        attested_at: f.circuit.attested_at,
        proof: prover::Groth16Proof {
            alg: prover::SUPPORTED_ALG.to_string(),
            a: (fq_hex(&proof.a.x), fq_hex(&proof.a.y)),
            b: (
                (fq_hex(&proof.b.x.c0), fq_hex(&proof.b.x.c1)),
                (fq_hex(&proof.b.y.c0), fq_hex(&proof.b.y.c1)),
            ),
            c: (fq_hex(&proof.c.x), fq_hex(&proof.c.y)),
            public_inputs: pis.iter().map(fr_hex).collect(),
        },
    };

    let err = prover::verify_attestation_detailed(&pk.vk, &att).expect_err(
        "issue #1: a forgery built from the PUBLIC proving key, with invented \
                     key material and no card, must not verify",
    );

    // The rejection must be a *proof* rejection, not a malformed-input
    // technicality: if it were the latter, the forgery would still be
    // structurally acceptable and only the decoder would be objecting.
    assert_eq!(
        err,
        prover::VerifyError::ProofInvalid,
        "the forgery must fail the pairing equation itself"
    );
}

/// The forgery is entirely independent of the *real* key material: the
/// invented `gsk`/`usk`/`idm` differ from the genuine card's, so no change to
/// key custody can affect the outcome.
#[test]
fn forgery_needs_no_key_material_at_all() {
    let f = forge();
    let (_card, real_gsk, real_usk) = setup_card();
    assert_ne!(
        real_gsk, f.gsk,
        "the forged gsk must not be the genuine one"
    );
    assert_ne!(
        real_usk, f.usk,
        "the forged usk must not be the genuine one"
    );
    assert_ne!(IDM, f.idm, "the forged idm must not be the genuine card's");
}

/// Pins the residual trust explicitly, so it can never be quietly forgotten:
/// the public statement says only "∃ gsk, usk, idm making the transcript
/// consistent with idi". Nothing ties those keys to the deployed oracle, so a
/// forger's invented keys are as good as the real ones.
#[test]
fn keys_are_not_pinned_to_a_verifier_known_value() {
    let f = forge();
    // All three key inputs are private witnesses, not public inputs.
    let public = f.circuit.public_inputs();
    // The only thing a verifier learns is the identity claim.
    assert_eq!(public.idi, f.circuit.idi);
    assert_eq!(public.r1, f.circuit.r1);
    assert_eq!(public.attested_at, f.circuit.attested_at);
    // There is no public field that a commitment to the keys could occupy.
    assert_eq!(
        prover::PUBLIC_INPUT_COUNT,
        3,
        "with no key-commitment public input, spec 10.2 is unimplemented"
    );
}

fn fr_hex(f: &Fr) -> String {
    use ark_serialize::CanonicalSerialize;
    let mut b = Vec::new();
    f.serialize_compressed(&mut b).expect("Fr serializes");
    hex::encode(b)
}

fn fq_hex(f: &ark_bn254::Fq) -> String {
    use ark_serialize::CanonicalSerialize;
    let mut b = Vec::new();
    f.serialize_compressed(&mut b).expect("Fq serializes");
    hex::encode(b)
}

/// The honest session, for contrast: it still verifies, so a rejection in
/// `forged_attestation_from_public_proving_key_is_rejected` would be specific
/// to the forgery rather than a blanket refusal.
#[test]
fn honest_session_still_verifies() {
    let (c1b, c2a, auth2, gsk, usk) = mint_fixed(&R1);
    let att = prover::prove(test_key(), &prove_req(c1b, c2a, auth2, gsk, usk))
        .expect("honest session proves");
    assert_eq!(
        prover::verify_attestation_detailed(&test_key().vk, &att),
        Ok(()),
        "the revision must not break honest attestations"
    );
}
