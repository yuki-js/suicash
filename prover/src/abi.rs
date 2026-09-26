//! Verifier ABI: the one canonical bytes <-> field mapping.
//!
//! This module is the single decoder shared by the prover, the circuit, the
//! Rust [`Attestation`](crate::Attestation) type and any external verifier.
//! Before this existed, `public_inputs_fr` (in `circuit`) served the producer
//! and the circuit while `verify_attestation` re-derived the layout
//! independently from hex — two implementations of one wire format, free to
//! drift.
//!
//! # Packing (3 Fr, Sui limit)
//!
//! `sui::groth16` accepts at most 8 public inputs (`MaxPublicInputs = 8`,
//! else abort `ETooManyPublicInputs`), each a 32-byte little-endian field
//! element, and consumes an Arkworks *canonical compressed* verifying key.
//! The statement here is deliberately far narrower than that cap:
//!
//! ```text
//! pi0 = idi (8B LE)   pi1 = r1 (8B LE)   pi2 = attested_at (u64 LE)
//! ```
//!
//! That is the whole public surface. Everything the transcript carries —
//! `c1b`, `c2a`, `r2`, `auth2`, and the master keys `gsk`/`usk`/`idm` — is a
//! *private* witness constrained in-circuit. A verifier learns which card
//! identifier was claimed and which session challenge it was bound to, and
//! nothing else.
//!
//! `r1` is public because it is the only freshness anchor in the protocol: it
//! comes from the holder's CSPRNG, whereas `attested_at` is a prover-chosen
//! scalar that must not be used for ordering.
//!
//! # Canonicality
//!
//! Every limb is decoded through [`canonical_fr`], which rejects any byte
//! string that is not the canonical little-endian encoding of the field
//! element it reduces to.
//!
//! Every limb here is 8 bytes, so each is far below `r` and therefore
//! *structurally* canonical — there is no ambiguous pair left to reject. The
//! check is still routed through the helper rather than dropped, for two
//! reasons: "one canonical decoder" stays literal rather than aspirational,
//! and a future repacking that widens a limb inherits the check for free.
//!
//! This was not academic. While `cm` was a 32-byte limb,
//! `Fr::from_le_bytes_mod_order` *reduced*, so `cm` and `cm - r` were two
//! distinct byte strings denoting one field element — a commitment ambiguity
//! reachable straight off the wire. Dropping the commitment removed that limb
//! and the hole with it. [`PublicInputs::from_fr`] must still reject
//! over-wide elements, or two byte strings could denote one proof.
//!
//! Both directions are checked: [`PublicInputs::to_fr`] refuses to *emit* a
//! non-canonical packing, and [`PublicInputs::from_fr`] refuses to *read* one.
//! A verifier that only checked one direction would still admit the ambiguous
//! pair.

use ark_bn254::Fr;
use ark_ff::{BigInteger, PrimeField};
use ark_serialize::CanonicalSerialize;
use thiserror::Error;

/// The only supported proof algorithm label.
pub const SUPPORTED_ALG: &str = "groth16-bn254";

/// Order of the public inputs. Also the wire order of
/// `Attestation.proof.public_inputs`.
pub const PUBLIC_INPUT_ORDER: [&str; 3] = ["idi", "r1", "attested_at"];

/// Number of public inputs the circuit publishes.
pub const PUBLIC_INPUT_COUNT: usize = 3;

/// Non-canonical or otherwise undecodable public-input encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("non-canonical public input encoding")]
pub struct NonCanonical;

type Result<T> = std::result::Result<T, NonCanonical>;

/// Canonical little-endian encoding of `f`, exactly `N` bytes.
///
/// Fails if `f` does not fit in `N` bytes, which is what makes the inverse
/// mapping injective.
fn fr_to_le<const N: usize>(f: &Fr) -> Result<[u8; N]> {
    let le = f.into_bigint().to_bytes_le();
    if le[N..].iter().any(|&b| b != 0) {
        return Err(NonCanonical);
    }
    let mut out = [0u8; N];
    out.copy_from_slice(&le[..N]);
    Ok(out)
}

/// Canonical field element for `bytes`, rejecting non-canonical encodings.
///
/// Round-trips through the wire encoding rather than comparing against the
/// modulus, so it stays correct for any field and any limb width.
fn canonical_fr(bytes: &[u8]) -> Result<Fr> {
    if bytes.len() > 32 {
        return Err(NonCanonical);
    }
    let f = Fr::from_le_bytes_mod_order(bytes);
    let mut round = Vec::with_capacity(32);
    f.serialize_compressed(&mut round).expect("Fr serializes");
    // A narrow limb is canonical iff its bytes are exactly the low
    // `bytes.len()` bytes of the canonical encoding. Since an N-byte value is
    // < 2^(8N) << r, the high bytes of `round` are necessarily zero, so this
    // reduces to "is this the canonical encoding of the element it reduces to".
    if round[..bytes.len()] != *bytes {
        return Err(NonCanonical);
    }
    Ok(f)
}

