# `prover` — IDi-level FeliCa attestation

A reimplementation of the FeliCa DES oracle prover, reduced to a single
identity claim.

`../ref-impl-of-oracle/` is the **reference implementation**: frozen, kept
green, and deliberately *not* modified. This crate is the new thing. The two
are meant to be diffed:

```sh
diff -u ../ref-impl-of-oracle/prover/src/circuit.rs src/circuit.rs
```

## The statement

Public inputs — 3 BN254 field elements, well under Sui's cap of 8:

| PI | Content | Bytes |
|---|---|---|
| `pi0` | `idi` — the card identifier, the sole identity claim | 8 LE |
| `pi1` | `r1` — the holder's fresh session challenge | 8 LE |
| `pi2` | `attested_at` — prover-chosen, drift signal only | 8 LE |

Private witness: `gsk`, `usk`, `idm`, `c1b`, `c2a`, `r2`, `auth2`.

Constraints **D1–D3** (new, the point of this crate) derive the FeliCa key
schedule *inside* the circuit:

```
D1: l     = gsk XOR idm
D2: alpha = DES_key(l)_data(usk)
D3: beta  = DES_key(alpha)_data(l)
```

Constraints **C1–C6** (carried over unchanged) enforce transcript consistency:
`3DES(l,β,r1) == c1b`, `3DES(l,β,r2) == c2a`, `auth2` CBC-decrypts under `r2`
to a plaintext with a valid 8-byte MAC under opcode `0x13`, TID `== tail6(r1)`,
and `idi` matches.

In English: *someone who knows the system master keys computed this, and the
transcript is arithmetically consistent with the claimed `idi`.*

## What changed, and what did not

| | reference | here |
|---|---|---|
| Public inputs | 8 | **3** |
| Constraints | 129,222 | **149,058** |
| Instance variables | 9 | **4** (3 + ONE) |
| `verifying_key.bin` | 520 B | **360 B** (= 264 + 3×32) |
| Poseidon commitment | in the statement | **removed** |
| `commit.rs` | present | **removed** |

Dropping the commitment is what retires the arkworks-vs-circomlib Poseidon
incompatibility. The reference's `docs/spec.md` claims native Sui Move can
reproduce the commitment digest, but `commit.rs` there admits the `ark`/`mds`
round constants are arkworks-internal and were never exported — so the claim
was unimplementable off-Rust. With no commitment in the statement, the
`prover` crate needs only what `sui::groth16` already accepts:

- 32-byte little-endian public-input scalars, ≤ 8 of them
- an Arkworks *canonical compressed* verifying key, passed straight to
  `prepare_verifying_key`

The 360-byte `vk` is exactly Sui's BN254 size for `n = 3`. No conversion.

`../sui/felica_oracle/` is the consumer: a Move package that pins the `vk` and
verifies proofs with `sui::groth16`. `prove_compressed` exists for it — the RPC
surface hex-encodes coordinates one at a time, but Move wants the Arkworks
compressed proof as one blob. Same proof object, second serialization; there is
still only one proving path.

Also gone: the read path. `auth2` already carries `idi`, so the single-block
`Read`, the encrypted response, and `R2` publication all drop out of the
statement. `R2` is now a private witness — nobody can decrypt the read
payload, including the verifier, which is the intended outcome at IDi level.

## ⚠ The P0 is still open

**D1–D3 did not fix the forgery, and the test suite proves it.**

`l` is defined as `gsk XOR idm`, so an attacker without the real master keys
simply *invents* `gsk`, `usk` and `idm`. D1–D3 then derive `l`, `alpha`,
`beta` consistently and the witness is as valid as an honest one. The set of
satisfying assignments is unchanged from the reference; only the
parameterisation moved from `{l, beta}` to `{gsk, usk, idm}`.

Demonstrated by `tests/forgery_authority.rs`:

```
$ cargo test --release --test forgery_authority -- --ignored
issue #1: a forgery built from the PUBLIC proving key, with invented
key material and no card, must not verify: ()
```

