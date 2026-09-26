# オラクルを Debian で動かす + cloudflared で公開

FeliCa オラクル(`server/` クレート)を Debian サーバで常駐させ、
Cloudflare Tunnel(cloudflared)で HTTPS 公開する手順。オラクルは
127.0.0.1 のみで待受し、ポートは直接開けず cloudflared だけを出口にする。

```
[Suica] ⇄ 母艦(usb-poc/facepay-host) ──HTTPS──> Cloudflare ──tunnel──> cloudflared ──> 127.0.0.1:3000 (oracle)
```

## 0. clone(usb-poc ブランチ)

```sh
sudo mkdir -p /opt/suicash && sudo chown "$USER" /opt/suicash
git clone -b usb-poc --single-branch https://github.com/yuki-js/suicash.git /opt/suicash
cd /opt/suicash
```

`prover/assets/proving_key.bin`(31MB)と `usb-poc/vendor/felica` も含まれる
(git-lfs 不要)。プライベートなら `gh auth login` か token 付き URL を使う。

## 1. ビルド(Rust)

```sh
sudo apt update && sudo apt install -y build-essential pkg-config curl git
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
. "$HOME/.cargo/env"
cargo build --release --manifest-path server/Cargo.toml --bin oracle
# → server/target/release/oracle
```

## 2. 鍵(FELICA_KEYS_JSON)を配置

**これがカードごとの秘密**。実カード用は今のオラクルと同じ gsk/usk/ノードパス
が必要(担当者から値を取得、または `keys.jsonl` から
`cargo run -p felica-prover --bin felica-keys` で再生成)。リポジトリには入れない。

```sh
sudo mkdir -p /etc/felica-oracle
sudo tee /etc/felica-oracle/keys.json >/dev/null <<'JSON'
{"gsk":"<16hex>","usk":"<16hex>","system_code":3,"areas":[0,64,2048,4032,4096],"services":[74]}
JSON
# サービス実行ユーザー(felica)が読めるよう所有者を合わせる。
# root 所有 + mode 600 のままだと "Permission denied (os error 13)" で起動失敗する。
sudo chown felica:felica /etc/felica-oracle/keys.json
sudo chmod 600 /etc/felica-oracle/keys.json
```

> /opt/suicash を root で clone した場合、felica ユーザーが proving key や
> バイナリを読めず同じ 13 が出ることがある。その時は:
> `sudo chmod -R a+rX /opt/suicash` (少なくとも prover/assets と server/target/release)。

動作確認だけなら fixture 鍵(`server/src/oracle/fixture.rs`。`rpc_attest` 例が
正確な JSON を出力)を使う。

## 3. オラクルを常駐(systemd)

```sh
sudo useradd -r -s /usr/sbin/nologin felica 2>/dev/null || true
sudo cp server/deploy/felica-oracle.service /etc/systemd/system/
# ユニット内の User / パスを環境に合わせて調整
sudo systemctl daemon-reload
sudo systemctl enable --now felica-oracle
# 確認
curl -s -X POST http://127.0.0.1:3000 -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"ping","params":{}}'   # → "pong"
sudo journalctl -u felica-oracle -f
```

> ⚠ 証明生成(attest)は 1 回 ~500MB。メモリ不足だと attest でプロセスが落ち、
> ping ごと無応答(502/524)になる。ユニットの `MemoryMax` と実メモリを確認。

## 4. cloudflared で公開

```sh
# インストール(Debian amd64)
curl -L https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-amd64.deb -o /tmp/cloudflared.deb
sudo dpkg -i /tmp/cloudflared.deb

cloudflared tunnel login                      # ブラウザで Cloudflare アカウント認可
cloudflared tunnel create felica-oracle       # → <TUNNEL_ID> と /root/.cloudflared/<ID>.json

# 設定を配置(example を編集: TUNNEL_ID / hostname / credentials-file)
sudo mkdir -p /etc/cloudflared
sudo cp server/deploy/cloudflared-config.example.yml /etc/cloudflared/config.yml
sudo cp ~/.cloudflared/<TUNNEL_ID>.json /etc/cloudflared/
sudo $EDITOR /etc/cloudflared/config.yml       # <TUNNEL_ID> と hostname を実値に

# DNS ルート(サブドメインをトンネルへ)
cloudflared tunnel route dns felica-oracle felica-oracle.example.com

# 常駐サービス化(config.yml を使う)
sudo cloudflared service install
sudo systemctl enable --now cloudflared
```

公開 URL は `https://felica-oracle.example.com`。動作確認:

```sh
curl -s -X POST https://felica-oracle.example.com -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"ping","params":{}}'   # → "pong"
```

## 5. クライアントを新オラクルへ向ける

```sh
# usb-poc
./usb-poc/target/release/usb-poc --oracle https://felica-oracle.example.com
# facepay-host(決済端末デーモン)
FACEPAY_ORACLE=https://felica-oracle.example.com/ cargo run --release ...
```

## トラブルシュート

- `ping` は 200 だが `attest` で 502/524 → 証明生成でプロセスが落ちている。
  メモリ増設 / `MemoryMax` 緩和 / `journalctl -u felica-oracle` で OOM・panic 確認。
- cloudflared 側 524(タイムアウト)→ Cloudflare エッジは最大 100 秒。
  証明は数秒なので、524 が出るならバックエンドが無応答(上と同じ)。
- `challenge` は通るが `settle` が失敗 → 鍵(gsk/usk)がカードと不一致。
