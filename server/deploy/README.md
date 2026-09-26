# Running the oracle on Debian + exposing it with cloudflared

Steps to run the FeliCa oracle (`server/` crate) as a service on a Debian server and
expose it over HTTPS via Cloudflare Tunnel (cloudflared). The oracle listens on
127.0.0.1 only; no ports are opened directly and cloudflared is the only way out.

```
[Suica] ⇄ host PC (usb-poc/facepay-host) ──HTTPS──> Cloudflare ──tunnel──> cloudflared ──> 127.0.0.1:3000 (oracle)
```

## 0. Clone (usb-poc branch)

```sh
sudo mkdir -p /opt/suicash && sudo chown "$USER" /opt/suicash
git clone -b usb-poc --single-branch https://github.com/yuki-js/suicash.git /opt/suicash
cd /opt/suicash
```

`prover/assets/proving_key.bin` (31MB) and `usb-poc/vendor/felica` are included
(no git-lfs needed). If the repo is private, use `gh auth login` or a URL with a token.

## 1. Build (Rust)

```sh
sudo apt update && sudo apt install -y build-essential pkg-config curl git
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
. "$HOME/.cargo/env"
cargo build --release --manifest-path server/Cargo.toml --bin oracle
# → server/target/release/oracle
```

## 2. Install the keys (FELICA_KEYS_JSON)

**These are the per-card secrets.** Real cards need the same gsk/usk/node paths as the
current oracle (get the values from the maintainer, or regenerate them from `keys.jsonl`
with `cargo run -p felica-prover --bin felica-keys`). Never commit them to the repository.

```sh
sudo mkdir -p /etc/felica-oracle
sudo tee /etc/felica-oracle/keys.json >/dev/null <<'JSON'
{"gsk":"<16hex>","usk":"<16hex>","system_code":3,"areas":[0,64,2048,4032,4096],"services":[74]}
JSON
# Make the file owned by the service user (felica) so it can read it.
# Left as root-owned + mode 600, startup fails with "Permission denied (os error 13)".
sudo chown felica:felica /etc/felica-oracle/keys.json
sudo chmod 600 /etc/felica-oracle/keys.json
```

> If /opt/suicash was cloned as root, the felica user may be unable to read the proving key
> or binary and hit the same error 13. In that case:
> `sudo chmod -R a+rX /opt/suicash` (at least prover/assets and server/target/release).

For a quick smoke test, use the fixture keys (`server/src/oracle/fixture.rs`; the
`rpc_attest` example prints the exact JSON).

## 3. Run the oracle as a service (systemd)

```sh
sudo useradd -r -s /usr/sbin/nologin felica 2>/dev/null || true
sudo cp server/deploy/felica-oracle.service /etc/systemd/system/
# Adjust User / paths in the unit to your environment
sudo systemctl daemon-reload
sudo systemctl enable --now felica-oracle
# Check
curl -s -X POST http://127.0.0.1:3000 -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"ping","params":{}}'   # → "pong"
sudo journalctl -u felica-oracle -f
```

> ⚠ Proof generation (attest) takes ~500MB per run. Without enough memory the process dies
> during attest and even ping stops responding (502/524). Check the unit's `MemoryMax` and
> actual memory.

## 4. Expose with cloudflared

```sh
# Install (Debian amd64)
curl -L https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-amd64.deb -o /tmp/cloudflared.deb
sudo dpkg -i /tmp/cloudflared.deb

cloudflared tunnel login                      # authorize your Cloudflare account in a browser
cloudflared tunnel create felica-oracle       # → <TUNNEL_ID> and /root/.cloudflared/<ID>.json

# Install the config (edit the example: TUNNEL_ID / hostname / credentials-file)
sudo mkdir -p /etc/cloudflared
sudo cp server/deploy/cloudflared-config.example.yml /etc/cloudflared/config.yml
sudo cp ~/.cloudflared/<TUNNEL_ID>.json /etc/cloudflared/
sudo $EDITOR /etc/cloudflared/config.yml       # set <TUNNEL_ID> and hostname to real values

# DNS route (point the subdomain at the tunnel)
cloudflared tunnel route dns felica-oracle felica-oracle.example.com

# Install as a service (uses config.yml)
sudo cloudflared service install
sudo systemctl enable --now cloudflared
```

The public URL is `https://felica-oracle.example.com`. To check:

```sh
curl -s -X POST https://felica-oracle.example.com -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"ping","params":{}}'   # → "pong"
```

## 5. Point clients at the new oracle

```sh
# usb-poc
./usb-poc/target/release/usb-poc --oracle https://felica-oracle.example.com
# facepay-host (payment terminal daemon)
FACEPAY_ORACLE=https://felica-oracle.example.com/ cargo run --release ...
```

## Troubleshooting

- `ping` returns 200 but `attest` gives 502/524 → the process is dying during proof generation.
  Add memory / relax `MemoryMax` / check `journalctl -u felica-oracle` for OOM or panic.
- 524 (timeout) from cloudflared → the Cloudflare edge allows at most 100 seconds.
  Proofs take a few seconds, so a 524 means the backend is unresponsive (same as above).
- `challenge` succeeds but `settle` fails → the keys (gsk/usk) don't match the card.
