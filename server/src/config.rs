//! Server configuration from the environment.
//!
//! Split by sensitivity (spec `docs/spec.md` §2: key hierarchy "from
//! environment", oracle stays stateless across calls per §8):
//! - Secret `FELICA_KEYS_JSON`: key material plus the node path it belongs
//!   to (`gsk`/`usk` hex, `system_code`, `areas`, `services`).
//!   System/area/service codes select the authenticated key node, so they
//!   are key material too.
//! - ConfigMap: `FELICA_BIND_ADDR` (optional). Purely operational, no keying.
//!
//! Single-node deployment: one GSK/USK pair plus the node path
//! (system code, area list, service list) it belongs to.
//!
//! There is no proving-key configuration in this phase — `attest` and the ZK
//! circuit arrive in phase 2 and bring `FELICA_PROVING_KEY_PATH` with them.

use anyhow::Context;
use serde::Deserialize;

use crate::api::parse_hex;

fn de_hex8<'de, D>(d: D) -> Result<[u8; 8], D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(d)?;
    parse_hex::<8>(&s).map_err(serde::de::Error::custom)
}

/// Key material from the Secret: resolved keys plus the node path they
/// belong to. System/area/service codes select the authenticated key node.
#[derive(Debug, Clone, Deserialize)]
struct KeysJson {
    #[serde(deserialize_with = "de_hex8")]
    gsk: [u8; 8],
    #[serde(deserialize_with = "de_hex8")]
    usk: [u8; 8],
    system_code: u16,
    areas: Vec<u16>,
    services: Vec<u16>,
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub bind_addr: Option<String>,
    /// Resolved GSK (8-byte key from the Secret).
    pub gsk: [u8; 8],
    /// Resolved USK (8-byte key from the Secret).
    pub usk: [u8; 8],
    /// System code the holder must poll/select.
    pub system_code: u16,
    /// Area code list for Authentication1.
    pub areas: Vec<u16>,
    /// Service code list for Authentication1.
    pub services: Vec<u16>,
}

fn required_var(var: &str) -> anyhow::Result<String> {
    std::env::var(var).with_context(|| format!("{var} env var is required"))
}

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let keys_raw = required_var("FELICA_KEYS_JSON")?;
        let keys: KeysJson = serde_json::from_str(&keys_raw).context("invalid FELICA_KEYS_JSON")?;
        Ok(Self {
            bind_addr: std::env::var("FELICA_BIND_ADDR").ok(),
            gsk: keys.gsk,
            usk: keys.usk,
            system_code: keys.system_code,
            areas: keys.areas,
            services: keys.services,
        })
    }
}
