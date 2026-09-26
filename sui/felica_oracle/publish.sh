#!/usr/bin/env bash
# felica_oracle を publish し、SuiCash の共有 ZK ゲートを作成して
# usb-poc/facepay/gate.json(sui-pay.mjs が読む設定)を書き出す。
#
#   ./publish.sh
#
# 前提: sui client がゲート運用者のアドレスで対象ネットワークに向いており、
# ガスを持っていること。ゲートは一度作れば使い回す — 再 publish すると
# dedup 状態(burn 済み r1)が新しいゲートでは空になることに注意。
#
# 環境変数:
#   SUICASH_GATE_DRIFT  attested_at と Clock の許容差(秒)。既定 900。
#                       タップ(オラクルの発行時刻)から決済確定までの間隔を
#                       覆う必要がある。
#   SUICASH_VK          verifying key のパス。既定は prover のセレモニー出力。
set -euo pipefail
cd "$(dirname "$0")"

DRIFT="${SUICASH_GATE_DRIFT:-900}"
VK_BIN="${SUICASH_VK:-../../prover/assets/verifying_key.bin}"

[ -f "$VK_BIN" ] || { echo "verifying key が見つかりません: $VK_BIN" >&2; exit 1; }
VK_HEX="0x$(xxd -p "$VK_BIN" | tr -d '\n')"

echo "== publish felica_oracle ==" >&2
PUB_JSON=$(sui client publish --json)
PKG=$(echo "$PUB_JSON" | jq -r '.objectChanges[] | select(.type=="published") | .packageId')
[ -n "$PKG" ] && [ "$PKG" != null ] || { echo "publish 失敗:" >&2; echo "$PUB_JSON" >&2; exit 1; }
echo "package: $PKG" >&2

echo "== create shared gate (drift ${DRIFT}s, allowlist: open) ==" >&2
GATE_JSON=$(sui client call --package "$PKG" --module suicash_gate --function create_gate \
    --args "$VK_HEX" "[]" "$DRIFT" --json)
GATE=$(echo "$GATE_JSON" | jq -r '.objectChanges[]? | select((.objectType // "") | contains("::gate::Gate<")) | .objectId')
[ -n "$GATE" ] && [ "$GATE" != null ] || { echo "gate 作成失敗:" >&2; echo "$GATE_JSON" >&2; exit 1; }
echo "gate: $GATE" >&2

OUT=../../usb-poc/facepay/gate.json
jq -n --arg p "$PKG" --arg g "$GATE" '{package: $p, gate: $g}' > "$OUT"
echo "wrote $OUT" >&2
jq . "$OUT"
