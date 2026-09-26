/// Tests for the SuiCash transaction surface: the shared-gate topology, the
/// Clock-fed drift check, the single-spend `r1` burn, the receipt event, and
/// the OpenZeppelin access control on every admin path.
///
/// The cryptographic negatives live in `zk_verifier_tests`; these tests pin
/// what `suicash_gate` adds on top, against the same real fixture proof.
#[test_only]
module felica_oracle::suicash_gate_tests;

use felica_oracle::felica_auth;
use felica_oracle::fixture;
use felica_oracle::gate::{Self, Gate};
use felica_oracle::suicash_gate::{Self, SUICASH, SUICASH_GATE, Operator, AttestationVerified};
use openzeppelin_access::access_control::{Self, AccessControl, Auth};
use sui::clock;
use sui::event;
use sui::test_scenario;

const HOLDER: address = @0xA11CE;
const OPERATOR_ADDR: address = @0xBEA7;
const STRANGER: address = @0x57A6E;
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

/// Stand up the protocol and share one open gate, as the publisher would:
/// `init` (root + operator for the publisher) then a root-gated `create_gate`.
fun setup_open_gate(ts: &mut test_scenario::Scenario) {
    suicash_gate::init_for_testing(ts.ctx());
    ts.next_tx(HOLDER);
    let ac = ts.take_shared<AccessControl<SUICASH_GATE>>();
    let root = suicash_gate::new_root_auth(&ac, ts.ctx());
    suicash_gate::create_gate(&root, fixture::vk(), NO_ALLOWLIST, DRIFT_5MIN, ts.ctx());
    test_scenario::return_shared(ac);
    ts.next_tx(HOLDER);
}

#[test]
fun genuine_attestation_passes_and_emits_receipt() {
    let mut ts = test_scenario::begin(HOLDER);
    setup_open_gate(&mut ts);

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
    setup_open_gate(&mut ts);

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
    setup_open_gate(&mut ts);

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

/// The root admin can delegate allowlist management: a granted operator
/// narrows the gate to the fixture card, a stranger's card stops verifying,
/// and removing the entry closes the gate entirely.
#[test]
fun operator_manages_the_allowlist() {
    let mut ts = test_scenario::begin(HOLDER);
    setup_open_gate(&mut ts);

    // Root grants `Operator` to a dedicated address.
    let mut ac = ts.take_shared<AccessControl<SUICASH_GATE>>();
    access_control::grant_role<_, Operator>(&mut ac, OPERATOR_ADDR, ts.ctx());
    test_scenario::return_shared(ac);

    // The operator narrows the open gate to exactly the fixture card.
    ts.next_tx(OPERATOR_ADDR);
    let mut g = ts.take_shared<Gate<SUICASH>>();
    let ac = ts.take_shared<AccessControl<SUICASH_GATE>>();
    let op = suicash_gate::new_operator_auth(&ac, ts.ctx());
    suicash_gate::allow(&mut g, &op, fixture::idi());
    assert!(!gate::is_open(&g));
    assert!(gate::is_idi_allowed(&g, fixture::idi()));
    test_scenario::return_shared(g);
    test_scenario::return_shared(ac);

    // The allowed card still pays.
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
    clk.destroy_for_testing();
    test_scenario::return_shared(g);

    // The operator revokes the card; the gate admits nothing now, and stays
    // closed rather than reopening (narrowing is irreversible).
    ts.next_tx(OPERATOR_ADDR);
    let mut g = ts.take_shared<Gate<SUICASH>>();
    let ac = ts.take_shared<AccessControl<SUICASH_GATE>>();
    let op = suicash_gate::new_operator_auth(&ac, ts.ctx());
    suicash_gate::disallow(&mut g, &op, fixture::idi());
    assert!(!gate::is_open(&g));
    assert!(!gate::is_idi_allowed(&g, fixture::idi()));
    test_scenario::return_shared(g);
    test_scenario::return_shared(ac);
    ts.end();
}

/// The operator can retune the drift bound without republishing: shrinking it
/// below the attestation's age makes the same proof stale.
#[test]
#[expected_failure(abort_code = 4)] // felica_auth::EClockDrift
fun operator_retunes_the_drift_bound() {
    let mut ts = test_scenario::begin(HOLDER);
    setup_open_gate(&mut ts);

    ts.next_tx(HOLDER);
    let mut g = ts.take_shared<Gate<SUICASH>>();
    let ac = ts.take_shared<AccessControl<SUICASH_GATE>>();
    // The publisher is an operator from `init`, so it can retune directly.
    let op: Auth<Operator> = access_control::new_auth(&ac, ts.ctx());
    assert!(suicash_gate::admin_delay_ms() == 24 * 60 * 60 * 1_000);
    assert!(gate::max_clock_drift_seconds(&g) == DRIFT_5MIN);
    suicash_gate::set_max_clock_drift_seconds(&mut g, &op, 0);
    assert!(gate::max_clock_drift_seconds(&g) == 0);
    test_scenario::return_shared(g);
    test_scenario::return_shared(ac);

    // One second past the attested second is now beyond policy.
    ts.next_tx(HOLDER);
    let mut g = ts.take_shared<Gate<SUICASH>>();
    let mut clk = clock::create_for_testing(ts.ctx());
    clk.set_for_testing((fixture::attested_at() + 1) * 1000);
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

/// A stranger holds no role, so minting an `Operator` proof aborts in
/// OpenZeppelin's `access_control` — there is no admin path that does not go
/// through that mint.
#[test]
#[expected_failure(abort_code = access_control::EUnauthorized)]
fun stranger_cannot_mint_operator_auth() {
    let mut ts = test_scenario::begin(HOLDER);
    setup_open_gate(&mut ts);

    ts.next_tx(STRANGER);
    let ac = ts.take_shared<AccessControl<SUICASH_GATE>>();
    let _op: Auth<Operator> = access_control::new_auth(&ac, ts.ctx());
    abort 99 // unreachable
}

/// Gate creation is root-gated: an operator (but not root) cannot mint the
/// root proof that `create_gate` requires, so rogue verifying keys cannot be
/// pinned by anyone but the deployment owner.
#[test]
#[expected_failure(abort_code = access_control::EUnauthorized)]
fun operator_cannot_mint_root_auth() {
    let mut ts = test_scenario::begin(HOLDER);
    setup_open_gate(&mut ts);

    let mut ac = ts.take_shared<AccessControl<SUICASH_GATE>>();
    access_control::grant_role<_, Operator>(&mut ac, OPERATOR_ADDR, ts.ctx());
    test_scenario::return_shared(ac);

    ts.next_tx(OPERATOR_ADDR);
    let ac = ts.take_shared<AccessControl<SUICASH_GATE>>();
    let _root: Auth<SUICASH_GATE> = access_control::new_auth(&ac, ts.ctx());
    abort 99 // unreachable
}
