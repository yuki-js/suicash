//! R1CS gadgets for DES / 3DES-EDE / CBC / MAC (spec §7 constraints 1–6).
//!
//! Table source of truth lives in [`crate::des`]; this module only wires
//! those tables into constraints. S-boxes are enforced by degree-63
//! interpolation polynomials over BN254 Fr (one per box, precomputed once):
//! `out == P(in)` with `in`/`out` bound to 6/4-bit decompositions.

use std::sync::LazyLock;

use ark_bn254::Fr;
use ark_ff::{Field, Zero};
use ark_r1cs_std::fields::FieldVar;
use ark_r1cs_std::{alloc::AllocVar, boolean::Boolean, eq::EqGadget, fields::fp::FpVar};
use ark_relations::r1cs::{ConstraintSystemRef, SynthesisError};

use crate::abi;
use crate::des::{sbox_lookup, E, IP, P, PC1, PC2, SHIFTS};

/// Interpolation of the binary-indexed S-box table (`sbox_mapped`).
fn interpolate(table: &[u8; 64]) -> Vec<Fr> {
    let n = 64usize;
    let mut mat: Vec<Vec<Fr>> = vec![vec![Fr::from(0u64); n + 1]; n];
    for i in 0..n {
        let x = Fr::from(i as u64);
        let mut pow = Fr::from(1u64);
        for j in 0..n {
            mat[i][j] = pow;
            pow *= x;
        }
        mat[i][n] = Fr::from(table[i] as u64);
    }
    for col in 0..n {
        let mut pivot = col;
        while pivot < n && mat[pivot][col].is_zero() {
            pivot += 1;
        }
        assert!(pivot < n, "vandermonde invertible");
        mat.swap(col, pivot);
        let inv = mat[col][col].inverse().expect("nonzero pivot");
        for j in col..=n {
            mat[col][j] *= inv;
        }
        for row in 0..n {
            if row != col {
                let factor = mat[row][col];
                if !factor.is_zero() {
                    for j in col..=n {
                        let sub = mat[col][j] * factor;
                        mat[row][j] -= sub;
                    }
                }
            }
        }
    }
    (0..n).map(|i| mat[i][n]).collect()
}

fn mapped_table(box_idx: usize) -> [u8; 64] {
    let mut out = [0u8; 64];
    for v in 0..64u8 {
        let bits = [
            (v >> 5) & 1 == 1,
            (v >> 4) & 1 == 1,
            (v >> 3) & 1 == 1,
            (v >> 2) & 1 == 1,
            (v >> 1) & 1 == 1,
            v & 1 == 1,
        ];
        out[v as usize] = sbox_lookup(box_idx, bits);
    }
    out
}

static SBOX_POLYS: LazyLock<Vec<Vec<Fr>>> =
    LazyLock::new(|| (0..8).map(|b| interpolate(&mapped_table(b))).collect());

fn eval_poly(in_var: &FpVar<Fr>, coeffs: &[Fr]) -> FpVar<Fr> {
    // Horner from the top: acc = c63; acc = acc*in + c62; …
    let mut acc = FpVar::constant(coeffs[coeffs.len() - 1]);
    for c in coeffs[..coeffs.len() - 1].iter().rev() {
        acc = acc * in_var.clone() + *c;
    }
    acc
}

/// Allocate LE bits (bit j of byte i at index 8*i+j) as witnesses.
pub fn alloc_le_bits(
    cs: ConstraintSystemRef<Fr>,
    bytes: &[u8],
) -> Result<Vec<Boolean<Fr>>, SynthesisError> {
    bytes
        .iter()
        .flat_map(|b| (0..8).map(move |j| (b, j)))
        .map(|(b, j)| Boolean::new_witness(cs.clone(), || Ok(((b >> j) & 1) == 1)))
        .collect()
}

/// LE bits → DES-ordered bits (DES bit k = LE[8*(k/8)+7-(k%8)]). Wiring only.
pub fn le_to_des(le: &[Boolean<Fr>]) -> Vec<Boolean<Fr>> {
    assert_eq!(le.len() % 8, 0);
    let n = le.len();
    let mut out = Vec::with_capacity(n);
    for k in 0..n {
        let byte = k / 8;
        let idx = byte * 8 + (7 - (k % 8));
        out.push(le[idx].clone());
    }
    out
}