/// The circuit's public inputs, in wire byte form.
///
/// This is the decoded view of `proof.public_inputs`. Field construction
/// (producer) and field decoding (verifier) both go through this type so the
/// two can never disagree about the layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PublicInputs {
    /// Claimed card identifier — the sole identity statement (8B).
    pub idi: [u8; 8],
    /// Holder's fresh session challenge (8B).
    pub r1: [u8; 8],
    /// Prover-chosen Unix seconds; a drift signal, never an ordering key.
    pub attested_at: u64,
}

impl PublicInputs {
    /// Encode to the public-input vector.
    ///
    /// Fails if any limb is not canonically encoded (see module docs).
    pub fn to_fr(&self) -> Result<Vec<Fr>> {
        Ok(vec![
            canonical_fr(&self.idi)?,
            canonical_fr(&self.r1)?,
            Fr::from(self.attested_at),
        ])
    }

    /// Decode a public-input vector back to wire bytes.
    ///
    /// Rejects a wrong-length vector and any element that does not occupy
    /// exactly its limb width, i.e. a non-canonical encoding.
    pub fn from_fr(pis: &[Fr]) -> Result<Self> {
        if pis.len() != PUBLIC_INPUT_COUNT {
            return Err(NonCanonical);
        }
        Ok(Self {
            idi: fr_to_le::<8>(&pis[0])?,
            r1: fr_to_le::<8>(&pis[1])?,
            attested_at: u64::from_le_bytes(fr_to_le::<8>(&pis[2])?),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> PublicInputs {
        PublicInputs {
            idi: [25, 26, 27, 28, 29, 30, 31, 32],
            r1: [1, 2, 3, 4, 5, 6, 7, 8],
            attested_at: 1_758_768_000,
        }
    }

    #[test]
    fn roundtrip_is_exact() {
        let pi = sample();
        assert_eq!(PublicInputs::from_fr(&pi.to_fr().unwrap()), Ok(pi));
    }

    #[test]
    fn public_input_count_is_pinned() {
        assert_eq!(PUBLIC_INPUT_ORDER.len(), PUBLIC_INPUT_COUNT);
        assert_eq!(sample().to_fr().unwrap().len(), PUBLIC_INPUT_COUNT);
    }

    /// The claim is that 8-byte limbs are *structurally* canonical, so the
    /// canonicality helper is belt-and-braces today. Pin the structural half
    /// so a future repacking that widens a limb is caught here rather than in
    /// production — that is exactly how the 32-byte `cm` ambiguity arose.
    #[test]
    fn narrow_limbs_are_structurally_canonical() {
        for width in [1usize, 2, 4, 8, 16] {
            let limb = vec![0xFFu8; width];
            assert!(
                canonical_fr(&limb).is_ok(),
                "{width}-byte all-ones limb must be canonical"
            );
        }
        assert!(
            canonical_fr(&[0xFF; 32]).is_err(),
            "32-byte all-ones must not"
        );
    }

    /// `from_fr` must reject an element wider than its limb, otherwise a
    /// verifier would accept two byte strings for one proof.
    #[test]
    fn from_fr_rejects_wide_element() {
        let pi = sample();
        // 2^64: one past the 8-byte limb width used by every input.
        // (Fr's Ord is not the integer order, so assert on the encoding.)
        let too_wide = ark_bn254::Fr::from_le_bytes_mod_order(&[0, 0, 0, 0, 0, 0, 0, 0, 1]);
        let mut enc = Vec::new();
        too_wide
            .serialize_compressed(&mut enc)
            .expect("Fr serializes");
        assert_eq!(enc[8], 0x01, "2^64 has a non-zero 9th byte");

        for idx in 0..PUBLIC_INPUT_COUNT {
            let mut pis = pi.to_fr().unwrap();
            pis[idx] += too_wide;
            assert_eq!(
                PublicInputs::from_fr(&pis),
                Err(NonCanonical),
                "limb {idx} must reject a wide element"
            );
        }
    }

    #[test]
    fn from_fr_rejects_wrong_length() {
        let pis = sample().to_fr().unwrap();
        assert_eq!(PublicInputs::from_fr(&pis[..2]), Err(NonCanonical));
        let mut long = pis.clone();
        long.push(Fr::from(0u64));
        assert_eq!(PublicInputs::from_fr(&long), Err(NonCanonical));
    }
}
