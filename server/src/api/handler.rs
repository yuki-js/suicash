//! RPC wire layer: trait, struct, and pure delegation to [`super::service`].
//! No deciding, validating, or computing happens here.

use std::sync::{Arc, OnceLock};

use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
};
use tokio::sync::Semaphore;

use super::types::{
    AttestRequest, AttestResponse, ChallengeRequest, ChallengeResponse, SettleRequest,
    SettleResponse,
};
use crate::config::AppConfig;
use crate::params::ProvingKeyBytes;

/// Concurrent `attest` operations allowed.
///
/// One Groth16 prove is ~1.2 s of CPU-bound work peaking near 500 MB across
/// ~10 rayon threads. Without a gate, N concurrent `attest` calls each grab a
/// core set and the pod runs out of memory long before the CPU limit stops
/// them. One at a time matches the measured footprint, and the deployment's
/// `replicas: 1` already assumes it.
///
/// Requests beyond this queue on the semaphore rather than being rejected, so
/// the failure mode is latency, not an error.
const MAX_CONCURRENT_PROOFS: usize = 1;

/// Wire params stay destructured: jsonrpsee maps spec's object-form params
/// (`{"idm": …, "r1": …}`) onto these by name. Each handler immediately folds
/// them into the corresponding `*Request` struct — the struct is the fixed
/// request type; validation lives on it.
#[rpc(server)]
pub trait OracleApi {
    #[method(name = "ping")]
    async fn ping(&self) -> RpcResult<String>;

    #[method(name = "challenge")]
    async fn challenge(&self, idm: String, r1: String) -> RpcResult<ChallengeResponse>;

    #[method(name = "settle")]
    async fn settle(
        &self,
        idm: String,
        r1: String,
        c1b: String,
        c2a: String,
    ) -> RpcResult<SettleResponse>;

    #[method(name = "attest")]
    async fn attest(
        &self,
        idm: String,
        c1b: String,
        c2a: String,
        auth2: String,
    ) -> RpcResult<AttestResponse>;

    #[method(name = "get_verifying_key")]
    async fn get_verifying_key(&self) -> RpcResult<String>;
}

pub struct OracleImpl {
    pub(crate) config: Arc<AppConfig>,
    /// Setup-loaded proving-key bytes (one load at startup, shared read-only).
    pub(crate) proving_key: ProvingKeyBytes,
    /// Deserialized key, initialized once (startup warm or first attest).
    ///
    /// Held as an `Arc` so `attest` can move a handle into the blocking pool
    /// for the ~1.2 s prove. Cloning the `ProvingKey` itself would copy ~30 MB
    /// per request; cloning the `Arc` is a refcount bump.
    pk_cache: OnceLock<Arc<prover::FelicaProvingKey>>,
    /// Bounds concurrent proofs; see [`MAX_CONCURRENT_PROOFS`].
    proof_slots: Semaphore,
}

impl OracleImpl {
    /// Construction does no I/O; `main` loads via `ProvingKeyBytes::load`.
    pub fn new(config: AppConfig, proving_key: ProvingKeyBytes) -> Self {
        Self {
            config: Arc::new(config),
            proving_key,
            pk_cache: OnceLock::new(),
            proof_slots: Semaphore::new(MAX_CONCURRENT_PROOFS),
        }
    }

    /// Deserialize the preloaded bytes once. Called at startup for fail-fast
    /// validation; `attest` reuses the cache without per-request I/O.
    pub fn ensure_keys_loaded(&self) -> anyhow::Result<()> {
        self.load_cached()
            .map(|_| ())
            .map_err(|e| anyhow::anyhow!("invalid proving key: {e}"))
    }

    pub(crate) fn proving_key(
        &self,
    ) -> Result<Arc<prover::FelicaProvingKey>, prover::KeyLoadError> {
        self.load_cached()
    }

    /// Run one proof off the async runtime, gated on the concurrency slot.
    ///
    /// `prover::prove` is synchronous and CPU-bound. Called directly from an
    /// `async fn` it would occupy a Tokio worker thread for the whole ~1.2 s,
    /// which also stops that worker from answering `ping` — the liveness and
    /// readiness probes poll exactly that, so a few concurrent `attest` calls
    /// could get the pod killed mid-proof. `spawn_blocking` moves the work to
    /// the blocking pool, and the semaphore keeps the pool from being flooded.
    pub(crate) async fn prove_blocking(
        &self,
        pk: Arc<prover::FelicaProvingKey>,
        req: prover::ProveRequest,
    ) -> Result<prover::Attestation, prover::ProverError> {
        let _permit = self
            .proof_slots
            .acquire()
            .await
            .map_err(|_| prover::ProverError::ProveFailed)?;
        tokio::task::spawn_blocking(move || prover::prove(&pk, &req))
            .await
            .map_err(|_| prover::ProverError::ProveFailed)?
    }

    /// Stable-only once-init (`OnceLock::get_or_try_init` is unavailable on
    /// this toolchain). A raced loser is dropped; the winner is reused.
    fn load_cached(&self) -> Result<Arc<prover::FelicaProvingKey>, prover::KeyLoadError> {
        if let Some(pk) = self.pk_cache.get() {
            return Ok(Arc::clone(pk));
        }
        let pk = Arc::new(prover::load_proving_key(self.proving_key.bytes())?);
        // A raced loser is dropped; the winner's Arc is the one returned.
        let _ = self.pk_cache.set(Arc::clone(&pk));
        Ok(self.pk_cache.get().expect("cache set above").clone())
    }
}

#[async_trait]
impl OracleApiServer for OracleImpl {
    async fn ping(&self) -> RpcResult<String> {
        Ok("pong".to_string())
    }

    async fn challenge(&self, idm: String, r1: String) -> RpcResult<ChallengeResponse> {
        self.challenge(ChallengeRequest { idm, r1 }).await
    }

    async fn settle(
        &self,
        idm: String,
        r1: String,
        c1b: String,
        c2a: String,
    ) -> RpcResult<SettleResponse> {
        self.settle(SettleRequest { idm, r1, c1b, c2a }).await
    }

    async fn attest(
        &self,
        idm: String,
        c1b: String,
        c2a: String,
        auth2: String,
    ) -> RpcResult<AttestResponse> {
        self.attest(AttestRequest {
            idm,
            c1b,
            c2a,
            auth2,
        })
        .await
    }

    async fn get_verifying_key(&self) -> RpcResult<String> {
        self.get_verifying_key().await
    }
}
