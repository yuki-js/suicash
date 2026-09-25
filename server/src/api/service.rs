//! Oracle business logic: the `impl OracleImpl` processing behind the wire
//! handlers in [`super::handler`]. Wire translation stays there; everything
//! that decides, validates, or computes lives here.
//!
//! Phase 1 covers the two FeliCa mutual-authentication RPCs. `attest` — the
//! ZK proof — arrives in phase 2 and needs no change here beyond adding a
//! method.

use jsonrpsee::core::RpcResult;

use super::handler::OracleImpl;
use super::types::{ChallengeRequest, ChallengeResponse, SettleRequest, SettleResponse};

impl OracleImpl {
    /// First leg: derive the card's session keys from `(gsk, usk, idm)` and
    /// answer with C1A, the reader half of the first challenge.
    ///
    /// The response also carries the node path (system code, areas, services)
    /// because the holder needs it to build `Authentication1`, and `c1a` alone
    /// is not actionable.
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

    /// Second leg: verify the card's C1B against the holder's `r1`, recover
    /// R2 from C2A, and answer with C2B.
    ///
    /// This is the whole job in this phase. The reference implementation also
    /// built an encrypted single-block `Read` command here and returned it as
    /// `ecmd`; that is gone, because `auth2` already carries `idi` and the
    /// IDi-level statement covers no read response.
    ///
    /// `r1` is a required parameter precisely so this check is not
    /// tautological: without the holder's real challenge, C1B would be
    /// recover-then-re-encrypt and always match.
    pub async fn settle(&self, req: SettleRequest) -> RpcResult<SettleResponse> {
        let idm = req.idm_bytes().map_err(crate::error::invalid_params)?;
        let r1 = req.r1_bytes().map_err(crate::error::invalid_params)?;
        let c1b = req.c1b_bytes().map_err(crate::error::invalid_params)?;
        let c2a = req.c2a_bytes().map_err(crate::error::invalid_params)?;
        let session =
            crate::oracle::OracleKeys::new(self.config.gsk, self.config.usk).session(&idm);
        // Genuine C1B authentication: `3DES(L,β,r1) == c1b`.
        if !session.check_c1b(&r1, &c1b) {
            return Err(crate::error::c1b_mismatch());
        }
        let r2 = session.open_r2(&c2a);
        let c2b = session.c2b(&r2);
        Ok(SettleResponse {
            c2b: hex::encode(c2b),
        })
    }
}
