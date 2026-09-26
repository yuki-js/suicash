/// SuiCash's transaction surface over `felica_oracle::gate`.
///
/// `gate` and `zk_verifier` are deliberately library-shaped: pure functions,
/// caller-supplied clocks, a caller-chosen witness type. None of that is
/// callable from a PTB without a concrete deployment making three choices,
/// and this module is those choices, nothing more:
///
/// * **the witness** — `SUICASH`, so a SuiCash gate is its own type and a
///   verification cannot be satisfied by some other deployment's gate;
/// * **the clock** — `sui::clock::Clock`, because `verify_and_claim` takes
///   `now_seconds` from its caller and a *payer-chosen* timestamp would make
///   the drift check theatre;
/// * **the receipt** — an `AttestationVerified` event, so an indexer or the
///   payment terminal can watch verifications without replaying transactions.
///
/// The intended use is one PTB per payment: `verify` first, the coin split
/// and transfer after it. PTB commands are atomic, so the transfer simply
/// cannot happen unless the pairing check passed and the `r1` burn went
/// through in the same transaction.
module felica_oracle::suicash_gate;

use felica_oracle::felica_auth;
use felica_oracle::gate::{Self, Gate};
use sui::clock::Clock;
use sui::event;

/// The witness branding SuiCash's gate. Private by construction: no function
/// hands one out, so `Gate<SUICASH>` values exist only where this module
/// created them.
public struct SUICASH has drop {}

/// One accepted attestation. `idi` is the *verified* claim — the value the
/// pairing check bound, not the envelope's word for it.
public struct AttestationVerified has copy, drop {
    idi: vector<u8>,
    r1: vector<u8>,
    attested_at: u64,
    sender: address,
}

/// Create and share the SuiCash gate. Run once at deployment.
///
/// * `vk_bytes` — the Arkworks compressed verifying key this deployment pins
///   (360 bytes; `prover/assets/verifying_key.bin`).
/// * `allowlist` — IDi values to accept; empty admits any card the oracle
///   holds keys for. Narrowing later is `gate::allow` on the shared object.
/// * `max_clock_drift_seconds` — tolerated `|attested_at − Clock|`. Covers
///   the gap between the card tap (when the oracle timestamps the proof) and
///   the payment settling on chain, so minutes, not milliseconds.
public fun create_gate(
    vk_bytes: vector<u8>,
    allowlist: vector<vector<u8>>,
    max_clock_drift_seconds: u64,
    ctx: &mut TxContext,
) {
    gate::share(gate::create<SUICASH>(vk_bytes, allowlist, max_clock_drift_seconds, ctx))
}

/// Verify one attestation against the shared gate, burn its `r1`, and emit
/// the receipt. Aborts — and with it the whole PTB — on any failure.
///
/// The wire fields are exactly the oracle's `attest` result after
/// `prover::proof_compressed_bytes` / `prover::public_inputs_bytes`:
/// `proof` is the Arkworks compressed proof blob, `public_inputs` the packed
/// 96-byte scalar triple. `presented_r1` is the holder's challenge from the
/// card session; the gate checks it against the proof's binding *and* its
/// dedup store, which is what makes each attestation single-spend.
public fun verify(
    g: &mut Gate<SUICASH>,
    idi: vector<u8>,
    attested_at: u64,
    proof: vector<u8>,
    public_inputs: vector<u8>,
    presented_r1: vector<u8>,
    clock: &Clock,
    ctx: &TxContext,
) {
    let att = felica_auth::new(b"groth16-bn254", idi, attested_at, proof, public_inputs);
    let verified_idi = gate::verify_and_claim(g, &att, presented_r1, clock.timestamp_ms() / 1000);
    event::emit(AttestationVerified {
        idi: verified_idi,
        r1: presented_r1,
        attested_at,
        sender: ctx.sender(),
    });
}
