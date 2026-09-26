# Build context is the repository root: the `oracle` crate depends on the
# `prover` crate by relative path, and both plus the ceremony key are copied in.
#
#   docker build -f server/Dockerfile -t felica-ida-oracle .
#   docker run --rm -p 3000:3000 \
#     -e FELICA_KEYS_JSON='{"gsk":"<16 hex>","usk":"<16 hex>","system_code":3,"areas":[64],"services":[72]}' \
#     felica-ida-oracle
#
# ⚠ The attestation produced by this image does NOT prove card presence. The
#   circuit constrains transcript consistency and key-schedule binding only; see
#   prover/README.md and the open P0 in prover/tests/forgery_authority.rs.
#   Do not describe a valid attestation as evidence that a card was present.

# ── builder ───────────────────────────────────────────────────────────────────
FROM rust:1.89-slim-bookworm AS builder

WORKDIR /build

# Cache dependency compilation independently of source churn: stub the crate
# roots, resolve the graph, then copy the real sources over the stubs.
COPY prover/Cargo.toml prover/Cargo.lock ./prover/
COPY server/Cargo.toml server/Cargo.lock ./server/
RUN mkdir -p prover/src server/src \
 && echo 'fn main() {}' > server/src/main.rs \
 && : > server/src/lib.rs \
 && : > prover/src/lib.rs \
 && rm -f prover/Cargo.lock \
 && (cd server && cargo fetch --locked) \
 && rm -f server/Cargo.lock

COPY prover ./prover
COPY server ./server

# --locked so a stale lockfile fails the build rather than silently resolving
# different dependency versions than CI tested.
RUN cd server && cargo build --release --locked --bin oracle \
 && strip target/release/oracle

# ── runtime ───────────────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl \
 && rm -rf /var/lib/apt/lists/*

# 65532 matches the `runAsUser` in the reference manifests. Non-root by
# construction, not only by policy.
RUN useradd -u 65532 -r -s /usr/sbin/nologin appuser

COPY --from=builder /build/server/target/release/oracle /usr/local/bin/oracle

# The Groth16 proving key. Baked so `docker run` works out of the box; override
# FELICA_PROVING_KEY_PATH to mount one instead, which is what a deployment
# should do — the image is 31 MB heavier for it, and a baked key cannot be
# rotated without a rebuild.
#
# This key is *public material* by construction: Groth16 verifying keys are not
# secret, and the proving key here is a development-grade single-party setup
# (see prover/README.md). Baking it is a packaging convenience, not a security
# control.
COPY prover/assets/proving_key.bin /app/proving-key/key.bin

USER 65532:65532
EXPOSE 3000

ENV FELICA_BIND_ADDR=0.0.0.0:3000 \
    FELICA_PROVING_KEY_PATH=/app/proving-key/key.bin

# Exercises the real JSON-RPC surface, not just a TCP accept. Startup
# deserializes the 31 MB key before binding, so give it room.
HEALTHCHECK --interval=30s --timeout=5s --start-period=60s --retries=3 \
  CMD curl -sf -X POST http://127.0.0.1:3000 \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc":"2.0","id":1,"method":"ping","params":[]}' | grep -q pong

ENTRYPOINT ["/usr/local/bin/oracle"]
