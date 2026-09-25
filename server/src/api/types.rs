//! Fixed RPC data types.
//!
//! The holder builds `Authentication1` from the `challenge` response, so the
//! node path (system code, area list, service list) is part of the response —
//! `c1a` alone is not actionable.
//!
//! The surface is reduced to the three calls the IDi-level statement needs:
//! `challenge` → `settle` → `attest`. There is no `cm`, no `read_spec`, no
//! `ecmd`, and no `r2` on the wire. See `README.md` for why.

use serde::{Deserialize, Serialize};

// # Unknown parameters are silently ignored
//
// jsonrpsee's `#[method]` extracts each named parameter individually, so a
// param the method does not declare — `read_spec` on `settle`, `cm` on
// `attest` — is dropped without error.
//
// `#[serde(deny_unknown_fields)]` on these structs does *not* change that: the
// `#[method]` signature takes `idm: String, r1: String, …`, and the `*Request`
// struct is assembled by hand afterwards, so serde never sees the raw params
// object. Adding the attribute is a no-op that reads as if it were enforced.
//
// Enforcing it means changing each `#[method]` to take the struct as a single
// positional param, which converts the wire format from object-form to
// array-wrapped. That is a breaking client change and belongs in a version
// bump.
//
// The consequence is pinned as tests in `tests/rpc_http.rs` rather than left as
// a surprise: a reference-era client that sends `cm` gets a valid attestation
// with the commitment quietly absent, and only fails later, on the client,
// looking for `cm_out`.

/// Strict hex → fixed array via the `hex` crate (8B/32B hex fields).
pub fn parse_hex<const N: usize>(s: &str) -> Result<[u8; N], String> {
    let v = hex::decode(s).map_err(|e| e.to_string())?;
    v.try_into()
        .map_err(|v: Vec<u8>| format!("expected {N} bytes, got {}", v.len()))
}

/// `challenge` params.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChallengeRequest {
    /// Card IDm (8-byte hex).
    pub idm: String,
    /// Holder challenge (8-byte hex).
    pub r1: String,
}

impl ChallengeRequest {
    pub fn idm_bytes(&self) -> Result<[u8; 8], String> {
        parse_hex(&self.idm)
    }
    pub fn r1_bytes(&self) -> Result<[u8; 8], String> {
        parse_hex(&self.r1)
    }
}

/// `settle` params.
///
/// `r1` is required. Without the true `r1`, C1B verification is tautological
/// (recover-then-re-encrypt always matches) and settle cannot authenticate —
/// the holder generated `r1` at challenge, so sending it costs nothing and
/// enables the genuine `3DES(L,β,r1) == c1b` check.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SettleRequest {
    pub idm: String,
    /// Holder challenge (8-byte hex) from the `challenge` call.
    pub r1: String,
    pub c1b: String,
    pub c2a: String,
}

impl SettleRequest {
    pub fn idm_bytes(&self) -> Result<[u8; 8], String> {
        parse_hex(&self.idm)
    }
    pub fn r1_bytes(&self) -> Result<[u8; 8], String> {
        parse_hex(&self.r1)
    }
    pub fn c1b_bytes(&self) -> Result<[u8; 8], String> {
        parse_hex(&self.c1b)
    }
    pub fn c2a_bytes(&self) -> Result<[u8; 8], String> {
        parse_hex(&self.c2a)
    }
}

/// `attest` params.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AttestRequest {
    pub idm: String,
    pub c1b: String,
    pub c2a: String,
    /// AUTH2 ciphertext (32-byte hex).
    pub auth2: String,
}

impl AttestRequest {
    pub fn idm_bytes(&self) -> Result<[u8; 8], String> {
        parse_hex(&self.idm)
    }
    pub fn c1b_bytes(&self) -> Result<[u8; 8], String> {
        parse_hex(&self.c1b)
    }
    pub fn c2a_bytes(&self) -> Result<[u8; 8], String> {
        parse_hex(&self.c2a)
    }
    pub fn auth2_bytes(&self) -> Result<[u8; 32], String> {
        parse_hex(&self.auth2)
    }
}

/// `challenge` result: `c1a` plus the node path the holder must use.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChallengeResponse {
    /// Reader challenge block for the card (8-byte hex).
    pub c1a: String,
    /// System code the holder must poll/select before Authentication1.
    pub system_code: u16,
    /// Area code list for the Authentication1 command.
    pub areas: Vec<u16>,
    /// Service code list for the Authentication1 command.
    pub services: Vec<u16>,
}

/// `settle` result.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SettleResponse {
    /// Final mutual-auth response for the card (8-byte hex).
    pub c2b: String,
}

/// Groth16 proof over BN254.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Groth16Proof {
    pub alg: String,
    pub a: (String, String),
    pub b: ((String, String), (String, String)),
    pub c: (String, String),
    pub public_inputs: Vec<String>,
}

/// `attest` result.
///
/// # Wire contract
///
/// | field         | bound to proof as |
/// |---------------|-------------------|
/// | `idi`         | `pi0`             |
/// | `attested_at` | `pi2`             |
///
/// Both are public inputs, so a verifier is entitled to treat them as such —
/// and `prover::verify_attestation_detailed` re-checks the correspondence
/// before returning `Ok`, which is what makes it safe to read `idi` from an
/// envelope that reported success.
///
/// `r1` is deliberately *not* echoed. It is a public input (`pi1`) and the
/// proof is bound to it, but the holder generated it and the holder is the
/// party that checks freshness against it. The oracle publishing it back
/// would be the oracle vouching for its own freshness.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AttestResponse {
    /// Derived card identifier (8-byte hex).
    pub idi: String,
    /// Oracle Unix timestamp in seconds.
    ///
    /// NOT authoritative time: it is a prover-chosen public scalar. Use it
    /// only as a drift signal, never to order attestations.
    pub attested_at: u64,
    pub proof: Groth16Proof,
}
