/// Tests for the SuiCash transaction surface: the shared-gate topology, the
/// Clock-fed drift check, the single-spend `r1` burn, and the receipt event.
///
/// The cryptographic negatives live in `zk_verifier_tests`; these tests pin
/// what `suicash_gate` adds on top, against the same real fixture proof.
#[test_only]
module felica_oracle::suicash_gate_tests;

use felica_oracle::felica_auth;
use felica_oracle::fixture;
use felica_oracle::gate::{Self, Gate};
use felica_oracle::suicash_gate::{Self, SUICASH, AttestationVerified};
use sui::clock;
use sui::event;
use sui::test_scenario;

const HOLDER: address = @0xA11CE;
const NO_ALLOWLIST: vector<vector<u8>> = vector[];
const DRIFT_5MIN: u64 = 300;

/// The `r1` the fixture proof commits to (`pi1`).
fun fixture_r1(): vector<u8> {
    let att = felica_auth::new(
        b"groth16-bn254",
        fixture::idi(),
        fixture::attested_at(),
        fixture::proof(),
        fixture::public_inputs(),
    );
    felica_auth::r1(&att)
}

/// A clock reading `attested_at` exactly — zero drift.
fun clock_at_attestation(ctx: &mut TxContext): clock::Clock {
    let mut clk = clock::create_for_testing(ctx);
    clk.set_for_testing(fixture::attested_at() * 1000);
    clk
}

#[test]
fun genuine_attestation_passes_and_emits_receipt() {
    let mut ts = test_scenario::begin(HOLDER);
    suicash_gate::create_gate(fixture::vk(), NO_ALLOWLIST, DRIFT_5MIN, ts.ctx());
    ts.next_tx(HOLDER);

    let mut g = ts.take_shared<Gate<SUICASH>>();
    let clk = clock_at_attestation(ts.ctx());
    suicash_gate::verify(
        &mut g,
        fixture::idi(),
        fixture::attested_at(),
        fixture::proof(),
        fixture::public_inputs(),
        fixture_r1(),
        &clk,
        ts.ctx(),
    );

    // One receipt, and the challenge is burned in the shared store.
    assert!(event::events_by_type<AttestationVerified>().length() == 1);
    assert!(gate::is_seen(&g, fixture_r1()));

    clk.destroy_for_testing();
    test_scenario::return_shared(g);
    ts.end();
}

/// The same attestation cannot be spent twice: the second `verify` must hit
/// the gate's dedup store, which is the property that makes a payment PTB
/// single-shot per card session.
#[test]
#[expected_failure(abort_code = 1)] // gate::EReplay
fun replayed_attestation_is_rejected() {
    let mut ts = test_scenario::begin(HOLDER);
    suicash_gate::create_gate(fixture::vk(), NO_ALLOWLIST, DRIFT_5MIN, ts.ctx());
    ts.next_tx(HOLDER);

    let mut g = ts.take_shared<Gate<SUICASH>>();
    let clk = clock_at_attestation(ts.ctx());
    suicash_gate::verify(
        &mut g,
        fixture::idi(),
        fixture::attested_at(),
        fixture::proof(),
        fixture::public_inputs(),
        fixture_r1(),
        &clk,
        ts.ctx(),
    );
    suicash_gate::verify(
        &mut g,
        fixture::idi(),
        fixture::attested_at(),
        fixture::proof(),
        fixture::public_inputs(),
        fixture_r1(),
        &clk,
        ts.ctx(),
    );
    abort 99 // unreachable
}

/// The drift bound is fed from the on-chain Clock, not a caller-chosen
/// number: a stale attestation must abort once the chain's clock has moved
/// past the tolerance.
#[test]
#[expected_failure(abort_code = 4)] // felica_auth::EClockDrift
fun stale_attestation_is_rejected() {
    let mut ts = test_scenario::begin(HOLDER);
    suicash_gate::create_gate(fixture::vk(), NO_ALLOWLIST, DRIFT_5MIN, ts.ctx());
    ts.next_tx(HOLDER);

    let mut g = ts.take_shared<Gate<SUICASH>>();
    let mut clk = clock::create_for_testing(ts.ctx());
    clk.set_for_testing((fixture::attested_at() + DRIFT_5MIN + 1) * 1000);
    suicash_gate::verify(
        &mut g,
        fixture::idi(),
        fixture::attested_at(),
        fixture::proof(),
        fixture::public_inputs(),
        fixture_r1(),
        &clk,
        ts.ctx(),
    );
    abort 99 // unreachable
}