/// DES-ordered bits → LE bits (inverse wiring).
pub fn des_to_le(des: &[Boolean<Fr>]) -> Vec<Boolean<Fr>> {
    assert_eq!(des.len() % 8, 0);
    let n = des.len();
    let mut le = vec![Boolean::FALSE; n];
    for (k, bit) in des.iter().enumerate() {
        let idx = (k / 8) * 8 + (7 - (k % 8));
        le[idx] = bit.clone();
    }
    le
}

fn permute_vars(vars: &[Boolean<Fr>], table: &[usize]) -> Vec<Boolean<Fr>> {
    table.iter().map(|&j| vars[j].clone()).collect()
}

/// S-box gadget: 6 input bits (b0 first/MSB) → 4 output bits (o0 first/MSB).
/// Output witnesses carry native values for `witness_in`; constraints pin
/// them via `out == P(in)`.
pub fn sbox_gadget(
    cs: ConstraintSystemRef<Fr>,
    box_idx: usize,
    input: &[Boolean<Fr>],
    witness_in: [bool; 6],
) -> Result<Vec<Boolean<Fr>>, SynthesisError> {
    assert_eq!(input.len(), 6);
    let out_val = sbox_lookup(box_idx, witness_in);
    let out_bits_val = [
        ((out_val >> 3) & 1) == 1,
        ((out_val >> 2) & 1) == 1,
        ((out_val >> 1) & 1) == 1,
        (out_val & 1) == 1,
    ];
    let out: Vec<Boolean<Fr>> = out_bits_val
        .iter()
        .map(|b| Boolean::new_witness(cs.clone(), || Ok(*b)))
        .collect::<Result<_, _>>()?;
    // in = b0*32+…+b5, out = o0*8+…+o3 as field elements.
    let in_fp = Boolean::le_bits_to_fp(&[
        input[5].clone(),
        input[4].clone(),
        input[3].clone(),
        input[2].clone(),
        input[1].clone(),
        input[0].clone(),
    ])?;
    let out_fp = Boolean::le_bits_to_fp(&[
        out[3].clone(),
        out[2].clone(),
        out[1].clone(),
        out[0].clone(),
    ])?;
    let expected = eval_poly(&in_fp, &SBOX_POLYS[box_idx]);
    expected.enforce_equal(&out_fp)?;
    Ok(out)
}

/// Round keys (16×48) as wirings over the key's LE bits.
fn round_key_vars(key_le: &[Boolean<Fr>]) -> Vec<Vec<Boolean<Fr>>> {
    assert_eq!(key_le.len(), 64);
    let key_des = le_to_des(key_le);
    let pc1 = permute_vars(&key_des, &PC1);
    let mut c: Vec<Boolean<Fr>> = pc1[..28].to_vec();
    let mut d: Vec<Boolean<Fr>> = pc1[28..].to_vec();
    let mut out = Vec::with_capacity(16);
    for s in SHIFTS.iter() {
        c.rotate_left(*s);
        d.rotate_left(*s);
        let mut cd = Vec::with_capacity(56);
        cd.extend_from_slice(&c);
        cd.extend_from_slice(&d);
        out.push(permute_vars(&cd, &PC2));
    }
    out
}

