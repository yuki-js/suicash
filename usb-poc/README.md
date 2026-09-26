# `usb-poc`

A command-line proof of concept: complete FeliCa mutual authentication against a
**real** Suica card over USB, obtain a Groth16 attestation of its IDi from the
oracle, and verify that proof locally.

Linux, RC-S380 over USB, no key material on the client.

```sh
# reader access (once)
echo 'SUBSYSTEM=="usb", ATTRS{idVendor}=="054c", ATTRS{idProduct}=="06c3", GROUP="plugdev", MODE="0664"' \
  | sudo tee /etc/udev/rules.d/60-usb-poc.rules
sudo udevadm control --reload-rules

cargo run --release --manifest-path usb-poc/Cargo.toml -- --help
cargo run --release --manifest-path usb-poc/Cargo.toml
```

Exit codes: `0` verified, `1` operational failure, `2` bad arguments, `3` proof
rejected or a cross-check failed.

## RC-S380 is the Port-100 driver

This trips people up. In `felica` the reader families are separate drivers:

| Reader | USB VID:PID | Driver |
|---|---|---|
| **Sony RC-S380** | `054c:06c1`, `054c:06c3` | `port100` ← **the default here** |
| Sony RC-S300 | `054c:0dc8`, `054c:0dc9`, `054c:0d8f` | `port400` |
| Sony RC-S320 | `054c:01bb` | `rcs320` |
| RC-S330 / S360 / S370 | `054c:02e1`, `054c:0193` | `rcs956` |

`--reader auto` will try all four if you are not sure which you have.

## The PoC holds no secrets

There is no `--gsk` flag, no key file argument, and nothing secret touches disk.
That is not a stylistic choice — it is the point:

> The card is the oracle's verifier.

`challenge` returns `C1A = 3DES(alpha, L, R1)`, and the card independently
recomputes `L = k_group XOR IDm` from its own keys and its own IDm. **If the
oracle's `gsk`/`usk` were wrong, or the IDm were wrong, C1A would be garbage and
the card would simply not answer.** The card accepting C1A *is* the proof that
the oracle holds this card's real keys.

That means the strongest step in the chain is one that **cannot be forged by
anyone holding the proving key**, because it requires a physical card.

## The chain, and what each link is worth

| # | Step | Who checked it | Forgeable? |
|---|---|---|---|
| 1 | Card accepts C1A | the **card** | no — needs the physical card |
| 2 | Oracle accepts C1B, card accepts C2B | both sides | no |
| 3 | AUTH2 MAC valid under R2 | the oracle | no |
| 4 | Groth16 proof verifies against the published `vk` | this process | **yes** — see below |

Links 1–3 are checks a real card performed. Link 4 is a proof that *someone* ran
a correct session.

## The caveat the PoC prints on purpose

Step 4 is **not** evidence that a card was present. Issue #1 in the prover is
open: `gsk`/`usk`/`idm` are unconstrained witnesses, so anyone holding the
**public** proving key can mint an accepted attestation for an arbitrary IDi
with no card, no IDm and no master keys. `prover/tests/forgery_authority.rs`
demonstrates it, and its release-gate assertion is still `#[ignore]`d and red.

So the honest claim this PoC supports is:

> A verified attestation proves *an oracle holding these master keys completed a
> FeliCa mutual authentication with a card and attested the IDi it found*. Given
> links 1–3 also passed in the same run, a real card took part. The proof on its
> own carries no such guarantee.

For a demo whose goal is a working end-to-end story rather than a research
artifact, `prover/README.md` recommends dropping the ZK entirely and having the
oracle Ed25519-sign `{idi, version, expiry}`: no proving key exists to leak,
~250 bytes, sub-millisecond verify.

## What the output shows

- reader identity and chipset, activation bitrate
- `IDm` and `PMm` in cleartext, plus polling optional data
- every system code the card answers to
- per-service **key versions** for the ten Suica services, with the `0000` DES
  key set flagged — this is how you check the oracle is keyed for this card
- the card's own area/service code list
- blocks per service
- the oracle's node path, and the key versions on that exact path
- R1, C1A, C1B, C2A, C2B, and the raw 32-byte AUTH2 ciphertext
- the claimed IDi, `attested_at`, and every proof coordinate
- the three public inputs decoded to their 8-byte limbs, with a canonicality
  check on each
- the pairing result and six cross-checks

## Why the frames are built by hand

`felica` keeps its command serializer private (`felica_standard::command::serialize`
is a private module), and `Authentication2Response::encrypted_payload` is
`pub(crate)`. `FelicaStandard::authentication1` does accept `challenge_1a` as an
argument, but it performs the rest of the session locally with keys this process
does not have. Neither fits a client whose whole point is that the oracle holds
the keys, so every frame is assembled explicitly here.

A fork that makes those internals `pub` is the other option, and is arguably
cleaner — see the note in the handover below.

## Prerequisites

- Linux, a `libusb`-bound RC-S380, and the udev rule above
- an oracle with the card's real FeliCa keys provisioned, whose `system_code`,
  `areas` and `services` describe the node path to the `0x004A` issuance
  service
- `FELICA_TEST_PK` is **not** needed: the PoC only verifies, and
  `get_verifying_key` supplies the `vk`

The key set in the oracle's config must actually be that card's key chain. If it
is not, Authentication1 fails with a wrong response code and the PoC says so
with a hint rather than hanging.

## Troubleshooting

| Symptom | Cause |
|---|---|
| `opening the reader` | udev rule missing, or run as root |
| `no card answered within Ns` | tap the card and hold it flat; try `--reader auto` |
| `card answered 01, expected 11` on Authentication1 | `gsk`/`usk` are not this card's keys, or the node path is wrong. C1A was rejected. |
| `[code] ...` from the oracle | the RPC's JSON-RPC error; the message is the server's |
| exit 3 | the proof did not verify, or a cross-check failed — do not demo this |

Set `RUST_LOG=debug` to get `felica`'s frame-level logging alongside the PoC's
own transcript.

## Status: unverified

This code has **not been compiled**. It was written against the `felica` and
`prover` APIs by reading their source, and the first `cargo check` was aborted
when the build filled the disk. Expect to fix compile errors on the first pass.

It is also a **duplicate**: `/tmp/felica-idi-poc/suica-idi-attest` already
implements this (3,306 lines, with a card emulator, a mock oracle, a
`--self-test` mode that needs no hardware, and Suica block decoders). That
version also patches `felica` to expose the DES challenge algebra instead of
reimplementing the framing, which is the better design. Prefer it, and treat
this crate as a second reference or as scaffolding to be deleted.
