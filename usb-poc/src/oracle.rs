//! JSON-RPC client for the FeliCa oracle (`server/`).
//!
//! The oracle is a *stateless card-key-derivation oracle*: it holds the FeliCa
//! master keys and performs only the DES operations that need them. Everything
//! else — polling, framing, and the card itself — happens in this process.
//!
//! The split is deliberate and is what makes this PoC hold no secrets. The
//! oracle never sees a block of card data, and this process never sees a key:
//! the two only ever exchange the four challenge blocks needed to complete
//! FeliCa mutual authentication, plus the resulting attestation.
//!
//! Note on error handling: jsonrpsee answers a JSON-RPC level failure with
//! **HTTP 200** and an `error` member in the body, so transport success and
//! protocol success are separate things and both are checked.

use anyhow::{anyhow, bail, Context, Result};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};

/// `challenge` result: C1A plus the node path the card must be addressed with.
#[derive(Debug, Clone, Deserialize)]
pub struct Challenge {
    /// `3DES(alpha, L, R1)` — what the card checks to decide we are legitimate.
    pub c1a: String,
    /// System code the oracle is provisioned for. Cross-checked against the
    /// code the card was polled with, because a mismatch means the oracle is
    /// keyed for a different system than the card in front of us.
    pub system_code: u16,
    /// Area codes, in order, forming the node path down to the leaf service.
    pub areas: Vec<u16>,
    /// Service codes, in order, forming the leaf of the node path.
    pub services: Vec<u16>,
}

/// `settle` result: the card's C2B, which closes mutual authentication.
#[derive(Debug, Clone, Deserialize)]
pub struct Settle {
    pub c2b: String,
}

/// `attest` result: the claimed identity and its proof.
#[derive(Debug, Clone, Deserialize)]
pub struct AttestResult {
    pub idi: String,
    pub attested_at: u64,
    pub proof: WireProof,
}

/// The oracle's `a`/`b`/`c` are hex coordinate strings; `b` is two Fq2 pairs.
#[derive(Debug, Clone, Deserialize)]
pub struct WireProof {
    pub alg: String,
    pub a: (String, String),
    pub b: ((String, String), (String, String)),
    pub c: (String, String),
    /// Exactly three 32-byte little-endian scalars, hex-encoded: `idi`, `r1`,
    /// `attested_at` in that order.
    pub public_inputs: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RpcError {
    code: i32,
    message: String,
}

#[derive(Debug, Deserialize)]
struct Reply<T> {
    result: Option<T>,
    error: Option<RpcError>,
}

pub struct Oracle {
    base: String,
    agent: ureq::Agent,
}

impl Oracle {
    pub fn new(base: impl Into<String>) -> Result<Self> {
        let mut base = base.into();
        while base.ends_with('/') {
            base.pop();
        }
        if base.is_empty() {
            bail!("--oracle must not be empty");
        }
        // A 30 s ceiling keeps a wedged oracle from hanging the demo; `attest`
        // legitimately takes ~1.2 s of CPU on the server.
        let config = ureq::AgentBuilder::new()
            .timeout_connect(std::time::Duration::from_secs(10))
            .timeout_read(std::time::Duration::from_secs(30))
            .timeout_write(std::time::Duration::from_secs(10))
            .build();
        Ok(Self { base, agent: config })
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    fn call<T: DeserializeOwned>(&self, method: &str, params: Value) -> Result<T> {
        let body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        });

        // A non-2xx status here is a proxy/ingress failure, not a JSON-RPC error;
        // ureq surfaces it as `Error::Status`, which `?` turns into a failure.
        let response = self
            .agent
            .post(&self.base)
            .set("Content-Type", "application/json")
            .send_json(body)
            .with_context(|| format!("POST {method} to {}", self.base))?;

        let reply: Reply<T> = response
            .into_json()
            .with_context(|| format!("decoding the {method} reply"))?;

        if let Some(err) = reply.error {
            bail!("oracle {method} failed: [{}] {}", err.code, err.message);
        }
        reply
            .result
            .ok_or_else(|| anyhow!("oracle {method} returned neither result nor error"))
    }

    /// Liveness. Also the cheapest way to confirm the URL and the key set.
    pub fn ping(&self) -> Result<String> {
        self.call("ping", json!([]))
    }

    /// Fetch the Groth16 verifying key (360 bytes for a 3-public-input circuit).
    pub fn verifying_key(&self) -> Result<String> {
        self.call("get_verifying_key", json!([]))
    }

    /// Ask the oracle for C1A and the node path.
    ///
    /// `r1` is *our* random; the oracle never chooses it, which is what makes
    /// freshness a property this process can check rather than one it must trust.
    pub fn challenge(&self, idm: &str, r1: &str) -> Result<Challenge> {
        self.call("challenge", json!({ "idm": idm, "r1": r1 }))
    }

    /// Hand the card's C1B and C2A back; receive C2B.
    ///
    /// This is the bilateral step: the oracle can only accept C1B if it derives
    /// the same `L` and `beta` the card did, which requires both the master
    /// keys and the same IDm.
    pub fn settle(&self, idm: &str, r1: &str, c1b: &str, c2a: &str) -> Result<Settle> {
        self.call(
            "settle",
            json!({ "idm": idm, "r1": r1, "c1b": c1b, "c2a": c2a }),
        )
    }

    /// Ask for a Groth16 attestation of the IDi inside the AUTH2 frame.
    pub fn attest(&self, idm: &str, c1b: &str, c2a: &str, auth2: &str) -> Result<AttestResult> {
        self.call(
            "attest",
            json!({ "idm": idm, "c1b": c1b, "c2a": c2a, "auth2": auth2 }),
        )
    }
}
