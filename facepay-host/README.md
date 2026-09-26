# facepay-host — host PC daemon for the payment terminal

PC-side daemon (Rust) for the Hi-CARA payment terminal. It reads FeliCa, authenticates the
IDi via the oracle, performs on-chain payments from the IDi-derived wallet, and talks to the
terminal UI (`../face-auth-ui`) over WebSocket.

```
[Suica/PASMO] --RC-S634--> facepay-host --oracle(challenge/settle/attest)--> IDi
                                 │
                                 ├─ sui-pay.mjs: balance query / transfer for the IDi-derived wallet
                                 └─ WebSocket(:8899) <--adb reverse--> Hi-CARA WebView
```

## Layout

- `src/main.rs` — Rust daemon: FeliCa reading + oracle authentication + WS server + CLI
- `sui-pay.mjs` — Node helper: IDi → wallet derivation (same scheme as regist-web),
  balance query and transfer (`@mysten/sui`). Derivation verified to match exactly.

The two-layer design (Rust calling a Node helper) keeps wallet derivation and transaction
building strictly identical to regist-web (`@mysten/sui`), preventing address mismatches.

## Setup

```sh
npm install                 # sui-pay.mjs dependency (@mysten/sui)
cargo build --release       # daemon
```

Requires: Node.js, Rust, adb from the Android SDK, libusb.

## Running

```sh
# Make ws://localhost:8899 reachable from the terminal (Hi-CARA)
adb reverse tcp:8899 tcp:8899
# Open the WebView at this URL (device-shell / existing shell)
#   http://localhost:5173/?ws=ws://localhost:8899   ← URL serving face-auth-ui + ?ws=

FACEPAY_MERCHANT=0x<merchant address> \
FACEPAY_SUI_HELPER=./sui-pay.mjs \
SUI_RPC=https://sui-testnet-rpc.publicnode.com \
  cargo run --release
```

## Operator CLI

| Command | Action |
| --- | --- |
| `idle` | Balance mode (show balance only after face authentication) |
| `pay <SUI>` | Wait for payment. On successful face auth, send the amount to the merchant and show it gate-LCD style |
| `testcard <idiHex>` | Insert a simulated card without a reader (testing/demo) |
| `remove` | Remove the simulated card |
| `status` | Current mode, card, and terminal connection state |
| `quit` | Exit |

## Protocol (host PC ↔ terminal)

Counterpart of `../face-auth-ui/src/terminal.ts`. Host → terminal sends JSON events over WS
(`card` / `mode` / `paymentResult` / `cardRemoved`); terminal → host sends
`faceOk` / `faceNg` / `enrolled` / `cancel`.

## Environment variables

| Variable | Default | Purpose |
| --- | --- | --- |
| `FACEPAY_ORACLE` | felica-oracle.ouchiserver… | Oracle JSON-RPC |
| `FACEPAY_WS_PORT` | 8899 | WebSocket port |
| `FACEPAY_MERCHANT` | (empty) | Payment recipient (merchant). Payments disabled if unset |
| `FACEPAY_SUI_HELPER` | sui-pay.mjs | Path to the Node helper |
| `SUI_RPC` | publicnode testnet | Fullnode RPC (passed to the helper) |
| `SUICASH_GATE_PKG` | (gate.json) | `felica_oracle` package ID (takes precedence over gate.json) |
| `SUICASH_GATE_OBJ` | (gate.json) | Shared `Gate<SUICASH>` object ID (likewise) |

## On-chain ZK verification

Payments require the oracle's Groth16 proof. On each card tap the daemon gets an
`attest` result, converts the coordinate-form proof to the Arkworks compressed
bytes `sui::groth16` accepts, and keeps the resulting attestation JSON. On
`faceOk` in payment mode it passes that JSON to `sui-pay.mjs` via
`SUICASH_ATTESTATION`; the helper `moveCall`s
`felica_oracle::suicash_gate::verify` at the head of the same PTB as the coin
split and transfer. PTBs are atomic, so if the proof does not verify the whole
transfer aborts — there is no payment path without a proof.

The proof is bound to a fresh `r1` (drawn per session) and the on-chain gate
burns it, so an attestation is single-use: the next payment requires another
card tap. The gate lives in `gate.json` (written by
`sui/felica_oracle/publish.sh`) or in `SUICASH_GATE_PKG` / `SUICASH_GATE_OBJ`.

`testcard` inserts a simulated card with no oracle attestation, so it can show a
balance but **payments are refused** until a real card is tapped. Existing,
already-burned `r1` values also abort (`gate::EReplay`) — re-tap to get a fresh
proof.

## Registration check

"Registered on-chain" is approximated by the IDi-derived wallet having a balance > 0
(topped up via regist-web). A proper registry (Move) is future work.

## Known issue: physical reader (RC-S634)

The bundled Sony RC-S634/UA has USB ID `054C:06C2`. felica-rs's Port-100 driver
looks for `06C1`/`06C3`, so `open_reader` currently cannot open the RC-S634
(simulated insertion and payment via `testcard` are verified). Reading physical cards needs
reader ID support in felica-rs, or using suica-viewer-cli alongside. Oracle authentication,
on-chain payment, WS, and UI are verified (real transfer digest confirmed).
