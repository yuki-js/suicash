#!/usr/bin/env bash
# Publishes felica_oracle, creates SuiCash's shared ZK gate, and writes
# usb-poc/facepay/gate.json (the config read by sui-pay.mjs).
#
#   ./publish.sh
#
# Gate administration is gated by OpenZeppelin access control: publishing runs
# this package's `init`, which shares an `AccessControl<SUICASH_GATE>` registry
# with the publisher as root admin (and operator, so the steps below work
# without an extra grant). Creating the gate then requires a root `Auth`,
# minted and spent in the same PTB via the `new_root_auth` helper.
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

# The shared access-control registry created by `init` during publish.
REGISTRY=$(echo "$PUB_JSON" | jq -r '.objectChanges[]? | select((.objectType // "") | contains("access_control::AccessControl")) | .objectId')
[ -n "$REGISTRY" ] && [ "$REGISTRY" != null ] || { echo "registry not found in publish output:" >&2; echo "$PUB_JSON" >&2; exit 1; }
echo "registry: $REGISTRY" >&2

echo "== create shared gate (drift ${DRIFT}s, allowlist: open) ==" >&2
GATE_JSON=$(sui client ptb \
    --make-move-vec "<vector<u8>>" "[]" --assign allowlist \
    --move-call "$PKG::suicash_gate::new_root_auth" "@$REGISTRY" --assign root_auth \
    --move-call "$PKG::suicash_gate::create_gate" "root_auth" "$VK_HEX" "allowlist" "$DRIFT" \
    --json)
GATE=$(echo "$GATE_JSON" | jq -r '.objectChanges[]? | select((.objectType // "") | contains("::gate::Gate<")) | .objectId')
[ -n "$GATE" ] && [ "$GATE" != null ] || { echo "gate creation failed:" >&2; echo "$GATE_JSON" >&2; exit 1; }
echo "gate: $GATE" >&2

OUT=../../usb-poc/facepay/gate.json
jq -n --arg p "$PKG" --arg g "$GATE" --arg r "$REGISTRY" '{package: $p, gate: $g, registry: $r}' > "$OUT"
echo "wrote $OUT" >&2
jq . "$OUT"
