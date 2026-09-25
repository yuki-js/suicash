//! Fixed RPC data types.
//!
//! The holder builds `Authentication1` from the `challenge` response, so the
//! node path (system code, area list, service list) is part of the response —
//! `c1a` alone is not actionable.
//!
//! Phase 1: `challenge` and `settle`, the two FeliCa mutual-authentication
//! RPCs. `attest` lands in phase 2.

use serde::{Deserialize, Serialize};

/// Strict hex → fixed array via the `hex` crate (8B hex fields).
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
