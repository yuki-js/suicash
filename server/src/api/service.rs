//! Oracle business logic: the `impl OracleImpl` processing behind the wire
//! handlers in [`super::handler`]. Wire translation stays there; everything
//! that decides, validates, or computes lives here.

use jsonrpsee::core::RpcResult;

use super::handler::OracleImpl;
use super::types::{
    AttestRequest, AttestResponse, ChallengeRequest, ChallengeResponse, SettleRequest,
    SettleResponse,
};

impl OracleImpl {
    /// Verifying key bytes as hex. Encoded from the deserialized cache.
    pub async fn get_verifying_key(&self) -> RpcResult<String> {
        let pk = self
            .proving_key()
            .map_err(|e| crate::error::prove_failed(e.to_string()))?;
        Ok(hex::encode(prover::encode_verifying_key(&pk.vk)))
    }

    pub async fn challenge(&self, req: ChallengeRequest) -> RpcResult<ChallengeResponse> {
        let idm = req.idm_bytes().map_err(crate::error::invalid_params)?;
        let r1 = req.r1_bytes().map_err(crate::error::invalid_params)?;
        let oracle = crate::oracle::OracleKeys::new(self.config.gsk, self.config.usk);
        if self.config.areas.is_empty() && self.config.services.is_empty() {
            return Err(crate::error::internal(
                "oracle node path not configured (set AREAS/SERVICES)",
            ));
        }
        let c1a = oracle.session(&idm).c1a(&r1);
        Ok(ChallengeResponse {
            c1a: hex::encode(c1a),
            system_code: self.config.system_code,
            areas: self.config.areas.clone(),
            services: self.config.services.clone(),
        })
    }

    /// Settle the second leg of mutual auth and hand back `c2b`.
    ///
    /// At IDi level this is the whole job. The reference implementation also
    /// built an encrypted single-block `Read` command here and returned it as
    /// `ecmd`; that is gone, because `auth2` already carries `idi` and nothing
    /// in the statement covers a read response.
    pub async fn settle(&self, req: SettleRequest) -> RpcResult<SettleResponse> {
        let idm = req.idm_bytes().map_err(crate::error::invalid_params)?;
        let r1 = req.r1_bytes().map_err(crate::error::invalid_params)?;
        let c1b = req.c1b_bytes().map_err(crate::error::invalid_params)?;
        let c2a = req.c2a_bytes().map_err(crate::error::invalid_params)?;
        let session =
            crate::oracle::OracleKeys::new(self.config.gsk, self.config.usk).session(&idm);
        // Genuine C1B authentication (possible only because `r1` is supplied):
        // `3DES(L,β,r1) == c1b`, else `C1B_MISMATCH`.
        if !session.check_c1b(&r1, &c1b) {
            return Err(crate::error::c1b_mismatch());
        }
        let r2 = session.open_r2(&c2a);
        let c2b = session.c2b(&r2);
        Ok(SettleResponse {
            c2b: hex::encode(c2b),
        })
    }

    pub async fn attest(&self, req: AttestRequest) -> RpcResult<AttestResponse> {
        use crate::oracle::{verify_session, AttestError};
        use std::time::{SystemTime, UNIX_EPOCH};

        // Proving uses only the setup-loaded key: deserialized once and
        // cached (`handler::ensure_keys_loaded` warms it at startup).
        // Key generation never happens here.
        let pk = self
            .proving_key()
            .map_err(|e| crate::error::prove_failed(e.to_string()))?;
        let idm = req.idm_bytes().map_err(crate::error::invalid_params)?;
        let c1b = req.c1b_bytes().map_err(crate::error::invalid_params)?;
        let c2a = req.c2a_bytes().map_err(crate::error::invalid_params)?;
        let auth2 = req.auth2_bytes().map_err(crate::error::invalid_params)?;
        let keys = crate::oracle::OracleKeys::new(self.config.gsk, self.config.usk);
        let verified = verify_session(&keys, &idm, &c1b, &c2a, &auth2).map_err(|e| match e {
            AttestError::MacMismatch => crate::error::mac_mismatch(),
            // UnexpectedTn shares TID_MISMATCH: both mean "this AUTH2 is not
            // from the fresh session you claim" (the spec has no separate code).
            AttestError::TidMismatch | AttestError::UnexpectedTn => crate::error::tid_mismatch(),
            AttestError::Malformed => crate::error::invalid_params(e.to_string()),
        })?;
        let attested_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| crate::error::internal(e.to_string()))?
            .as_secs();

        // Groth16 proof over (idi, r1, attested_at) of the transcript
        // (c1b, c2a, auth2, gsk, usk, idm). All six of those are private
        // witnesses: the verifier learns the identity claim and nothing else.
        //
        // Proving is ~1.2 s of CPU-bound work. It is dispatched to the
        // blocking pool and gated on a semaphore so concurrent `attest` calls
        // queue instead of thrashing every core and stalling the event loop
        // that also serves `ping` (which is what the liveness probe polls).
        let req_for_prove = prover::ProveRequest {
            idm,
            c1b,
            c2a,
            auth2,
            gsk: self.config.gsk,
            usk: self.config.usk,
            attested_at,
        };
        let att = self
            .prove_blocking(pk, req_for_prove)
            .await
            .map_err(|e| match e {
                prover::ProverError::MacMismatch => crate::error::mac_mismatch(),
                prover::ProverError::TidMismatch => crate::error::tid_mismatch(),
                prover::ProverError::C1bMismatch => crate::error::c1b_mismatch(),
                prover::ProverError::ProveFailed => crate::error::prove_failed(e.to_string()),
            })?;

        // The prover and the session verifier extract IDi independently, so
        // disagreement means one of them is wrong. This was a
        // `debug_assert_eq!`, which is compiled out in release — so in
        // production the oracle would attest to a session it had not actually
        // verified. Enforced unconditionally.
        if att.idi != verified.idi {
            return Err(crate::error::internal(
                "prover IDi disagrees with session verifier",
            ));
        }

        Ok(AttestResponse {
            idi: hex::encode(att.idi),
            attested_at: att.attested_at,
            proof: super::types::Groth16Proof {
                alg: att.proof.alg,
                a: att.proof.a,
                b: att.proof.b,
                c: att.proof.c,
                public_inputs: att.proof.public_inputs,
            },
        })
    }
}
