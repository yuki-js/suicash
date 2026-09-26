/// FeliCa attestation, the wire shape a Sui Move verifier consumes.
///
/// This module owns the *bytes → statement* decoding and nothing else: it
/// parses what the oracle sent, exposes the envelope fields, and cross-checks
/// the envelope against the public inputs the proof commits to. It performs no
/// cryptography — `felica_oracle::zk_verifier` does that.
///
/// The layout is the one `prover::abi` defines, and the two must be changed
/// together:
///
/// ```text
/// public_inputs (96 bytes, three 32-byte little-endian scalars)
///   pi0 = idi        (8B LE, upper 24 bytes zero)
///   pi1 = r1         (8B LE, upper 24 bytes zero)
///   pi2 = attested_at (u64 LE, upper 24 bytes zero)
/// ```
module felica_oracle::felica_auth;

/// Number of public inputs the circuit publishes.
const PUBLIC_INPUT_COUNT: u64 = 3;
/// Byte width of one public-input scalar, as `sui::groth16` requires.
const SCALAR_BYTES: u64 = 32;
/// Byte width of every limb in this packing.
const LIMB_BYTES: u64 = 8;
/// The only supported proof algorithm label.
const SUPPORTED_ALG: vector<u8> = b"groth16-bn254";

/// Malformed wire shape: wrong length, or a scalar blob that is not 96 bytes.
const EMalformed: u64 = 0;
/// The `alg` label is not `groth16-bn254`.
const EUnsupportedAlg: u64 = 1;
/// The envelope disagrees with what the public inputs commit to, or a scalar
/// is not the canonical encoding of its 8-byte limb.
const EEnvelopeInconsistent: u64 = 2;
/// The presented holder challenge is not the one the proof is bound to.
const EChallengeMismatch: u64 = 3;
/// `attested_at` is further from the verifier's clock than policy allows.
const EClockDrift: u64 = 4;

/// A Groth16/BN254 attestation over FeliCa IDi, as published by the oracle.
///
/// The fields mirror the RPC `attest` result. `proof` and `public_inputs` are
/// the *serialized* forms `sui::groth16` accepts, not the JSON coordinate
/// lists: a Move transaction has no use for the hex view, and converting on
/// chain would only add a way to get the conversion wrong.
public struct Attestation has drop {
    /// Algorithm label. Must be `groth16-bn254`; anything else is rejected
    /// rather than ignored, because an unrecognised label means the producer
    /// meant something this verifier does not implement.
    alg: vector<u8>,
    /// Claimed card identifier (8 bytes).
    idi: vector<u8>,
    /// Prover-chosen Unix timestamp in seconds. A drift signal, never an
    /// ordering key — see `clock_drift_ok`.
    attested_at: u64,
    /// Arkworks canonical compressed proof points.
    proof: vector<u8>,
    /// The three public-input scalars, 32 bytes little-endian each.
    public_inputs: vector<u8>,
}

/// Build an attestation from its wire parts.
///
/// Aborts on a malformed shape: a bad `alg`, or a `public_inputs` blob that is
/// not exactly 96 bytes. The *contents* of those scalars are checked in
/// `assert_envelope_consistent`, which is where a claim can be compared with
/// what the proof actually commits to.
public fun new(
    alg: vector<u8>,
    idi: vector<u8>,
    attested_at: u64,
    proof: vector<u8>,
    public_inputs: vector<u8>,
): Attestation {
    assert!(alg == SUPPORTED_ALG, EUnsupportedAlg);
    assert!(idi.length() == LIMB_BYTES, EMalformed);
    assert!(
        public_inputs.length() == PUBLIC_INPUT_COUNT * SCALAR_BYTES,
        EMalformed
    );
    Attestation { alg, idi, attested_at, proof, public_inputs }
}

/// The claimed card identifier.
public fun idi(att: &Attestation): vector<u8> { att.idi }

/// The prover-chosen timestamp, in Unix seconds.
public fun attested_at(att: &Attestation): u64 { att.attested_at }

/// The algorithm label, already checked against `SUPPORTED_ALG` by `new`.
public fun alg(att: &Attestation): vector<u8> { att.alg }

/// The Arkworks compressed proof points, for `sui::groth16`.
public fun proof_points(att: &Attestation): vector<u8> { att.proof }

/// The packed public-input scalars, for `sui::groth16`.
public fun public_inputs(att: &Attestation): vector<u8> { att.public_inputs }

/// The holder challenge `r1` this proof is bound to (`pi1`).
///
/// This is the freshness anchor: it comes from the holder's CSPRNG, whereas
/// `attested_at` is chosen by the prover and can be anything.
public fun r1(att: &Attestation): vector<u8> { limb(att, 1) }

