//! Server configuration from the environment.
//!
//! Split by sensitivity (spec `docs/spec.md` §2: key hierarchy "from
//! environment", oracle stays stateless across calls per §8):
//! - Secret key material plus the node path it belongs to (`gsk`/`usk` hex,
//!   `system_code`, `areas`, `services`), read from `FELICA_KEYS_JSON_FILE`
//!   (preferred: a mounted Secret) or `FELICA_KEYS_JSON`. System/area/service
//!   codes select the authenticated key node, so they are key material too.
//! - ConfigMap: `FELICA_BIND_ADDR` (optional), `FELICA_PROVING_KEY_PATH`
//!   (optional). Purely operational, no keying.
//!
//! Single-node deployment: one GSK/USK pair plus the node path
//! (system code, area list, service list) it belongs to.

use std::path::PathBuf;

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
    /// Filesystem path to the Groth16 proving key. Baked into the image by
    /// default; a deployment should mount one instead so it can be rotated
    /// without a rebuild.
    pub proving_key_path: PathBuf,
}

/// Read the key blob from `FELICA_KEYS_JSON_FILE` if set, else `FELICA_KEYS_JSON`.
///
/// The file form exists because an env var is a poor way to deliver a secret:
/// it is visible in `kubectl describe pod`, in `/proc/1/environ`, in crash dumps,
/// and in most APM and log-collection agents. A mounted Secret is visible to
/// strictly fewer of them.
///
/// Whichever is used, the key material is parsed and then dropped; it is held in
/// `AppConfig` as plain `[u8; 8]` for the life of the process, which is a known
/// gap (the reference had the same one despite depending on `zeroize`).
fn keys_source() -> anyhow::Result<String> {
    if let Some(path) = std::env::var_os("FELICA_KEYS_JSON_FILE") {
        let path = PathBuf::from(path);
        return std::fs::read_to_string(&path)
            .with_context(|| format!("read FELICA_KEYS_JSON_FILE at {}", path.display()));
    }
    std::env::var("FELICA_KEYS_JSON")
        .context("FELICA_KEYS_JSON_FILE or FELICA_KEYS_JSON is required")
}

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let keys_raw = keys_source()?;
        let keys: KeysJson = serde_json::from_str(&keys_raw).context("invalid key material")?;
        Ok(Self {
            bind_addr: std::env::var("FELICA_BIND_ADDR").ok(),
            gsk: keys.gsk,
            usk: keys.usk,
            system_code: keys.system_code,
            areas: keys.areas,
            services: keys.services,
            proving_key_path: std::env::var("FELICA_PROVING_KEY_PATH")
                .map(PathBuf::from)
                .unwrap_or_else(|_| crate::params::default_proving_key_path()),
        })
    }
}
