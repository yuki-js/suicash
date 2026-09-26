# `sui/felica_oracle` — on-chain FeliCa attestation verifier

Verifies a FeliCa IDi attestation, produced by `../../prover`, directly on
Sui. No conversion layer: the statement is 3 public inputs and the verifying
key is 360 bytes, both of which `sui::groth16` already accepts as-is.

```
sources/felica_auth.move   the wire type; bytes → statement, envelope checks
sources/zk_verifier.move   pinned verifying key, the pairing check
sources/gate.move          the state a stateless check cannot have
sources/suicash_gate.move  SuiCash's deployment: shared gate, Clock, receipt event
tests/fixture.move         @generated — real vk + a real proof
tests/zk_verifier_tests.move
tests/suicash_gate_tests.move
publish.sh                 publish + create the shared gate + write gate.json
```

## The statement

| PI | Content | Bytes |
|---|---|---|
| `pi0` | `idi` — the card identifier, the sole identity claim | 8 LE |
| `pi1` | `r1` — the holder's fresh session challenge | 8 LE |
| `pi2` | `attested_at` — prover-chosen, drift signal only | 8 LE |

Each is a 32-byte little-endian scalar; the upper 24 bytes must be zero, and
`felica_auth` enforces that. Everything else — `c1b`, `c2a`, `auth2`, `r2` and
the master keys `gsk`/`usk`/`idm` — is a private witness constrained in
circuit. A verifier learns which card was claimed and which challenge it was
bound to, and nothing else.

## Usage

```move
use felica_oracle::gate::{Self, Gate};
use felica_oracle::zk_verifier;

// once, at publish time: pin the key the deployment trusts
let gate = gate::create<MyWitness>(vk_bytes, allowed_idis, 300, ctx);
transfer::share_object(gate);

// per presentation
let idi = gate::verify_and_claim(&mut gate, &att, presented_r1, now_seconds());
```

`verify_and_claim` returns the claimed IDi. It aborts rather than returning
`false`, so a caller cannot forget to check.

## The SuiCash deployment (`suicash_gate`)

`gate` and `zk_verifier` are libraries; `suicash_gate` is the deployment: it
pins the witness type (`SUICASH`), shares one `Gate<SUICASH>`, feeds the drift
check from `sui::clock::Clock` instead of a payer-chosen number, and emits an
`AttestationVerified` receipt event.

Deploy with `./publish.sh` (publishes the package, creates the shared gate,
writes `usb-poc/facepay/gate.json` for the payment helper). Per payment, the
facepay daemon builds one PTB:

```
0: suicash_gate::verify(gate, idi, attested_at, proof, public_inputs, r1, Clock)
1: SplitCoins(gas, amount)
2: TransferObjects([coin], merchant)
```

PTB commands are atomic, so the transfer cannot execute unless the Groth16
proof verified on chain and the `r1` burn succeeded in the same transaction.
One attestation authorises exactly one payment; the next payment needs a new
card tap.

Order inside the gate is deliberate: drift and allowlist first (an
unauthorised caller cannot use the call as a free pairing oracle), then the
challenge binding and the replay check, then the pairing, then the `r1` burn
last — a rejected presentation must not invalidate a legitimate holder's
pending challenge.

## What a verified attestation does *not* mean

Read `prover/README.md` before deploying this. Issue #1 is open: the circuit
derives the key schedule from `gsk`/`usk`/`idm`, but those are unconstrained
witnesses, so a party holding the public proving key can mint an accepted
attestation with no card, no `IDm` and no master keys.

So a verified attestation is **not** evidence that a card was present, **not**
evidence of which oracle produced it, and `attested_at` is **not** time. This
package verifies a proof; it cannot supply a property the statement does not
contain.

What it *does* give you, given a secret proving key: someone who knows the
FeliCa system master keys for this key version computed a transcript that is
consistent with the claimed IDi, and the public inputs cannot be swapped after
the fact.

## The fixture

`tests/fixture.move` is generated, not hand-written:

```sh
cargo run --release --manifest-path prover/Cargo.toml --bin felica-fixture -- \
    prover/assets/proving_key.bin sui/felica_oracle/tests/fixture.move
sui move test
```

It carries a real `vk` and a real proof, so `genuine_attestation_verifies` is a
live pairing check against the ceremony key rather than a shape test. Proofs
are randomized, so the hex changes on every run — regenerate, do not diff. The
fixture and the key must be regenerated together: `pinned_key_is_the_fixture_key`
is the test that notices when they drift apart.

## Tests

25 tests, and the negative cases are the point — each one mutates exactly one
thing and requires a specific abort code:

| Mutation | Rejected by |
|---|---|
| mutated verifying key | `zk_verifier` |
| altered `pi0` | envelope check |
| altered `pi1` (no envelope field mirrors it) | the pairing |
| envelope lying about `idi` / `attested_at` | envelope check |
| non-canonical scalar (byte set above the limb) | canonicality rule |
| unknown `alg` label | `felica_auth` |
| replayed `r1` | `gate` dedup store |
| foreign `r1` | challenge binding |
| disallowed IDi | allowlist |
| clock drift beyond policy | drift check |
| removing the last allowlist entry | cannot widen the gate |

Note the two-stage rejection of a bad IDi: rewriting only the envelope is
caught by the envelope check, while altering `pi0` itself gets that far and is
caught by the pairing. Both are required, and both are pinned.