/// Single DES block gadget. `data_des` is 64 DES-ordered bits, `key_le` is
/// 64 LE key bits, `witness_data`/`witness_key` are native bytes for hints.
#[allow(clippy::too_many_arguments)]
pub fn des_gadget(
    cs: ConstraintSystemRef<Fr>,
    data_des: &[Boolean<Fr>],
    key_le: &[Boolean<Fr>],
    encrypt: bool,
    witness_data: &[u8; 8],
    witness_key: &[u8; 8],
) -> Result<Vec<Boolean<Fr>>, SynthesisError> {
    assert_eq!(data_des.len(), 64);
    assert_eq!(key_le.len(), 64);
    let mut rkeys = round_key_vars(key_le);
    if !encrypt {
        rkeys.reverse();
    }
    // Native hint for intermediate round values (to allocate XOR/S-box outputs).
    let native_keys: [[bool; 48]; 16] = {
        use crate::des::{bytes_to_des_bits, round_keys};
        let rk = round_keys(witness_key);
        let mut arr = [[false; 48]; 16];
        for (i, k) in rk.iter().enumerate() {
            arr[i] = *k;
        }
        let _ = bytes_to_des_bits(witness_data);
        arr
    };
    let _ = native_keys;

    let ip = permute_vars(data_des, &IP);
    let mut l: Vec<Boolean<Fr>> = ip[..32].to_vec();
    let mut r: Vec<Boolean<Fr>> = ip[32..].to_vec();

    // Native simulation for witness hints.
    let mut l_native = {
        use crate::des::bytes_to_des_bits;
        let b = bytes_to_des_bits(witness_data);
        let ipn: Vec<bool> = IP.iter().map(|&j| b[j]).collect();
        ipn[..32].to_vec()
    };
    let mut r_native = {
        use crate::des::bytes_to_des_bits;
        let b = bytes_to_des_bits(witness_data);
        let ipn: Vec<bool> = IP.iter().map(|&j| b[j]).collect();
        ipn[32..].to_vec()
    };
    let mut c_native = {
        use crate::des::bytes_to_des_bits;
        let kb = bytes_to_des_bits(witness_key);
        let pc1: Vec<bool> = PC1.iter().map(|&j| kb[j]).collect();
        pc1
    };
    // Re-derive native round keys as bool vecs for hints.
    let native_rkeys: Vec<Vec<bool>> = {
        let mut c: Vec<bool> = c_native[..28].to_vec();
        let mut d: Vec<bool> = c_native[28..].to_vec();
        let mut out = Vec::new();
        for s in SHIFTS.iter() {
            c.rotate_left(*s);
            d.rotate_left(*s);
            let mut cd = Vec::with_capacity(56);
            cd.extend_from_slice(&c);
            cd.extend_from_slice(&d);
            out.push(PC2.iter().map(|&j| cd[j]).collect::<Vec<bool>>());
        }
        if !encrypt {
            out.reverse();
        }
        out
    };
    let _ = &mut c_native;

    for round in 0..16 {
        let rk = &rkeys[round];
        let rk_nat = &native_rkeys[round];
        // Expansion (wiring).
        let expanded = permute_vars(&r, &E);
        let expanded_nat: Vec<bool> = E.iter().map(|&j| r_native[j]).collect();
        // XOR with round key.
        let mut xored = Vec::with_capacity(48);
        let mut xored_nat = Vec::with_capacity(48);
        for i in 0..48 {
            let a = expanded[i].clone();
            let b = rk[i].clone();
            xored.push(a.clone() ^ b.clone());
            xored_nat.push(expanded_nat[i] ^ rk_nat[i]);
        }
        // S-boxes.
        let mut s_out = Vec::with_capacity(32);
        let mut s_out_nat = Vec::with_capacity(32);
        for b in 0..8 {
            let chunk: Vec<Boolean<Fr>> = xored[b * 6..b * 6 + 6].to_vec();
            let chunk_nat: [bool; 6] = xored_nat[b * 6..b * 6 + 6].try_into().unwrap();
            let o = sbox_gadget(cs.clone(), b, &chunk, chunk_nat)?;
            let v = sbox_lookup(b, chunk_nat);
            s_out.extend(o);
            s_out_nat.push(((v >> 3) & 1) == 1);
            s_out_nat.push(((v >> 2) & 1) == 1);
            s_out_nat.push(((v >> 1) & 1) == 1);
            s_out_nat.push((v & 1) == 1);
        }
        // P permutation (wiring).
        let f = permute_vars(&s_out, &P);
        let f_nat: Vec<bool> = P.iter().map(|&j| s_out_nat[j]).collect();
        // L xor F.
        let mut new_r = Vec::with_capacity(32);
        let mut new_r_nat = Vec::with_capacity(32);
        for i in 0..32 {
            new_r.push(l[i].clone() ^ f[i].clone());
            new_r_nat.push(l_native[i] ^ f_nat[i]);
        }
        l = r;
        r = new_r;
        l_native = r_native;
        r_native = new_r_nat;
    }
    // No final swap: pre-output R16 || L16.
    let mut pre = Vec::with_capacity(64);
    pre.extend_from_slice(&r);
    pre.extend_from_slice(&l);
    // FP = IP^{-1} (wiring).
    let mut fp_table = [0usize; 64];
    for (i, &v) in IP.iter().enumerate() {
        fp_table[v] = i;
    }
    Ok(permute_vars(&pre, &fp_table))
}

