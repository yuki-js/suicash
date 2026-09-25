# `server` — FeliCa oracle, IDi-level

The oracle completes FeliCa DES mutual authentication on a card holder's
behalf, then attests the card's identity with a zero-knowledge proof.

`../ref-impl-of-oracle/` is the **reference implementation**: frozen, kept
green, deliberately not modified. This crate is the new thing, and it pairs
with `../prover/`.

```sh
diff -u ../ref-impl-of-oracle/server/src/api/service.rs src/api/service.rs
```

## RPC surface

| Method | Params | Result |
|---|---|---|
| `ping` | — | `"pong"` |
| `challenge` | `{idm, r1}` | `{c1a, system_code, areas, services}` |
| `settle` | `{idm, r1, c1b, c2a}` | `{c2b}` |
| `attest` | `{idm, c1b, c2a, auth2}` | `{idi, attested_at, proof}` |
| `get_verifying_key` | — | vk hex (360 bytes for 3 public inputs) |

Flow: `challenge` → card `Authentication1` → `settle` → card `Authentication2`
→ `attest`. Stateless; the three calls share no session state.

- `challenge` derives the card's session keys from `(gsk, usk, idm)` and
  answers with C1A, plus the **node path** (system code, areas, services) that
  the holder needs to build `Authentication1` — `c1a` alone is not actionable.
- `settle` verifies C1B against the holder's `r1`, recovers R2 from C2A, and
  answers with C2B. That completes mutual authentication.
- `attest` verifies the AUTH2 ciphertext and produces a Groth16 proof binding
  the claimed `idi` to the master-key derivation.

Error codes: `-32602` invalid params, `-32010` `MAC_MISMATCH`, `-32011`
`TID_MISMATCH` (also covers a non-fresh AUTH2 transaction number), `-32012`
`C1B_MISMATCH`, `-32020` `PROVE_FAILED`, `-32603` internal.

## The `attest` envelope

Three public inputs, well under Sui's cap of 8. Everything the transcript
carries — `c1b`, `c2a`, `r2`, `auth2`, and the master keys — is a **private
witness**, so a verifier learns the identity claim and nothing else.

| PI | Content |
|---|---|
| `pi0` | `idi` — the sole identity claim |
| `pi1` | `r1` — the holder's fresh session challenge |
| `pi2` | `attested_at` — prover-chosen, drift signal only |

The response is `{idi, attested_at, proof}` and nothing else. `prover::
verify_attestation_detailed` re-checks that correspondence before returning
`Ok`, which is what makes it safe to read `idi` from an envelope that reported
success. `attest_envelope_is_idi_only` pins the shape.

**`r1` is deliberately not echoed.** It is a public input and the proof is
bound to it, but the holder generated it and the holder is the party that
checks freshness. The oracle publishing it back would be the oracle vouching
for its own freshness.

## Removed from the reference, and why

