# facepay-admin — SuiCash host PC admin tool (CLI / GUI)

Admin tool for operating the payment terminal (Hi-CARA) from the host PC. **Just launching it
enables every feature**, and the terminal client is auto-started via adb as well. Comes in two
forms: CLI and GUI.

Either form is launched with the merchant address `FACEPAY_MERCHANT` (payments are disabled if unset).

## CLI (`facepay`)

```sh
# Build
cargo build --manifest-path usb-poc/Cargo.toml --bin facepay

# Run (stdin commands: pay <SUI> / idle / status / quit)
FACEPAY_MERCHANT=0x… ./usb-poc/target/debug/facepay
```

## GUI (`facepay-admin`)

```sh
# Build
cargo build --manifest-path facepay-admin/Cargo.toml

# Run (the daemon and terminal start automatically; closing the window stops everything)
FACEPAY_MERCHANT=0x… ./facepay-admin/target/debug/facepay-admin
```
