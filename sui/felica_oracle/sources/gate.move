/// A shared, stateful gate that turns a pure attestation check into a
/// single-use authorisation.
///
/// `felica_oracle::zk_verifier` is deliberately stateless: a stateless check
/// cannot tell a first presentation from a replay, and pretending otherwise is
/// how replays get through. This module supplies the state that requires —
///
/// * **R1 dedup.** `r1` comes from the holder's CSPRNG and is the only
///   freshness anchor in the protocol, so an accepted `r1` is burned and a
///   second presentation aborts.
/// * **IDi allowlist.** Optional. Empty means "any card the oracle holds keys
///   for"; non-empty means exactly those cards.
/// * **Clock drift.** A coarse sanity bound on `attested_at`, which is a
///   prover-chosen scalar and therefore never an ordering key.
///
/// Every one of these is *verifier* policy. The oracle is stateless and
/// enforces none of them, so a deployment that skips this module has no
/// replay protection at all.
module felica_oracle::gate;

use felica_oracle::felica_auth::{Self, Attestation};
use felica_oracle::zk_verifier::{Self, VerifyingKey};
use sui::object::{Self, UID};
use sui::table::{Self, Table};

/// The claimed IDi is not on the allowlist.
const ENotAllowed: u64 = 0;
/// This holder challenge was already accepted.
const EReplay: u64 = 1;
/// `disallow` was called for an IDi that is not on the allowlist.
const EUnknownIdi: u64 = 2;

/// Shared state for one FeliCa attestation gate.
public struct Gate<phantom Key: drop> has key {
    id: UID,
    /// The pinned circuit identity. Never negotiated per session.
    verifier_key: VerifyingKey,
    /// Accepted `r1` values, burned on first use.
    seen: Table<vector<u8>, bool>,
    /// IDi allowlist. Meaningless while `allow_any` is set.
    allowlist: vector<vector<u8>>,
    /// Whether any IDi is admitted. Set at creation, and cleared forever by
    /// the first `allow`.
    ///
    /// A separate flag rather than "an empty list means everything", because
    /// that reading fails open: `disallow`ing the last entry would silently
    /// turn a one-card gate into an any-card gate. Here, touching the policy
    /// in either direction can only ever narrow it.
    allow_any: bool,
    /// Largest tolerated `|attested_at - now|`, in seconds. A drift signal,
    /// not a freshness guarantee.
    max_clock_drift_seconds: u64,
}

/// Create a gate.
///
/// * `vk_bytes` — Arkworks canonical compressed verifying key (360 bytes for
///   this circuit: 264 + 3 scalars of 32).
/// * `allowlist` — IDi values to accept. Pass an empty vector to accept any
///   card; adding an entry later permanently narrows the gate to that list.
/// * `max_clock_drift_seconds` — bound for the coarse `attested_at` check.
public fun create<Key: drop>(
    vk_bytes: vector<u8>,
    allowlist: vector<vector<u8>>,
    max_clock_drift_seconds: u64,
    ctx: &mut TxContext,
): Gate<Key> {
    Gate<Key> {
        id: object::new(ctx),
        verifier_key: zk_verifier::key(vk_bytes),
        seen: table::new(ctx),
        allow_any: allowlist.is_empty(),
        allowlist,
        max_clock_drift_seconds,
    }
}

/// Verify `att`, consume its `r1`, and return the claimed IDi.
///
/// Order matters and is not incidental: the drift check and the allowlist run
/// *before* the proof, so an unauthorised caller cannot use this call as a
/// free pairing oracle, and the expensive pairing runs only for a claim that
/// would be accepted anyway. The `r1` burn is last, so a rejected
/// presentation does not consume a challenge.
///
/// `presented_r1` must be the challenge the verifier issued for this
/// presentation. The gate checks it against `pi1` and against its own dedup
/// store, which together mean: exactly one presentation per issued challenge.
public fun verify_and_claim<Key: drop>(
    gate: &mut Gate<Key>,
    att: &Attestation,
    presented_r1: vector<u8>,
    now_seconds: u64,
): vector<u8> {
    felica_auth::assert_clock_drift_ok(att, now_seconds, gate.max_clock_drift_seconds);

    let claimed_idi = felica_auth::idi(att);
    assert!(admits(gate, &claimed_idi), ENotAllowed);

    // The holder's challenge must be the one the proof is bound to, and must
    // not have been spent. Checking the proof binding here means a replay of
    // another holder's session aborts even if the IDi is allowed.
    felica_auth::assert_challenge_matches(att, presented_r1);
    assert!(!table::contains(&gate.seen, presented_r1), EReplay);

    // Proof last, and only for a claim that is otherwise acceptable.
    let verified_idi = zk_verifier::verify(&gate.verifier_key, att);

    table::add(&mut gate.seen, presented_r1, true);
    verified_idi
}

/// Add an IDi to the allowlist, and narrow the gate to exactly the list.
///
/// Clearing `allow_any` here is the point: once a deployment has expressed a
/// preference, "no preference" is no longer reachable.
public fun allow<Key: drop>(gate: &mut Gate<Key>, idi: vector<u8>) {
    gate.allow_any = false;
    gate.allowlist.push_back(idi);
}

/// Remove an IDi from the allowlist.
///
/// Aborts if the IDi was not listed, rather than silently succeeding: a
/// `disallow` that quietly does nothing is a revocation that reads as applied.
/// Removing the last entry leaves the gate closed, not open — see `allow_any`.
public fun disallow<Key: drop>(gate: &mut Gate<Key>, idi: vector<u8>) {
    let mut i = 0;
    while (i < gate.allowlist.length()) {
        if (gate.allowlist[i] == idi) {
            gate.allowlist.remove(i);
            return
        };
        i = i + 1;
    };
    abort EUnknownIdi
}

/// Has this `r1` already been accepted?
public fun is_seen<Key: drop>(gate: &Gate<Key>, r1: vector<u8>): bool {
    table::contains(&gate.seen, r1)
}

/// Is `idi` admitted by this gate?
///
/// Reads the gate's own policy, which is what callers usually want.
public fun is_idi_allowed<Key: drop>(gate: &Gate<Key>, idi: vector<u8>): bool {
    admits(gate, &idi)
}

/// Does this gate admit any IDi at all?
public fun is_open<Key: drop>(gate: &Gate<Key>): bool { gate.allow_any }

/// The admission rule: open gates admit everything, closed gates admit only
/// what is listed.
fun admits<Key: drop>(gate: &Gate<Key>, idi: &vector<u8>): bool {
    if (gate.allow_any) return true;
    let mut i = 0;
    while (i < gate.allowlist.length()) {
        if (&gate.allowlist[i] == idi) return true;
        i = i + 1;
    };
    false
}
