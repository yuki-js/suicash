/// SuiCash's transaction surface over `felica_oracle::gate`.
///
/// `gate` and `zk_verifier` are deliberately library-shaped: pure functions,
/// caller-supplied clocks, a caller-chosen witness type. None of that is
/// callable from a PTB without a concrete deployment making three choices,
/// and this module is those choices, plus the one the library cannot make for
/// itself — *who may administer the shared gate*:
///
/// * **the witness** — `SUICASH`, so a SuiCash gate is its own type and a
///   verification cannot be satisfied by some other deployment's gate;
/// * **the clock** — `sui::clock::Clock`, because `verify_and_claim` takes
///   `now_seconds` from its caller and a *payer-chosen* timestamp would make
///   the drift check theatre;
/// * **the receipt** — an `AttestationVerified` event, so an indexer or the
///   payment terminal can watch verifications without replaying transactions.
/// * **the access control** — OpenZeppelin `access_control`, because the admin
///   mutators (`create_gate`, `allow`, `disallow`, `set_max_clock_drift_seconds`)
///   were previously permissionless `public` functions: any stranger could
///   narrow a shared gate (griefing) or churn its allowlist from a PTB. The
///   library's mutators are now `public(package)`; the only PTB-reachable path
///   to them is through the `Auth`-gated wrappers below.
///
/// The intended use is one PTB per payment: `verify` first, the coin split
/// and transfer after it. PTB commands are atomic, so the transfer simply
/// cannot happen unless the pairing check passed and the `r1` burn went
/// through in the same transaction.
///
/// # Roles
///
/// * `SUICASH_GATE` — this module's one-time witness and the registry's root
///   role. Held by the publisher after `init`; governs `Operator` membership
///   and the timelocked root handoff.
/// * `Operator` — may manage the allowlist and the drift bound.
///
/// A privileged action takes `&Auth<Role>` and performs no further check: the
/// witness is unforgeable because the only registry that can mint it is the
/// one rooted at this module. Callers mint it and spend it in the same PTB:
///
/// ```move
/// let auth = ac.new_auth<_, Operator>(ctx); // aborts unless sender is an operator
/// suicash_gate::allow(&mut gate, &auth, idi);
/// ```
module felica_oracle::suicash_gate;

use felica_oracle::felica_auth;
use felica_oracle::gate::{Self, Gate};
use openzeppelin_access::access_control::{Self, AccessControl, Auth};
use sui::clock::Clock;
use sui::event;

/// Timelock on root-admin transfer and renounce: 24 hours in milliseconds.
///
/// Long enough that a compromised publisher key cannot silently hand the
/// registry to an attacker; short enough that a planned rotation completes
/// within a day. Tunable later through the registry's own delayed flow.
const ADMIN_DELAY_MS: u64 = 24 * 60 * 60 * 1_000;

/// The witness branding SuiCash's gate. Private by construction: no function
/// hands one out, so `Gate<SUICASH>` values exist only where this module
/// created them.
public struct SUICASH has drop {}

/// This module's one-time witness and the `AccessControl` root role.
///
/// Minted by Sui exactly once, at first publish, and consumed by `init`.
/// Whoever publishes the package becomes the first root admin.
public struct SUICASH_GATE has drop {}

/// May manage the allowlist and the drift bound.
///
/// Administered by the root role: only the root admin (or an address it
/// delegated to) can grant or revoke it, via `access_control::grant_role` /
/// `revoke_role` on the shared registry.
public struct Operator {}

/// One accepted attestation. `idi` is the *verified* claim — the value the
/// pairing check bound, not the envelope's word for it.
public struct AttestationVerified has copy, drop {
    idi: vector<u8>,
    r1: vector<u8>,
    attested_at: u64,
    sender: address,
}

/// Stand up the protocol on first publish: create the registry rooted at this
/// module's OTW, make the publisher an operator so the deployment can proceed
/// without an extra transaction, and share the registry.
///
/// Runs automatically at publish; never call it directly.
fun init(otw: SUICASH_GATE, ctx: &mut TxContext) {
    let mut registry = access_control::new(otw, ADMIN_DELAY_MS, ctx);
    registry.grant_role<_, Operator>(ctx.sender(), ctx);
    transfer::public_share_object(registry);
}