/// Check that the top-level metadata agrees with the public inputs.
///
/// The pairing equation already binds `public_inputs`, so a mismatch here is
/// not a soundness failure — it is a caller being told something the proof
/// never said. `idi` and `attested_at` are read out of the envelope by
/// downstream code, so an unchecked mismatch would hand that code a
/// statement nobody proved. This is the on-chain counterpart of
/// `prover::verify_attestation_detailed`'s `EnvelopeInconsistent`.
///
/// Also enforces the packing's canonicality rule: every limb is 8 bytes, so
/// the upper 24 bytes of each 32-byte scalar must be zero. Without that check
/// a scalar is only *equal* to its canonical encoding, not equal as a byte
/// string, and two distinct blobs could describe one statement.
public fun assert_envelope_consistent(att: &Attestation) {
    assert_canonical(att, 0);
    assert_canonical(att, 1);
    assert_canonical(att, 2);
    assert!(limb(att, 0) == att.idi, EEnvelopeInconsistent);
    assert!(scalar_u64(att, 2) == att.attested_at, EEnvelopeInconsistent);
}

/// Check that the holder presented the `r1` the proof is bound to.
///
/// The verifier is the party that knows which challenge it issued, so this
/// check cannot be delegated to the oracle: without it, a valid proof for
/// *someone else's* fresh session would be accepted here.
public fun assert_challenge_matches(att: &Attestation, presented_r1: vector<u8>) {
    assert!(presented_r1.length() == LIMB_BYTES, EMalformed);
    assert!(limb(att, 1) == presented_r1, EChallengeMismatch);
}

/// Coarse clock-drift check on `attested_at`.
///
/// Deliberately weak, and deliberately the only use of `attested_at`: the
/// value is a prover-chosen public scalar, so it cannot order attestations or
/// establish freshness. It is compared against the verifier's own clock only
/// to catch gross forgery or a badly skewed oracle. Replay protection is
/// `felica_oracle::gate`'s job, because it requires state and this cannot
/// have any.
public fun assert_clock_drift_ok(att: &Attestation, now_seconds: u64, max_drift_seconds: u64) {
    let drift = if (att.attested_at > now_seconds) {
        att.attested_at - now_seconds
    } else {
        now_seconds - att.attested_at
    };
    assert!(drift <= max_drift_seconds, EClockDrift);
}

/// The `index`-th public-input scalar as a 32-byte little-endian blob.
///
/// Copied out byte by byte: `std::vector` has no `slice` in the edition this
/// package builds against, and the alternative — passing the whole 96-byte
/// blob to `sui::groth16` and hoping the caller lined it up — is exactly the
/// kind of unchecked shape assumption this module exists to remove.
public fun scalar(att: &Attestation, index: u64): vector<u8> {
    assert!(index < PUBLIC_INPUT_COUNT, EMalformed);
    let mut out = vector[];
    let mut i = 0;
    while (i < SCALAR_BYTES) {
        out.push_back(*att.public_inputs.borrow(index * SCALAR_BYTES + i));
        i = i + 1;
    };
    out
}

/// The `index`-th limb, decoded to its 8 wire bytes.
///
/// Aborts if the scalar carries any high byte, i.e. if the packing is not
/// canonical for an 8-byte limb. The result is *shorter* than [`scalar`]: the
/// 24 zero bytes are dropped, because a caller comparing this against an IDi
/// or a challenge must be comparing equal-width things.
public fun limb(att: &Attestation, index: u64): vector<u8> {
    let bytes = scalar(att, index);
    assert!(is_canonical_limb(&bytes), EEnvelopeInconsistent);
    let mut out = vector[];
    let mut i = 0;
    while (i < LIMB_BYTES) {
        out.push_back(*bytes.borrow(i));
        i = i + 1;
    };
    out
}

/// The `index`-th limb read as a little-endian `u64`.
///
/// Decoded by hand because `u64::from_le_bytes` does not exist in the stdlib
/// this package builds against, and BCS is the wrong tool: it would accept
/// lengths and endianness the packing never produces.
public fun scalar_u64(att: &Attestation, index: u64): u64 {
    let bytes = limb(att, index);
    let mut value: u64 = 0;
    let mut shift: u8 = 0;
    let mut i = 0;
    while (i < LIMB_BYTES) {
        value = value | (((*bytes.borrow(i) as u64) << shift));
        shift = shift + 8;
        i = i + 1;
    };
    value
}

/// Is this scalar the canonical 32-byte encoding of an 8-byte limb?
///
/// Canonical here means: nothing above the low 8 bytes. The field modulus is
/// far above 2^64, so no 8-byte limb can be non-canonical *by value* — the
/// check exists to keep the mapping from blob to statement injective, and to
/// make a future repacking that widens a limb fail loudly instead of silently.
public fun is_canonical_limb(bytes: &vector<u8>): bool {
    if (bytes.length() != SCALAR_BYTES) return false;
    let mut i = LIMB_BYTES;
    while (i < SCALAR_BYTES) {
        if (bytes[i] != 0) return false;
        i = i + 1;
    };
    true
}

/// Abort unless the `index`-th scalar is a canonical 8-byte limb.
fun assert_canonical(att: &Attestation, index: u64) {
    assert!(is_canonical_limb(&scalar(att, index)), EEnvelopeInconsistent);
}
