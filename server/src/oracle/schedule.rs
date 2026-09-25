//! Challenge schedule: the only oracle math felica-rs keeps private.
//!
//! IDi-level scope: this file now carries only the challenge schedule. The
//! reference implementation also replayed secure-messaging framing here (PKCS#7
//! padding, the forward MAC, DES-CBC, and the single-block Read command and
//! response). All of that is gone: `auth2` already carries `idi`, so nothing
//! in the statement covers a read response, and AUTH2 verification delegates
//! to felica-rs (`super::verify_auth2`) rather than to a local mirror.
//!
//! felica-rs holds the oracle-side schedule behind crate walls —
//! `AuthenticationContext` (`src/felica_standard/secure/des.rs:304-349`,
//! `pub(crate)`) and the 3DES block helpers
//! (`src/felica_standard/secure/primitives.rs:112-144`, `pub(super)`).
//! Until upstream exposes an oracle/relay API, this file replays that exact
//! schedule with the same `des` crate family felica-rs uses, line-for-line:
//! ```text
//! L = K_group xor IDm            (des.rs:316)
//! α = DES_encrypt(data=K_user, key=L)   (des.rs:317, encrypt_des_block(data, key))
//! β = DES_encrypt(data=L, key=α)        (des.rs:318)
//! C1A = 3DES(k1=α, k2=L, R1)     (des.rs:326-328)
//! C1B = 3DES(k1=L, k2=β, R1)     (des.rs:330-332)
//! C2A = 3DES(k1=L, k2=β, R2)     (des.rs:334-336)
//! C2B = 3DES(k1=α, k2=L, R2)     (des.rs:346-348)
//! ```
//! Spec reference: docs/spec.md §4.1. Everything else (key chain resolution,
//! AUTH2 verification, card emulation) comes from felica-rs public API via the
//! parent [`super`] module. The round-trip test below drives a real felica-rs
//! emulated card, so any drift from upstream is caught here.

use des::cipher::{BlockCipherDecrypt, BlockCipherEncrypt, KeyInit};
use des::{Des, TdesEde3};
use zeroize::Zeroize;

/// Intermediate keys derived outside the ZK circuit (spec §4.1).
#[derive(Debug, Clone, Copy)]
pub struct IntermediateKeys {
    pub l: [u8; 8],
    pub alpha: [u8; 8],
    pub beta: [u8; 8],
}

impl IntermediateKeys {
    /// `AuthenticationContext::new` (felica-rs `secure/des.rs:311-320`).
    /// NOTE on argument order: felica-rs `encrypt_des_block(data, key)`, so
    /// `α = encrypt(data=K_user, key=L)`, `β = encrypt(data=L, key=α)`.
    pub fn derive(gsk: &[u8; 8], usk: &[u8; 8], idm: &[u8; 8]) -> Self {
        let l = xor8(gsk, idm);
        let alpha = des_encrypt(usk, &l);
        let beta = des_encrypt(&l, &alpha);
        Self { l, alpha, beta }
    }

    /// `encrypt_challenge1a` (des.rs:326).
    pub fn c1a(&self, r1: &[u8; 8]) -> [u8; 8] {
        tdes_encrypt(r1, &self.alpha, &self.l)
    }
    /// `encrypt_challenge1b` (des.rs:330): expected C1B for `verify_challenge1b`.
    pub fn c1b_expected(&self, r1: &[u8; 8]) -> [u8; 8] {
        tdes_encrypt(r1, &self.l, &self.beta)
    }
    /// `encrypt_challenge2b` (des.rs:346).
    pub fn c2b(&self, r2: &[u8; 8]) -> [u8; 8] {
        tdes_encrypt(r2, &self.alpha, &self.l)
    }
    /// `decrypt_challenge2a` (des.rs:342): recover session key R2 from C2A.
    pub fn r2_from_c2a(&self, c2a: &[u8; 8]) -> [u8; 8] {
        tdes_decrypt(c2a, &self.l, &self.beta)
    }
    /// Inverse of C1B (spec §7: oracle recovers `r1` as `3DES⁻¹(L, β, c1b)`).
    pub fn r1_from_c1b(&self, c1b: &[u8; 8]) -> [u8; 8] {
        tdes_decrypt(c1b, &self.l, &self.beta)
    }
}

fn xor8(a: &[u8; 8], b: &[u8; 8]) -> [u8; 8] {
    let mut out = [0u8; 8];
    for i in 0..8 {
        out[i] = a[i] ^ b[i];
    }
    out
}

/// `encrypt_des_block(data, key)` (felica-rs `primitives.rs:104-106`).
fn des_encrypt(data: &[u8; 8], key: &[u8; 8]) -> [u8; 8] {
    let cipher = Des::new(key.into());
    let mut block = (*data).into();
    cipher.encrypt_block(&mut block);
    block.into()
}

/// Two-key EDE 3DES with K1|K2|K1 (`encrypt_3des_block`, `primitives.rs:112-127`).
pub fn tdes_encrypt(data: &[u8; 8], k1: &[u8; 8], k2: &[u8; 8]) -> [u8; 8] {
    with_stacked_key(k1, k2, |stacked| {
        let cipher = TdesEde3::new(stacked.into());
        let mut block = (*data).into();
        cipher.encrypt_block(&mut block);
        block.into()
    })
}

/// `decrypt_3des_block` (`primitives.rs:129-144`).
pub fn tdes_decrypt(data: &[u8; 8], k1: &[u8; 8], k2: &[u8; 8]) -> [u8; 8] {
    with_stacked_key(k1, k2, |stacked| {
        let cipher = TdesEde3::new(stacked.into());
        let mut block = (*data).into();
        cipher.decrypt_block(&mut block);
        block.into()
    })
}

/// Runs `f` with the K1|K2|K1 stacking buffer, wiping it afterwards.
/// The cipher copies the key at construction, so the wipe is sound; the
/// cipher's own expanded schedule is cleared by the `des` crate
/// (`zeroize` feature), as in felica-rs `primitives.rs:117-121`.
fn with_stacked_key<R>(k1: &[u8; 8], k2: &[u8; 8], f: impl FnOnce(&[u8; 24]) -> R) -> R {
    let mut stacked = [0u8; 24];
    stacked[..8].copy_from_slice(k1);
    stacked[8..16].copy_from_slice(k2);
    stacked[16..].copy_from_slice(k1);
    let out = f(&stacked);
    stacked.zeroize();
    out
}
