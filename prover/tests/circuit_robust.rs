//! Circuit robustness: every §7 constraint must actually constrain.
//!
//! `prove`-level tests cannot tell a circuit hole from a native rejection —
//! [`prove`](prover::prove) runs native pre-checks first. These tests
//! bypass them via [`check_satisfiable`](prover::check_satisfiable)
//! and feed adversarial witnesses straight into the constraint system:
//! each mutation below violates exactly one constraint's relation, so a
//! satisfiable outcome would mean that constraint is dead wiring.
//! The emulator is only a genuine-witness mint (`common`); nothing here
//! trusts card behavior.

mod common;

use common::*;
use prover::{check_satisfiable, des::tdes_encrypt, verify_attestation};

/// Deterministic xorshift64* — no extra deps for fuzz randomness.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

#[test]
fn blank_is_unsatisfiable() {
    // All-zero witnesses must not satisfy anything: 3DES(0,0,0) != 0.
    assert!(
        !check_satisfiable(prover::circuit::FelicaCircuit::blank()),
        "blank circuit must be unsatisfiable"
    );
}

#[test]
fn genuine_is_satisfiable() {
    assert!(
        check_satisfiable(genuine_circuit(&R1)),
        "genuine witnesses must satisfy"
    );
}

/// Constraint 1 binds alone: flipped `c1b` breaks nothing else.
#[test]
fn constraint1_c1b_binds() {
    let mut c = genuine_circuit(&R1);
    c.c1b[0] ^= 0xFF;
    assert!(!check_satisfiable(c), "c1b mismatch must be unsatisfiable");
}

/// Constraint 2 binds alone: flipped `c2a` breaks nothing else
/// (`r2` witness and AUTH2 path are untouched).
#[test]
fn constraint2_c2a_binds() {
    let mut c = genuine_circuit(&R1);
    c.c2a[0] ^= 0xFF;
    assert!(!check_satisfiable(c), "c2a mismatch must be unsatisfiable");
}

/// Constraints 3+4 bind: flipped AUTH2 block diffuses through CBC+MAC.
#[test]
fn constraint34_auth2_binds() {
    for byte in [0, 8, 16, 24] {
        let mut c = genuine_circuit(&R1);
        c.auth2[byte] ^= 0xFF;
        assert!(
            !check_satisfiable(c),
            "auth2[{byte}] forgery must be unsatisfiable"
        );
    }
}

/// Constraint 5 binds in isolation: re-key `c1b` for a mutated `r1`, so
/// constraint 1 still holds and only the TID relation breaks.
#[test]
fn constraint5_tid_binds() {
    let mut c = genuine_circuit(&R1);
    c.r1[2] ^= 0xFF;
    // Recompute c1b for the mutated r1 (same card keys — the fixture card
    // is deterministic): constraint 1 holds by construction.
    let (_, gsk, usk) = setup_card();
    let (l, _, beta) = session_keys(&gsk, &usk);
    c.c1b = tdes_encrypt(&c.r1, &l, &beta);
    assert!(!check_satisfiable(c), "tid mismatch must be unsatisfiable");
}

/// Constraint 6 binds alone: flipped `idi` breaks nothing else
/// (`idi` feeds only the IDi equality and its own packing limb).
#[test]
fn constraint6_idi_binds() {
    let mut c = genuine_circuit(&R1);
    c.idi[0] ^= 0xFF;
    assert!(!check_satisfiable(c), "idi mismatch must be unsatisfiable");
}

/// `attested_at` is a free echoed scalar: flipping it stays satisfiable —
/// but the public inputs differ, which is what binds the proof to the
/// timestamp in the pairing equation. `idi` and `r1` are *not* free; see
/// `constraint5_tid_binds` / `constraint6_idi_binds`.
#[test]
fn free_variables_bind_via_public_inputs() {
    let c = genuine_circuit(&R1);
    let pi_before = c.public_inputs().to_fr().unwrap();
    let mut d = c.clone();
    d.attested_at ^= 0xFF;
    assert!(
        check_satisfiable(d.clone()),
        "attested_at is free by design"
    );
    let pi_after = d.public_inputs().to_fr().unwrap();
    assert_ne!(pi_before, pi_after, "attested_at moves the public inputs");
}

