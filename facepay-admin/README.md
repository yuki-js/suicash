# facepay-admin — SuiCash 母艦 管理ソフト(CLI / GUI)

決済端末(Hi-CARA)を母艦 PC から運用する管理ソフト。**起動しただけで全機能が
有効化**され、端末クライアントも adb 経由で自動起動する。CLI と GUI の 2 形態。

いずれも店舗アドレス `FACEPAY_MERCHANT` を付けて起動する(未設定だと決済不可)。

## CLI(`facepay`)

```sh
# ビルド
cargo build --manifest-path usb-poc/Cargo.toml --bin facepay

# 起動(標準入力で pay <SUI> / idle / status / quit)
FACEPAY_MERCHANT=0x… ./usb-poc/target/debug/facepay
```

## GUI(`facepay-admin`)

```sh
# ビルド
cargo build --manifest-path facepay-admin/Cargo.toml

# 起動(デーモンと端末も自動で立ち上がる。ウィンドウを閉じると全部止まる)
FACEPAY_MERCHANT=0x… ./facepay-admin/target/debug/facepay-admin
```