That test is `#[ignore]`d, exactly as in the reference. It is the release
gate and it is **still red**. Do not represent this crate as closing issue #1.

What D1–D3 do buy is narrower and worth stating precisely:

1. The oracle can no longer pass an `l`/`beta` that disagrees with its own key
   custody — the two derivations are pinned together in-circuit.
2. The statement is self-documenting: it names the inputs the session keys
   come from, instead of leaving `l`/`beta` as unexplained witnesses.

Neither is an anti-forgery property. `keys_are_not_pinned_to_a_verifier_known_value`
exists so this cannot be quietly forgotten.

### What closing it requires

A value the attacker cannot choose. Concretely, publish a commitment to the
key material as a **fourth public input** and prove knowledge of its
preimage:

```
pi3 = H(gsk)          with H in-circuit, and pi3 pinned to a published value
```

`gsk` then cannot be invented: the forger would need the real key's hash
preimage. This is reference spec §10.2 options 1–2, and it changes the
circuit, so it requires regenerating both keys and re-pinning the `vk`.

Note that pinning `H(gsk)` in a *document* rather than as a public input buys
nothing — the whole point is that the constraint and the value must meet
inside the R1CS.

There is a cheaper option for a demo, and it is not a proof: have the oracle
sign `{idi, version, expiry}` with Ed25519. ~250 bytes, sub-millisecond
verify, and **no proving key exists to leak**, which retires the "public
proving key is sufficient" problem outright. If the goal is a working
demonstration rather than a research artifact, that dominates this crate on
every axis except being a ZKP.

## Status

```
cargo build --release                        # clean
cargo fmt --check                            # clean
cargo clippy --all-targets -- -D warnings    # clean
cargo test --release                         # 42 passed, 0 failed, 1 ignored
sui move test --path ../sui/felica_oracle    # 25 passed, 0 failed
```

Tests need a ceremony proving key:

```sh
cargo build --release --bin felica-setup
./target/release/felica-setup assets --force
FELICA_TEST_PK=assets/proving_key.bin cargo test --release
```

`assets/proving_key.bin` is a **development-grade** trusted setup: single-party
`circuit_specific_setup` seeded from `OsRng`, no ceremony transcript, no
evidence of toxic-waste destruction. It is not a multi-party ceremony and
should not be described as one. It is also public material — which is fine
here, because as established above it is not the thing standing between an
attacker and a forgery.

## Layout

```
src/abi.rs       3-field packing; the single bytes <-> field mapping
src/circuit.rs   DES gadgets + D1-D3 + C1-C6
src/des.rs       native DES/3DES/CBC/MAC, shared tables, differential-tested
src/setup.rs     key load/encode; never generates on the proving path
src/bin/         felica-setup (ceremony), felica-keys (provisioning),
                 felica-fixture (regenerates the Move test fixture)
tests/           attest_prove, circuit_robust, forgery_authority
```

`felica-fixture` writes `../sui/felica_oracle/tests/fixture.move`: the real
`vk` plus a real proof, so the Move tests run a live pairing against the
ceremony key instead of a shape check. It is generated rather than
hand-copied for the reason the `cm` limb was dropped — a stale blob that no
longer matches the key still "verifies" nothing, and a test that has quietly
stopped verifying anything looks exactly like a passing test.

`setup.rs` and the two binaries are near-verbatim from the reference; the
substantive changes are confined to `abi.rs`, `circuit.rs` and `lib.rs`.

# Attribution

This submodule is partly based on my previous work. 
https://github.com/yuki-js/felica-oracle

But it has been rewritten and modified to fit the Sui Move environment, with a focus on reducing public inputs and improving the prover's efficiency. The original implementation served as a reference point for understanding the FeliCa DES oracle prover, but this crate represents a new approach to attestation at the IDi level.

The previous work has finished before the hackathon starts. And it is not a part of the hackathon submission. It's just for proving the concept of "Is FeliCa's mutual authentication protocol given as a zkp constraint system?" .