| Reference | Here | Reason |
|---|---|---|
| `settle` takes `read_spec`, returns `ecmd` | gone | `auth2` already carries `idi`; the statement covers no read response |
| `schedule` mirrors secure-messaging framing (PKCS#7, forward MAC, DES-CBC, read command/response) | gone | 468 → 118 lines; AUTH2 verification delegates to felica-rs anyway |
| `attest` takes `cm`, returns `cm_out` | gone | commitment no longer in the statement; also retires the arkworks-vs-circomlib Poseidon gap |
| `attest` returns `r2` | gone | `r2` is a private witness — see below |
| `verify_session` takes `cm` | gone | no commitment to echo |
| `read_invariants.rs` (450 lines) | gone | no read path to pin |
| `k_group`/`k_user` field names | `gsk`/`usk` | matches the prover's `ProveRequest` |
| `get_proving_key` RPC | already absent upstream | kept absent; regression-tested |

Net: `src/` 1,589 → ~1,210 lines, tests 23 → 15 across 4 files.

### `r2` staying private is the quiet win

In the reference, publishing `r2` meant that after attestation any party
holding it could produce syntactically valid ciphertexts — the
commitment-before-disclosure ordering was the only thing preventing fabricated
read data. At IDi level there is no read and no `r2` on the wire, so that
surface is **absent** rather than defended.

## Two defects fixed rather than copied

The reference had two availability defects that would have shipped as-is. Both
were flagged by an audit of the reference, and both are cheap to fix while
writing the file.

**1. Proving ran on the async runtime.** `attest` was `async fn` but called the
blocking ~1.2 s `prove()` directly, occupying a Tokio worker thread for the
duration. `ping` — which is exactly what the liveness and readiness probes poll
— is served by those same workers, so a few concurrent `attest` calls could get
the pod killed mid-proof. Now dispatched via `spawn_blocking`.

**2. No concurrency limit.** jsonrpsee defaults to 100 connections with no
bound on in-flight work. At ~500 MB peak per prove, 100 concurrent calls is a
guaranteed OOMKill, and `attest` is stateless so no card is needed. Now gated
on a `Semaphore` with `MAX_CONCURRENT_PROOFS = 1`; excess requests queue
rather than erroring, so the failure mode is latency.
`concurrent_attest_all_succeed` covers the combination.

The proving key is held as an `Arc` so a handle can move into the blocking
pool. Cloning the `ProvingKey` itself would copy 31 MB per request.

## Protocol break vs the reference

**Unknown params are silently ignored.** jsonrpsee's `#[method]` extracts each
named param individually, so a param the method does not declare — `cm` on
`attest`, `read_spec` on `settle` — is dropped without error.
`#[serde(deny_unknown_fields)]` on the `*Request` structs does *not* help: the
structs are assembled by hand after extraction and never see the raw params
object, so the attribute is a no-op that reads as if it were enforced.

Consequence: a reference-era client that sends `cm` still gets a **valid**
attestation, just with the commitment quietly absent. It fails later, on the
client, looking for `cm_out` in a response that no longer has the field. Both
cases are pinned as tests rather than left as a surprise.

Enforcing it means changing each `#[method]` to take its struct as a single
positional param, converting the wire format from object-form to array-wrapped.
That is a breaking client change and belongs in a version bump, not a silent
patch.

## Known limitations

**No transport authentication and no rate limiting.** The spec assigns client
authentication and rate limiting to a gateway (§11); nothing here enforces
either. `settle_has_no_transport_authentication` pins this as a decision.

It matters more than it looks: the oracle holds the FeliCa master keys and
derives per-card session keys for any `idm` it is asked about, so this service
is a card-key-derivation oracle, and `attest` is additionally a ~1.2 s
CPU-amplifying endpoint. Put a gateway in front of it before exposing it.

**The P0 is still open.** This service does **not** prove card presence.
`l = gsk ⊕ idm` with `idm` an ordinary request parameter, so the oracle is a
complete card simulator. D1–D3 in the circuit pin `l` to the oracle's own key
custody — a real consistency property, not an authority one.
`../prover/README.md` has the full argument and
`../prover/tests/forgery_authority.rs` demonstrates it empirically: a forgery
built from the public proving key with *invented* key material verifies.

`attest` does fail closed on disagreement between the prover and the native
session verifier (`service.rs`) — those were `debug_assert_eq!` in the
reference, compiled out in release, meaning the production image could attest
to a session it had not verified. They are unconditional here.

## Run it

```sh
cargo run --release --example rpc_attest    # prints the env the server needs
```

The example drives the emulated card itself, so the server must be started
with the fixture's derived keys — it prints the exact `FELICA_KEYS_JSON` and
`FELICA_PROVING_KEY_PATH`.

Startup takes ~4 s: the 31 MB key is deserialized before the socket binds, so a
corrupt key fails the deploy rather than the first request.

## Tests

```
cargo build --release                        clean
cargo fmt --check                            clean
cargo clippy --all-targets -- -D warnings    clean
cargo test --release                         15 passed, 0 failed
```

Needs the ceremony key from the prover crate:

```sh
FELICA_TEST_PK=../prover/assets/proving_key.bin cargo test --release
```

`tests/rpc_http.rs` runs a real jsonrpsee server on a loopback port and speaks
raw HTTP/1.1 to it, so the wire format is covered rather than assumed — and it
asserts that batch requests are refused, matching `src/main.rs`.

## Layout

```
src/main.rs            bootstrap; deserializes the key before binding
src/config.rs          FELICA_KEYS_JSON, bind addr, key path
src/params.rs          key file presence/size checks, one load at startup
src/error.rs           JSON-RPC error codes
src/api/handler.rs     trait, Arc'd key cache, proof semaphore
src/api/service.rs     business logic
src/api/types.rs       wire DTOs
src/oracle/mod.rs      key schedule, AUTH2 verification, session verification
src/oracle/schedule.rs challenge schedule only
src/oracle/fixture.rs  shared emulated card (tests + example)
```
