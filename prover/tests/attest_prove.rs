//! Session vectors minted entirely through felica-rs public API.
//!
//! A logging test driver (`EmuDriver`, felica-rs's own `emulator/tests`
//! pattern) wires the `FelicaStandard` reader straight against the card
//! emulator: polling → mutual_authentication → secure read. No DES/crypto
//! mirror lives here — oracle-side crypto is the prover's job (`src/`),
//! card-side is the emulator's.
//!
//! Property tests (roundtrip, packing pins, negatives, budget) use the same
//! fixture card with a fixed holder challenge (`R1`), so every public-input
//! limb can be pinned byte-for-byte. Fixture code lives in `common`.
//!
//! The read is exercised here only to confirm the card fixture still works
//! end to end; the IDi-level statement no longer covers the Read response,
//! so nothing about it is proved.

mod common;

use common::*;
use prover::{blank_constraint_counts, verify_attestation, ProverError, PUBLIC_INPUT_ORDER};

#[test]
fn polling_finds_card() {
    let (mut emu, _, _) = setup_card();
    let mut drv = EmuDriver {
        emu: &mut emu,
        log: Vec::new(),
    };
    let (_, poll) =
        felica::felica_standard::FelicaStandard::polling(&mut drv, "212F", SYSTEM_CODE, 0x00, 0x00)
            .expect("polling");
    assert_eq!(poll.idm, IDM);
}

#[test]
fn mutual_authentication_returns_idi() {
    let (_, _, _, idi) = auth_log();
    assert_eq!(idi, IDI);
}

#[test]
fn secure_read_returns_block() {
    let (_, _, _, blocks) = read_log();
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0], BLOCK);
}

#[test]
fn proves_genuine_session() {
    let (log, gsk, usk, _) = read_log();
    let (c1b, c2a, auth2) = auth_vectors(&log);

    let att = prover::prove(test_key(), &prove_req(c1b, c2a, auth2, gsk, usk))
        .expect("prove certifies genuine session");
    assert_eq!(att.idi, IDI, "circuit certifies IDi");
    assert_eq!(att.attested_at, ATTESTED_AT);
    assert_eq!(att.proof.public_inputs.len(), 3);
}

#[test]
fn roundtrip_and_packing_pins() {
    let (c1b, c2a, auth2, gsk, usk) = mint_fixed(&R1);
    let att = prover::prove(test_key(), &prove_req(c1b, c2a, auth2, gsk, usk))
        .expect("genuine session proves");
    assert_eq!(att.idi, IDI);
    assert_eq!(att.attested_at, ATTESTED_AT);
    assert!(
        verify_attestation(&test_key().vk, &att),
        "fresh proof verifies"
    );

    // 3-limb Sui packing, pinned byte-for-byte (all limbs < r, so LE
    // canonical round-trips exactly).
    assert_eq!(att.proof.public_inputs.len(), PUBLIC_INPUT_ORDER.len());
    let pi: Vec<Vec<u8>> = att
        .proof
        .public_inputs
        .iter()
        .map(|s| hex::decode(s).unwrap())
        .collect();
    for (i, raw) in pi.iter().enumerate() {
        assert_eq!(raw.len(), 32, "limb {i} is one 32-byte LE field element");
    }
    assert_eq!(&pi[0][..8], &IDI, "pi0 = idi");
    assert_eq!(&pi[1][..8], &R1, "pi1 = r1");
    assert_eq!(
        u64::from_le_bytes(pi[2][..8].try_into().unwrap()),
        ATTESTED_AT,
        "pi2 = attested_at"
    );
    // The transcript itself must NOT be public any more.
    assert_ne!(&pi[0][..8], &c1b, "c1b is private");
    assert_ne!(&pi[0][..8], &c2a, "c2a is private");
    assert_ne!(&pi[1][..8], &c1b, "r1 differs from c1b");
}