/// 3DES-EDE encrypt gadget: E(k1)→D(k2)→E(k1).
#[allow(clippy::too_many_arguments)]
pub fn tdes_encrypt_gadget(
    cs: ConstraintSystemRef<Fr>,
    data_des: &[Boolean<Fr>],
    k1_le: &[Boolean<Fr>],
    k2_le: &[Boolean<Fr>],
    w_data: &[u8; 8],
    w_k1: &[u8; 8],
    w_k2: &[u8; 8],
) -> Result<Vec<Boolean<Fr>>, SynthesisError> {
    use crate::des::{des_decrypt, des_encrypt};
    let t1 = des_encrypt(w_data, w_k1);
    let t2 = des_decrypt(&t1, w_k2);
    let e1 = des_gadget(cs.clone(), data_des, k1_le, true, w_data, w_k1)?;
    let e1_des_hint = {
        use crate::des::bytes_to_des_bits;
        bytes_to_des_bits(&t1);
        e1.clone()
    };
    let d2 = des_gadget(cs.clone(), &e1_des_hint, k2_le, false, &t1, w_k2)?;
    let _ = t2;
    des_gadget(cs, &d2, k1_le, true, &t2, w_k1)
}

// ---------------------------------------------------------------------------
// Full session circuit (spec §7 constraints 1–6 + aliased 7).
//
// The public-input packing lives in [`crate::abi`], which is the single
// canonical bytes <-> field mapping shared by the prover, this circuit, the
// Rust `Attestation` type and external verifiers. This module only allocates
// the resulting field elements as instance variables and links the bit
// decompositions to them.
// ---------------------------------------------------------------------------

use ark_relations::r1cs::ConstraintSynthesizer;

/// Session witnesses.
///
/// The master keys `gsk`/`usk` and the card's `idm` are the *only* free
/// secrets: `l`, `alpha` and `beta` are not witness fields at all, they are
/// derived inside the circuit by [`FelicaCircuit`]'s constraints D1-D3. That
/// is the entire point of this revision — previously `l` and `beta` were
/// unconstrained private witnesses, so anyone holding the Groth16 proving key
/// could pick them freely and mint a self-consistent attestation.
#[derive(Clone)]
pub struct FelicaCircuit {
    // --- public ---
    pub idi: [u8; 8],
    pub r1: [u8; 8],
    pub attested_at: u64,
    // --- private ---
    pub gsk: [u8; 8],
    pub usk: [u8; 8],
    pub idm: [u8; 8],
    pub c1b: [u8; 8],
    pub c2a: [u8; 8],
    pub r2: [u8; 8],
    pub auth2: [u8; 32],
}

impl FelicaCircuit {
    pub fn blank() -> Self {
        Self {
            idi: [0u8; 8],
            r1: [0u8; 8],
            attested_at: 0,
            gsk: [0u8; 8],
            usk: [0u8; 8],
            idm: [0u8; 8],
            c1b: [0u8; 8],
            c2a: [0u8; 8],
            r2: [0u8; 8],
            auth2: [0u8; 32],
        }
    }

    /// The circuit's public inputs, in wire byte form.
    pub fn public_inputs(&self) -> abi::PublicInputs {
        abi::PublicInputs {
            idi: self.idi,
            r1: self.r1,
            attested_at: self.attested_at,
        }
    }
}