/// D1–D3 bind the FeliCa key schedule. `l` is no longer a witness field, so
/// there is nothing to tamper with directly: instead, perturbing any of the
/// derivation *inputs* must break the transcript relations, because `l`,
/// `alpha` and `beta` all follow from them.
///
/// This is the property the IDi-level revision actually added. Note what it
/// is *not*: the keys are still unconstrained, so a forger can supply a
/// self-consistent invented triple. See `tests/forgery_authority.rs`.
#[test]
fn derivation_d1_d3_bind() {
    let c = genuine_circuit(&R1);

    // D1: l = gsk XOR idm. Flip either side; the derived l changes, so C1/C2
    // no longer hold for the same c1b/c2a.
    let mut d = c.clone();
    d.gsk[0] ^= 0x01;
    assert!(!check_satisfiable(d), "D1: gsk must bind");
    let mut d = c.clone();
    d.idm[0] ^= 0x01;
    assert!(!check_satisfiable(d), "D1: idm must bind");

    // D2: alpha = DES_l(usk). Flipping usk changes alpha, and therefore beta.
    let mut d = c.clone();
    d.usk[0] ^= 0x01;
    assert!(!check_satisfiable(d), "D2: usk must bind");

    // Control: a witness whose derivation inputs are unchanged must still
    // satisfy D1–D3. This proves the failures above come from the derivation
    // and not from some unrelated constraint rejecting the base witness.
    let mut d = c.clone();
    d.usk[3] ^= 0x01;
    d.usk[3] ^= 0x01;
    assert!(
        check_satisfiable(d),
        "an unchanged witness must stay satisfiable"
    );
}

/// Parity bits are free *in key position* — PC-1 drops bit 0 of every key
/// byte — but the IDi-level revision made the key material load-bearing
/// anyway, because D2 and D3 use it as DES **data**:
///
/// ```text
/// D2: alpha = DES_key(l)_data(usk)   <- usk is data
/// D3: beta  = DES_key(alpha)_data(l) <- l is data
/// ```
///
/// So flipping bit 0 of `usk` changes `alpha`, hence `beta`, hence C1/C2. And
/// flipping bit 0 of `gsk` changes `l` (D1 is a bare XOR, no PC-1), which is
/// D3's data. Both now bind, where in the previous design `l`/`beta` only ever
/// appeared in key position and a parity flip was free.
///
/// This is *stricter*, not a soundness regression. The underlying DES property
/// still holds and is pinned natively below.
#[test]
fn parity_bits_are_load_bearing_on_key_schedule_inputs() {
    let c = genuine_circuit(&R1);
    let (l, _, beta) = session_keys(&c.gsk, &c.usk);

    // Native, in key position: PC-1 drops the parity bit, so these keys are
    // equivalent. Pinned so the reasoning above stays grounded.
    for i in 0..8 {
        let mut lf = l;
        lf[i] ^= 0x01;
        assert_eq!(tdes_encrypt(&c.r1, &lf, &beta), c.c1b);
        let mut bf = beta;
        bf[i] ^= 0x01;
        assert_eq!(tdes_encrypt(&c.r1, &l, &bf), c.c1b);
    }

    // In the circuit the derivation inputs are DES data, so no parity slack.
    let mut d = c.clone();
    d.usk[2] ^= 0x01;
    assert!(
        !check_satisfiable(d),
        "a usk parity flip changes alpha, so it must bind"
    );
    let mut d = c.clone();
    d.gsk[4] ^= 0x01;
    assert!(
        !check_satisfiable(d),
        "a gsk parity flip changes l, which is D3's data, so it must bind"
    );
}