#[test]
fn tampered_public_input_fails_verify() {
    let (c1b, c2a, auth2, gsk, usk) = mint_fixed(&R1);
    let mut att = prover::prove(test_key(), &prove_req(c1b, c2a, auth2, gsk, usk))
        .expect("genuine session proves");
    // Flip the lowest nibble of pi0 (idi): stays valid hex/Fr, breaks pairing.
    let mut s = att.proof.public_inputs[0].clone();
    let last = s.pop().unwrap();
    s.push(if last == '0' { '1' } else { '0' });
    att.proof.public_inputs[0] = s;
    assert!(
        !verify_attestation(&test_key().vk, &att),
        "tampered pi must not verify"
    );
}

#[test]
fn tampered_proof_bytes_fail_verify() {
    let (c1b, c2a, auth2, gsk, usk) = mint_fixed(&R1);
    let mut att = prover::prove(test_key(), &prove_req(c1b, c2a, auth2, gsk, usk))
        .expect("genuine session proves");
    let mut s = att.proof.a.0.clone();
    let last = s.pop().unwrap();
    s.push(if last == '0' { '1' } else { '0' });
    att.proof.a.0 = s;
    assert!(
        !verify_attestation(&test_key().vk, &att),
        "tampered proof must not verify"
    );
}

#[test]
fn rejects_tampered_auth2() {
    let (c1b, c2a, mut auth2, gsk, usk) = mint_fixed(&R1);
    auth2[0] ^= 0xFF;
    assert_eq!(
        prover::prove(test_key(), &prove_req(c1b, c2a, auth2, gsk, usk)),
        Err(ProverError::MacMismatch)
    );
}

#[test]
fn rejects_tid_mismatch() {
    let (_, c2a, auth2, gsk, usk) = mint_fixed(&R1);
    let (c1b_b, _, _, _, _) = mint_fixed(&R1B);
    assert_eq!(
        prover::prove(test_key(), &prove_req(c1b_b, c2a, auth2, gsk, usk)),
        Err(ProverError::TidMismatch)
    );
}

#[test]
fn rejects_tampered_c1b() {
    let (mut c1b, c2a, auth2, gsk, usk) = mint_fixed(&R1);
    c1b[0] ^= 0xFF;
    assert!(
        prover::prove(test_key(), &prove_req(c1b, c2a, auth2, gsk, usk)).is_err(),
        "forged c1b must not prove"
    );
}

/// The native pre-checks recover `r1` from `c1b` via the key schedule, so a
/// wrong `gsk` is caught before proving. This is the *API-level* view of D1;
/// `circuit_robust::derivation_d1_d3_bind` proves the circuit itself
/// constrains it, with the native pre-checks bypassed.
#[test]
fn rejects_wrong_master_keys() {
    let (c1b, c2a, auth2, gsk, usk) = mint_fixed(&R1);
    let mut wrong_gsk = gsk;
    wrong_gsk[0] ^= 0xFF;
    assert!(
        prover::prove(test_key(), &prove_req(c1b, c2a, auth2, wrong_gsk, usk)).is_err(),
        "a wrong gsk must not certify the session"
    );
    let mut wrong_usk = usk;
    wrong_usk[0] ^= 0xFF;
    assert!(
        prover::prove(test_key(), &prove_req(c1b, c2a, auth2, gsk, wrong_usk)).is_err(),
        "a wrong usk must not certify the session"
    );
}

#[test]
fn constraint_budget_is_pinned() {
    let (n_constraints, n_instance, n_witness) = blank_constraint_counts();
    // 3 public inputs + ONE.
    assert_eq!(
        n_instance,
        PUBLIC_INPUT_ORDER.len() + 1,
        "instance pins packing"
    );
    assert_eq!(PUBLIC_INPUT_ORDER.len(), 3);
    assert!(n_witness > 100_000, "witnesses: {n_witness}");
    // 15 DES gadgets (13 transcript + 2 for D2/D3) at ~10k each.
    assert!(
        n_constraints <= 200_000,
        "constraint budget blown: {n_constraints}"
    );
}