impl ConstraintSynthesizer<Fr> for FelicaCircuit {
    fn generate_constraints(self, cs: ConstraintSystemRef<Fr>) -> Result<(), SynthesisError> {
        use crate::des::{des_decrypt, des_encrypt};

        // --- private witnesses (LE bits) ---
        let gsk_le = alloc_le_bits(cs.clone(), &self.gsk)?;
        let usk_le = alloc_le_bits(cs.clone(), &self.usk)?;
        let idm_le = alloc_le_bits(cs.clone(), &self.idm)?;
        let c1b_le = alloc_le_bits(cs.clone(), &self.c1b)?;
        let c2a_le = alloc_le_bits(cs.clone(), &self.c2a)?;
        let r2_le = alloc_le_bits(cs.clone(), &self.r2)?;
        let auth2_le = alloc_le_bits(cs.clone(), &self.auth2)?;

        // --- public inputs (3 Fr) ---
        // Canonical packing via the shared ABI decoder. A non-canonical
        // public input cannot be synthesised into a proof at all.
        let pis = self
            .public_inputs()
            .to_fr()
            .map_err(|_| SynthesisError::Unsatisfiable)?;
        let mut pi_vars = Vec::with_capacity(abi::PUBLIC_INPUT_COUNT);
        for pi in pis.iter() {
            pi_vars.push(FpVar::new_input(cs.clone(), || Ok(*pi))?);
        }
        // Link the two bit-decomposed public inputs. `attested_at` is echoed
        // and needs no bit linkage.
        let idi_le = alloc_le_bits(cs.clone(), &self.idi)?;
        let r1_le = alloc_le_bits(cs.clone(), &self.r1)?;
        Boolean::le_bits_to_fp(&idi_le)?.enforce_equal(&pi_vars[0])?;
        Boolean::le_bits_to_fp(&r1_le)?.enforce_equal(&pi_vars[1])?;

        // --- key derivation (D1-D3) ---
        // The FeliCa DES key schedule, moved inside the circuit. Operand
        // convention: the parenthesised argument is the DATA and the subscript
        // is the KEY, i.e. `alpha = DES_l(usk)`.
        //
        // Native values below are hints only; the constrained variables are
        // the gadget outputs.
        let l_native: [u8; 8] = std::array::from_fn(|i| self.gsk[i] ^ self.idm[i]);
        let alpha_native = des_encrypt(&self.usk, &l_native);
        let beta_native = des_encrypt(&l_native, &alpha_native);

        // D1: l = gsk XOR idm. Plain XOR constraints, no DES gadget.
        let l_le: Vec<Boolean<Fr>> = gsk_le
            .iter()
            .zip(idm_le.iter())
            .map(|(a, b)| a.clone() ^ b.clone())
            .collect();

        // D2: alpha = DES_key(l)_data(usk).
        let alpha_le = des_to_le(&des_gadget(
            cs.clone(),
            &le_to_des(&usk_le),
            &l_le,
            true,
            &self.usk,
            &l_native,
        )?);

        // D3: beta = DES_key(alpha)_data(l). `des_gadget` takes the key in LE
        // order (it applies `le_to_des` itself), so `alpha_le` goes in as-is.
        let beta_le = des_to_le(&des_gadget(
            cs.clone(),
            &le_to_des(&l_le),
            &alpha_le,
            true,
            &l_native,
            &alpha_native,
        )?);

        // --- DES-ordered views (wiring) ---
        let r1_des = le_to_des(&r1_le);
        let c1b_des = le_to_des(&c1b_le);
        let c2a_des = le_to_des(&c2a_le);
        let r2_des = le_to_des(&r2_le);

        // Constraint 1: 3DES(l,β,r1) == c1b.
        let e1 = tdes_encrypt_gadget(
            cs.clone(),
            &r1_des,
            &l_le,
            &beta_le,
            &self.r1,
            &l_native,
            &beta_native,
        )?;
        for (a, b) in e1.iter().zip(c1b_des.iter()) {
            a.enforce_equal(b)?;
        }
        // Constraint 2: 3DES(l,β,r2) == c2a (≡ 3DES⁻¹(l,β,c2a) == r2).
        let e2 = tdes_encrypt_gadget(
            cs.clone(),
            &r2_des,
            &l_le,
            &beta_le,
            &self.r2,
            &l_native,
            &beta_native,
        )?;
        for (a, b) in e2.iter().zip(c2a_des.iter()) {
            a.enforce_equal(b)?;
        }

        // Constraint 3: p = DES-CBC-decrypt(r2, auth2), zero IV.
        let mut pt_des_blocks: Vec<Vec<Boolean<Fr>>> = Vec::with_capacity(4);
        // Native plaintext for hints.
        let pt_native: Vec<u8> = {
            let mut out = Vec::with_capacity(32);
            let mut prev = [0u8; 8];
            for chunk in self.auth2.chunks(8) {
                let ct: [u8; 8] = chunk.try_into().unwrap();
                let d = des_decrypt(&ct, &self.r2);
                for i in 0..8 {
                    out.push(d[i] ^ prev[i]);
                }
                prev = ct;
            }
            out
        };
        for i in 0..4 {
            let ct_le: Vec<Boolean<Fr>> = auth2_le[i * 64..(i + 1) * 64].to_vec();
            let ct_des = le_to_des(&ct_le);
            let ct_bytes: [u8; 8] = self.auth2[i * 8..(i + 1) * 8].try_into().unwrap();
            let raw = des_gadget(cs.clone(), &ct_des, &r2_le, false, &ct_bytes, &self.r2)?;
            let prev_des: Vec<Boolean<Fr>> = if i == 0 {
                vec![Boolean::FALSE; 64]
            } else {
                le_to_des(&auth2_le[(i - 1) * 64..i * 64])
            };
            let mut ptb = Vec::with_capacity(64);
            for j in 0..64 {
                ptb.push(raw[j].clone() ^ prev_des[j].clone());
            }
            pt_des_blocks.push(ptb);
        }
        // Plaintext LE bits (256).
        let mut pt_le: Vec<Boolean<Fr>> = Vec::with_capacity(256);
        for b in pt_des_blocks.iter() {
            pt_le.extend(des_to_le(b));
        }

        // Constraint 4: MAC_verify_8(p, 0x13).
        let m0_bytes = [34u8, 0x13, 0, 0, 0, 0, 0, 0];
        let m0_le = Boolean::constant_vec_from_bytes(&m0_bytes);
        let mut m_des = le_to_des(&m0_le);
        let mut m_bytes = m0_bytes;
        for i in 0..3 {
            let key_le_block: Vec<Boolean<Fr>> = pt_le[i * 64..(i + 1) * 64].to_vec();
            let key_bytes: [u8; 8] = pt_native[i * 8..(i + 1) * 8].try_into().unwrap();
            let next = des_gadget(
                cs.clone(),
                &m_des,
                &key_le_block,
                true,
                &m_bytes,
                &key_bytes,
            )?;
            m_bytes = des_encrypt(&m_bytes, &key_bytes);
            m_des = next;
        }
        let mac_des = le_to_des(&pt_le[192..256]);
        for (a, b) in m_des.iter().zip(mac_des.iter()) {
            a.enforce_equal(b)?;
        }

        // Constraint 5: p.tid == tail_6(r1): pt[2..8] vs r1[2..8].
        for i in 16..64 {
            pt_le[i].enforce_equal(&r1_le[i])?;
        }
        // Constraint 6: p.idi == idi: pt[8..16] vs idi.
        for i in 0..64 {
            pt_le[64 + i].enforce_equal(&idi_le[i])?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod circuit_tests {
    use super::*;
    use ark_r1cs_std::R1CSVar;
    use ark_relations::r1cs::ConstraintSystem;

    #[test]
    fn sbox_poly_matches_table() {
        for b in 0..8 {
            let poly = &SBOX_POLYS[b];
            for v in 0..64u64 {
                let x = Fr::from(v);
                let mut acc = poly[63];
                for c in poly[..63].iter().rev() {
                    acc *= x;
                    acc += *c;
                }
                let expected = mapped_table(b)[v as usize] as u64;
                assert_eq!(acc, Fr::from(expected), "box {b} val {v}");
            }
        }
    }

    #[test]
    fn des_gadget_matches_spec_vector() {
        let cs = ConstraintSystem::<Fr>::new_ref();
        let key: [u8; 8] = hex::decode("133457799bbcdff1").unwrap().try_into().unwrap();
        let pt: [u8; 8] = hex::decode("0123456789abcdef").unwrap().try_into().unwrap();
        let ct: [u8; 8] = hex::decode("85e813540f0ab405").unwrap().try_into().unwrap();
        let key_le = alloc_le_bits(cs.clone(), &key).unwrap();
        let pt_le = alloc_le_bits(cs.clone(), &pt).unwrap();
        let pt_des = le_to_des(&pt_le);
        let out_des = des_gadget(cs.clone(), &pt_des, &key_le, true, &pt, &key).unwrap();
        assert!(cs.is_satisfied().unwrap());
        let out_le = des_to_le(&out_des);
        let vals: Vec<bool> = out_le.iter().map(|b| b.value().unwrap()).collect();
        let mut bytes = [0u8; 8];
        for (i, chunk) in vals.chunks(8).enumerate() {
            let mut v = 0u8;
            for (j, b) in chunk.iter().enumerate() {
                if *b {
                    v |= 1 << j;
                }
            }
            bytes[i] = v;
        }
        assert_eq!(bytes, ct);
    }
}
