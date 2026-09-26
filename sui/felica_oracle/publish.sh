#!/usr/bin/env bash
# Publishes felica_oracle, creates SuiCash's shared ZK gate, and writes
# usb-poc/facepay/gate.json (the config read by sui-pay.mjs).
#
#   ./publish.sh
#
# Prerequisite: sui client points at the target network with the gate
# operator's address, and that address has gas. Create the gate once and
# reuse it — note that re-publishing starts the new gate with empty dedup
# state (burned r1 values).
#
# Environment variables:
#   SUICASH_GATE_DRIFT  Allowed skew between attested_at and Clock (seconds). Default 900.
#                       Must cover the interval from the tap (oracle issuance
#                       time) to payment finalization.
#   SUICASH_VK          Path to the verifying key. Defaults to the prover's ceremony output.
set -euo pipefail
cd "$(dirname "$0")"

DRIFT="${SUICASH_GATE_DRIFT:-900}"
VK_BIN="${SUICASH_VK:-../../prover/assets/verifying_key.bin}"

[ -f "$VK_BIN" ] || { echo "verifying key not found: $VK_BIN" >&2; exit 1; }
VK_HEX="0x$(xxd -p "$VK_BIN" | tr -d '\n')"

echo "== publish felica_oracle ==" >&2
PUB_JSON=$(sui client publish --json)
PKG=$(echo "$PUB_JSON" | jq -r '.objectChanges[] | select(.type=="published") | .packageId')
[ -n "$PKG" ] && [ "$PKG" != null ] || { echo "publish failed:" >&2; echo "$PUB_JSON" >&2; exit 1; }
echo "package: $PKG" >&2

echo "== create shared gate (drift ${DRIFT}s, allowlist: open) ==" >&2
GATE_JSON=$(sui client call --package "$PKG" --module suicash_gate --function create_gate \
    --args "$VK_HEX" "[]" "$DRIFT" --json)
GATE=$(echo "$GATE_JSON" | jq -r '.objectChanges[]? | select((.objectType // "") | contains("::gate::Gate<")) | .objectId')
[ -n "$GATE" ] && [ "$GATE" != null ] || { echo "gate creation failed:" >&2; echo "$GATE_JSON" >&2; exit 1; }
echo "gate: $GATE" >&2

OUT=../../usb-poc/facepay/gate.json
jq -n --arg p "$PKG" --arg g "$GATE" '{package: $p, gate: $g}' > "$OUT"
echo "wrote $OUT" >&2
jq . "$OUT"
