/// On-chain Groth16 verification of a FeliCa attestation.
///
/// Thin by design. The circuit is the statement; this module only checks that
/// the proof is a proof of *that* statement, then hands the claimed IDi back.
/// It deliberately does not attempt to decide whether the card was present —
/// see the warning at the bottom, which is not optional reading.
///
/// # What a successful call means, precisely
///
/// Some party knew a witness satisfying the circuit for the public inputs
/// `idi`, `r1`, `attested_at`, and the proof binds exactly those values. Since
/// D1–D3 derive the key schedule in-circuit from `gsk`/`usk`/`idm`, that
/// witness is someone who knows the FeliCa system master keys for that card's
/// key version — or someone who invented a self-consistent set of them, which
/// remains possible (issue #1, `prover/README.md`). **A verified attestation is
/// not proof of card presence, and not proof that any particular oracle
/// produced it.** Deployments that need either property must add their own
/// gate on top; this module cannot supply it.
module felica_oracle::zk_verifier;

use felica_oracle::felica_auth::{Self, Attestation};
use sui::groth16::{Self, PreparedVerifyingKey};

/// The pairing equation does not hold for these public inputs.
const EProofInvalid: u64 = 1;

/// A pinned circuit identity: the Arkworks compressed verifying key, plus its
/// prepared form.
///
/// The `prepared` copy is built once at construction. `prepare_verifying_key`
/// costs one pairing, and doing it per verification would charge every caller
/// for a value that never changes. Because the key is baked into the type, a
/// published attestation is always checked against the key this object was
/// created with — the key is never negotiated per session.
public struct VerifyingKey has drop, store {
    /// Arkworks canonical compressed verifying key, kept for auditability: it
    /// is the value a deployment pins and the value `fixture::vk()` should
    /// equal.
    vk_bytes: vector<u8>,
    prepared: PreparedVerifyingKey,
}

/// Pin a verifying key.
///
/// Aborts if the bytes are not a valid Arkworks BN254 verifying key: the abort
/// comes from `sui::groth16`, which rejects off-curve and wrong-subgroup
/// points as well as bad shapes, and its code is not ours to re-export.
public fun key(vk_bytes: vector<u8>): VerifyingKey {
    VerifyingKey {
        vk_bytes,
        prepared: groth16::prepare_verifying_key(&groth16::bn254(), &vk_bytes),
    }
}

/// The pinned verifying key, as embedded.
public fun vk_bytes(k: &VerifyingKey): vector<u8> { k.vk_bytes }

/// Verify `att` against the pinned key and return the claimed IDi.
///
/// Performs, in order:
///
/// 1. envelope consistency — `idi` and `attested_at` must equal what the
///    public inputs commit to, and each scalar must be a canonical 8-byte limb;
/// 2. the pairing equation over BN254.
///
/// Aborts on any failure. Returns the claimed IDi, which is now known to be
/// the IDi the proof commits to — not a card that was shown to anyone.
///
/// The caller is responsible for freshness: this function is pure, so it
/// cannot tell a first presentation from a replay. Pass the holder's `r1`
/// and let `felica_oracle::gate` do the dedup, or bring your own state.
public fun verify(k: &VerifyingKey, att: &Attestation): vector<u8> {
    felica_auth::assert_envelope_consistent(att);

    let inputs = groth16::public_proof_inputs_from_bytes(felica_auth::public_inputs(att));
    let points = groth16::proof_points_from_bytes(felica_auth::proof_points(att));
    assert!(
        groth16::verify_groth16_proof(
            &groth16::bn254(),
            &k.prepared,
            &inputs,
            &points
        ),
        EProofInvalid
    );

    felica_auth::idi(att)
}