/// Create and share the SuiCash gate. Gated by the root role: only the
/// deployment owner can pin a new verifying key.
///
/// Run once per deployment. Re-publishing starts the new gate with empty
/// dedup state (burned `r1` values).
///
/// * `vk_bytes` — the Arkworks compressed verifying key this deployment pins
///   (360 bytes; `prover/assets/verifying_key.bin`).
/// * `allowlist` — IDi values to accept; empty admits any card the oracle
///   holds keys for. Narrowing later is `allow` on the shared object.
/// * `max_clock_drift_seconds` — tolerated `|attested_at − Clock|`. Covers
///   the gap between the card tap (when the oracle timestamps the proof) and
///   the payment settling on chain, so minutes, not milliseconds.
public fun create_gate(
    _: &Auth<SUICASH_GATE>,
    vk_bytes: vector<u8>,
    allowlist: vector<vector<u8>>,
    max_clock_drift_seconds: u64,
    ctx: &mut TxContext,
) {
    gate::share(gate::create<SUICASH>(vk_bytes, allowlist, max_clock_drift_seconds, ctx))
}

/// Add an IDi to the shared gate's allowlist. Gated by `Operator`.
///
/// Narrowing is irreversible by design (see `gate::allow`): once a deployment
/// expresses a preference, "no preference" is no longer reachable.
public fun allow(g: &mut Gate<SUICASH>, _: &Auth<Operator>, idi: vector<u8>) {
    gate::allow(g, idi);
}

/// Remove an IDi from the shared gate's allowlist. Gated by `Operator`.
///
/// Aborts if the IDi is not listed. Removing the last entry leaves the gate
/// closed, not open.
public fun disallow(g: &mut Gate<SUICASH>, _: &Auth<Operator>, idi: vector<u8>) {
    gate::disallow(g, idi);
}

/// Retune the coarse `attested_at` drift bound. Gated by `Operator`.
public fun set_max_clock_drift_seconds(
    g: &mut Gate<SUICASH>,
    _: &Auth<Operator>,
    seconds: u64,
) {
    gate::set_max_clock_drift_seconds(g, seconds);
}

/// Verify one attestation against the shared gate, burn its `r1`, and emit
/// the receipt. Aborts — and with it the whole PTB — on any failure.
///
/// Permissionless by design: any payer must be able to spend one attestation.
/// Administration is what is gated, not payment.
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

/// Mint a root proof for PTB construction.
///
/// Thin forwarder over OpenZeppelin's `access_control::new_auth`, kept in
/// this module so wallets and scripts only need this package's ID: one
/// `--move-call $PKG::suicash_gate::new_root_auth` instead of tracking the
/// `openzeppelin_access` package ID as well. Aborts unless the caller holds
/// the root role.
public fun new_root_auth(
    ac: &AccessControl<SUICASH_GATE>,
    ctx: &mut TxContext,
): Auth<SUICASH_GATE> {
    access_control::new_auth(ac, ctx)
}

/// Mint an operator proof for PTB construction.
///
/// Same rationale as `new_root_auth`. Aborts unless the caller holds
/// `Operator`.
public fun new_operator_auth(
    ac: &AccessControl<SUICASH_GATE>,
    ctx: &mut TxContext,
): Auth<Operator> {
    access_control::new_auth(ac, ctx)
}

/// The root-transfer timelock, in milliseconds. Records the OpenZeppelin
/// policy where operators can see it.
public fun admin_delay_ms(): u64 { ADMIN_DELAY_MS }

/// Run `init` under test, constructing the OTW manually (allowed in a
/// `#[test_only]` context) so a scenario can stand up the protocol.
#[test_only]
public fun init_for_testing(ctx: &mut TxContext) {
    init(SUICASH_GATE {}, ctx);
}