/// Fuzz: every constrained witness byte-array is mutated dozens of times at
/// deterministic pseudorandom positions; all outcomes must be unsatisfiable.
/// `attested_at` is excluded — free by design, covered above.
/// Every other region is fuzzed across *all* 8 bits, including bit 0. The
/// previous design had to skip bit 0 on `l`/`beta` as DES parity bits; the
/// IDi-level revision removed that exemption, because `gsk`/`usk` reach the
/// transcript as D2/D3 *data* (see
/// `parity_bits_are_load_bearing_on_key_schedule_inputs`).
#[test]
fn fuzz_mutations_never_satisfy() {
    let base = genuine_circuit(&R1);
    let mut rng = Rng(0x1234_5678_9ABC_DEF0);
    // (name, length, setter): flipping any constrained byte must break it.
    let mut hit = [false; 8];
    for step in 0..96 {
        let mut c = base.clone();
        let region = rng.below(8);
        let (field, idx, bit) = match region {
            0 => {
                let i = rng.below(8);
                let b = rng.below(8);
                c.r1[i] ^= 1 << b;
                hit[0] = true;
                ("r1", i, b)
            }
            1 => {
                let i = rng.below(8);
                let b = rng.below(8);
                c.c1b[i] ^= 1 << b;
                hit[1] = true;
                ("c1b", i, b)
            }
            2 => {
                let i = rng.below(8);
                let b = rng.below(8);
                c.c2a[i] ^= 1 << b;
                hit[2] = true;
                ("c2a", i, b)
            }
            3 => {
                let i = rng.below(32);
                let b = rng.below(8);
                c.auth2[i] ^= 1 << b;
                hit[3] = true;
                ("auth2", i, b)
            }
            4 => {
                let i = rng.below(8);
                let b = rng.below(8);
                c.idi[i] ^= 1 << b;
                hit[4] = true;
                ("idi", i, b)
            }
            5 => {
                let i = rng.below(8);
                let b = rng.below(8);
                c.r2[i] ^= 1 << b;
                hit[5] = true;
                ("r2", i, b)
            }
            6 => {
                let i = rng.below(8);
                let b = rng.below(8);
                c.gsk[i] ^= 1 << b;
                hit[6] = true;
                ("gsk", i, b)
            }
            _ => {
                let i = rng.below(8);
                let b = rng.below(8);
                c.usk[i] ^= 1 << b;
                hit[7] = true;
                ("usk", i, b)
            }
        };
        assert!(
            !check_satisfiable(c),
            "fuzz step {step}: {field}[{idx}] bit {bit} must be unsatisfiable"
        );
    }
    assert!(
        hit.iter().all(|h| *h),
        "fuzz must cover all witness regions"
    );
}

/// `verify_attestation` never panics and rejects garbage: empty strings,
/// non-hex, wrong lengths, and off-modulus field encodings (e.g. 32 bytes
/// of `0xff`, which exceeds `r`).
#[test]
fn verify_rejects_garbage() {
    let (c1b, c2a, auth2, gsk, usk) = mint_fixed(&R1);
    let base = prover::prove(test_key(), &prove_req(c1b, c2a, auth2, gsk, usk))
        .expect("genuine session proves");
    assert!(verify_attestation(&test_key().vk, &base));
    for tamper in ["zz", "", "00", &"ff".repeat(31), &"ff".repeat(33)] {
        let mut bad = base.clone();
        bad.proof.a.0 = tamper.to_string();
        assert!(
            !verify_attestation(&test_key().vk, &bad),
            "garbage a.x rejected"
        );
        let mut bad = base.clone();
        bad.proof.public_inputs[0] = tamper.to_string();
        assert!(
            !verify_attestation(&test_key().vk, &bad),
            "garbage pi rejected"
        );
    }
    let mut bad = base.clone();
    bad.proof.public_inputs.pop();
    assert!(
        !verify_attestation(&test_key().vk, &bad),
        "short pi rejected"
    );
    let mut bad = base.clone();
    bad.proof.public_inputs.push("00".repeat(32));
    assert!(
        !verify_attestation(&test_key().vk, &bad),
        "long pi rejected"
    );
}
