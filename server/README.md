# `server` — FeliCa oracle, phase 1 of 3

The oracle completes FeliCa DES mutual authentication on a card holder's
behalf, then attests the card's identity with a zero-knowledge proof.

This is **phase 1**: the two mutual-authentication RPCs, `challenge` and
`settle`. The third RPC, `attest`, and with it the Groth16 circuit and the
proving key, lands in phase 2.

`../ref-impl-of-oracle/` is the **reference implementation**: frozen, kept
green, deliberately not modified. This crate is the new thing, and phase 3
will pair it with `../prover/`.

## Phasing

| Phase | Adds | Needs a proving key? |
|---|---|---|
| **1 — this crate** | `challenge`, `settle` | no |
| 2 | `attest`, `get_verifying_key`, proving-key loading | yes |
| 3 | (nothing new; the two crates together) | yes |

Splitting here is not only about diff size. Phase 1 has **no ZK machinery at
all**: no circuit, no proving key, no `FELICA_PROVING_KEY_PATH`, no
`FELICA_TEST_PK`. The whole suite runs in milliseconds instead of ~1.2 s per
proof, and the server starts in milliseconds instead of ~4 s deserializing a
31 MB key. Each phase is independently reviewable and independently runnable.

## RPC surface

| Method | Params | Result |
|---|---|---|
| `ping` | — | `"pong"` |
| `challenge` | `{idm, r1}` | `{c1a, system_code, areas, services}` |
| `settle` | `{idm, r1, c1b, c2a}` | `{c2b}` |

Flow: `challenge` → card `Authentication1` → `settle` → card `Authentication2`.

- `challenge` derives the card's session keys from `(gsk, usk, idm)` and
  answers with C1A. It also returns the **node path** (system code, areas,
  services) because the holder needs it to build `Authentication1`, and `c1a`
  alone is not actionable.
- `settle` verifies the card's C1B against the holder's `r1`, recovers R2 from
  C2A, and answers with C2B. That completes mutual authentication.

Stateless: the two calls share no session state. The key material is
pre-resolved (`FELICA_KEYS_JSON` carries GSK/USK plus the node path), so
system → area → service chain resolution is provisioning's job, not the
server's.

Error codes: `-32602` invalid params, `-32012` `C1B_MISMATCH`, `-32603`
internal. `-32010` `MAC_MISMATCH` and `-32011` `TID_MISMATCH` are defined in
`error.rs` but unused until phase 2 — they belong to AUTH2 verification, which
`attest` drives.

### Why `settle` requires `r1`

Without the holder's real challenge, the C1B check is recover-then-re-encrypt
and *always* matches — `settle` could not authenticate anything. The holder
generated `r1` at `challenge`, so sending it costs nothing and makes the check
genuine: `3DES(L,β,r1) == c1b`, else `C1B_MISMATCH`.

`settle_rejects_c1b_from_a_different_r1` pins that a C1B from another session
is refused, which is the property `r1` buys.

## Removed from the reference, and why

| Reference | Here | Reason |
|---|---|---|
| `settle` takes `read_spec`, returns `ecmd` | gone | `auth2` already carries `idi`; the IDi-level statement covers no read response |
| `schedule` mirrors secure-messaging framing (PKCS#7, forward MAC, DES-CBC, read command/response) | gone | 468 → 118 lines; AUTH2 verification delegates to felica-rs anyway |
| `params.rs`, `FELICA_PROVING_KEY_PATH` | gone | phase 2 |
| `k_group`/`k_user` field names | `gsk`/`usk` | matches the prover's `ProveRequest` |
| `read_invariants.rs` (450 lines) | gone | no read path to pin |

`src/` is ~640 lines. Tests: 9 across 3 files.

## Protocol break vs the reference

**Unknown params are silently ignored.** jsonrpsee extracts named parameters
individually, so a param the method does not declare — `read_spec` on
`settle` — is dropped without error. `#[serde(deny_unknown_fields)]` on
`SettleRequest` does *not* help: the struct is assembled by hand after
extraction and never sees the raw params object. Enforcing it would mean
switching the trait to take each struct as a single positional param, which
changes the wire format from object-form to array-wrapped.

Consequence: a reference-era client that asks for a read still gets a **valid**
settlement with no read in it, and nothing errors. Pinned as a test rather
than left as a surprise. The clean fix is the wire-format change, and it
belongs in a version bump — not a silent patch.

## Known limitation, unchanged from the reference

**No transport authentication and no rate limiting.** `settle` is reachable by
anyone who can reach the port, and it is cheap enough that a caller can drive
sessions at will. The spec assigns client authentication and rate limiting to
a gateway (§11); nothing in this crate enforces either.

`settle_has_no_transport_authentication` pins this as a decision on record
rather than an oversight. It matters more than it looks: the oracle holds the
FeliCa master keys and derives per-card session keys for any `idm` it is
asked about, so this endpoint is a card-key-derivation oracle. Put a gateway
in front of it before exposing it.

## Run it

```sh
cargo run --release --example rpc_auth    # prints the keys the server needs
```

The example drives the emulated card itself, so the server must be started
with the fixture's derived keys:

```sh
FELICA_KEYS_JSON='{"gsk":"4abbe342d26c64f1","usk":"53af2483920bf339","system_code":3,"areas":[64],"services":[72]}' \
FELICA_BIND_ADDR=127.0.0.1:3000 \
  cargo run --release
```

## Tests

```
cargo build --release                        clean
cargo fmt --check                            clean
cargo clippy --all-targets -- -D warnings    clean
cargo test --release                         9 passed, 0 failed
```

No environment setup: no `FELICA_TEST_PK`, no ceremony, no fixtures on disk.

`tests/rpc_http.rs` runs a real jsonrpsee server on a loopback port and speaks
raw HTTP/1.1 to it, so the wire format is covered rather than assumed — and it
asserts that batch requests are refused, matching `src/main.rs`.

`tests/schedule_proof.rs` pins the DES schedule against the FIPS vector and
drives a real felica-rs emulated card end to end, including AUTH2
verification — that is the code phase 2 builds on, so it is worth having green
now.

## Layout

```
src/main.rs            bootstrap; no key material on disk in this phase
src/config.rs          FELICA_KEYS_JSON, bind addr
src/error.rs           JSON-RPC error codes
src/api/handler.rs     trait + struct
src/api/service.rs     challenge and settle logic
src/api/types.rs       wire DTOs
src/oracle/mod.rs      key schedule, AUTH2 verification
src/oracle/schedule.rs challenge schedule only
src/oracle/fixture.rs  shared emulated card (tests + example)
```
