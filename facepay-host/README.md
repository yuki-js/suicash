# facepay-host — 決済端末の母艦デーモン

Hi-CARA 決済端末の PC 側デーモン(Rust)。FeliCa を読み、オラクルで IDi を認証し、
IDi 導出ウォレットのオンチェーン決済を行い、端末 UI(`../face-auth-ui`)と
WebSocket で連携する。

```
[Suica/PASMO] --RC-S634--> facepay-host --oracle(challenge/settle/attest)--> IDi
                                 │
                                 ├─ sui-pay.mjs: IDi導出ウォレットの残高照会・送金
                                 └─ WebSocket(:8899) <--adb reverse--> Hi-CARA WebView
```

## 構成

- `src/main.rs` — Rust デーモン: FeliCa 読取 + オラクル認証 + WS サーバ + CLI
- `sui-pay.mjs` — Node ヘルパー: IDi→ウォレット導出(regist-web と同一方式)・
  残高照会・送金(`@mysten/sui`)。導出が完全一致することを確認済み

Rust から Node ヘルパーを呼ぶ二層構成にしているのは、ウォレット導出とトランザクション
生成を regist-web(`@mysten/sui`)と厳密に一致させ、アドレス不一致を防ぐため。

## セットアップ

```sh
npm install                 # sui-pay.mjs の依存(@mysten/sui)
cargo build --release       # デーモン
```

必要: Node.js、Rust、Android SDK の adb、libusb。

## 起動

```sh
# 端末(Hi-CARA)から ws://localhost:8899 に届くようにする
adb reverse tcp:8899 tcp:8899
# WebView をこの URL で開く(device-shell / 既存シェル)
#   http://localhost:5173/?ws=ws://localhost:8899   ← face-auth-ui を配信しているURL + ?ws=

FACEPAY_MERCHANT=0x<店舗アドレス> \
FACEPAY_SUI_HELPER=./sui-pay.mjs \
SUI_RPC=https://sui-testnet-rpc.publicnode.com \
  cargo run --release
```

## オペレータ CLI

| コマンド | 動作 |
| --- | --- |
| `idle` | 残高照会モード(顔認証後に残高のみ表示) |
| `pay <SUI>` | 決済待機。顔認証成功で指定額を店舗へ送金し改札LCD風に表示 |
| `testcard <idiHex>` | リーダー無しで擬似カード投入(検証・デモ用) |
| `remove` | 擬似カード離脱 |
| `status` | 現在のモード・カード・端末接続状態 |
| `quit` | 終了 |

## プロトコル(母艦↔端末)

`../face-auth-ui/src/terminal.ts` と対。母艦→端末は WS で JSON イベント
(`card` / `mode` / `paymentResult` / `cardRemoved`)、端末→母艦は
`faceOk` / `faceNg` / `enrolled` / `cancel`。

## 環境変数

| 変数 | 既定 | 用途 |
| --- | --- | --- |
| `FACEPAY_ORACLE` | felica-oracle.ouchiserver… | オラクル JSON-RPC |
| `FACEPAY_WS_PORT` | 8899 | WebSocket ポート |
| `FACEPAY_MERCHANT` | (空) | 決済の送金先(店舗)。未設定だと決済不可 |
| `FACEPAY_SUI_HELPER` | sui-pay.mjs | Node ヘルパーのパス |
| `SUI_RPC` | publicnode testnet | fullnode RPC(ヘルパーへ引継) |

## 登録判定

「オンチェーン登録あり」は、IDi 導出ウォレットに残高 > 0(regist-web でチャージ済み)
を代用している。厳密なレジストリ(Move)にするのは今後の課題。

## 既知の課題: 物理リーダー(RC-S634)

同梱の Sony RC-S634/UA は USB ID `054C:06C2`。felica-rs の Port-100 ドライバが
見ている ID は `06C1`/`06C3` のため、現状 `open_reader` が RC-S634 を開けない
(`testcard` での擬似投入・決済は動作確認済み)。物理カード読取には felica-rs 側の
リーダー ID 対応、または suica-viewer-cli 併用が要る。オラクル認証・オンチェーン
決済・WS・UI は検証済み(実送金 digest 確認済み)。
