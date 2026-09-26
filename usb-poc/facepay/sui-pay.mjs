#!/usr/bin/env node
/**
 * 母艦の Sui 決済ヘルパー。
 * IDi から決定的にウォレットを導出(regist-web と同一方式)し、残高照会・送金を行う。
 * facepay デーモンからサブコマンドで呼ばれ、JSON を1行返す。
 *
 *   node sui-pay.mjs address <idiHex>
 *   node sui-pay.mjs balance <idiHex>
 *   node sui-pay.mjs pay     <idiHex> <amountMist> <merchantAddr>
 *   node sui-pay.mjs verify  <idiHex>            (ZK検証のみ。疎通確認用)
 *
 * 決済はゼロ知識証明の検証を必須とする。`pay` は同一 PTB の先頭で
 * felica_oracle::suicash_gate::verify を moveCall し、オラクルの Groth16 証明が
 * オンチェーンで通らなければ送金コマンドごとアボートする。証明なしの送金経路は
 * 存在しない。
 *
 * 環境変数:
 *   SUI_RPC              fullnode JSON-RPC(既定は publicnode の testnet)
 *   SUICASH_GATE_PKG     felica_oracle パッケージ ID(gate.json より優先)
 *   SUICASH_GATE_OBJ     共有 Gate<SUICASH> オブジェクト ID(同上)
 *   SUICASH_ATTESTATION  attestation JSON(facepay デーモンが注入):
 *     { "idi": hex8, "attested_at": unixSec, "r1": hex8,
 *       "proof": hexCompressed, "public_inputs": hex96 }
 *
 * gate.json(このファイルと同じディレクトリ、publish スクリプトが生成):
 *   { "package": "0x…", "gate": "0x…" }
 *
 * ⚠ 導出方式は regist-web/src/lib/wallet.ts と厳密に一致させること:
 *   seed = SHA-256("suicash-aa-wallet:v1:" || idiBytes(8)) → Ed25519 秘密鍵
 */
import { Ed25519Keypair } from "@mysten/sui/keypairs/ed25519";
import { SuiClient } from "@mysten/sui/client";
import { Transaction } from "@mysten/sui/transactions";
import { webcrypto } from "node:crypto";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const RPC = process.env.SUI_RPC || "https://sui-testnet-rpc.publicnode.com";
const SEED_DOMAIN = "suicash-aa-wallet:v1:";

function hexToBytes(h) {
  const s = h.trim().toLowerCase().replace(/[^0-9a-f]/g, "");
  const out = new Uint8Array(s.length / 2);
  for (let i = 0; i < out.length; i++) out[i] = parseInt(s.slice(i * 2, i * 2 + 2), 16);
  return out;
}

async function deriveKeypair(idiHex) {
  const idiBytes = hexToBytes(idiHex);
  const domain = new TextEncoder().encode(SEED_DOMAIN);
  const input = new Uint8Array(domain.length + idiBytes.length);
  input.set(domain, 0);
  input.set(idiBytes, domain.length);
  const seed = new Uint8Array(await webcrypto.subtle.digest("SHA-256", input));
  return Ed25519Keypair.fromSecretKey(seed);
}

/** ZK ゲートの所在。env 優先、なければ隣の gate.json。無ければ null。 */
function gateConfig() {
  const pkg = process.env.SUICASH_GATE_PKG;
  const gate = process.env.SUICASH_GATE_OBJ;
  if (pkg && gate) return { package: pkg, gate };
  try {
    const here = dirname(fileURLToPath(import.meta.url));
    const j = JSON.parse(readFileSync(join(here, "gate.json"), "utf8"));
    if (j.package && j.gate) return j;
  } catch {}
  return null;
}

/** facepay が注入する attestation。形が揃っていなければ null。 */
function attestation() {
  const raw = process.env.SUICASH_ATTESTATION;
  if (!raw) return null;
  try {
    const a = JSON.parse(raw);
    if (a.idi && a.r1 && a.proof && a.public_inputs && Number.isFinite(a.attested_at)) return a;
  } catch {}
  return null;
}

/** verify の moveCall を tx に積む。決済 PTB の先頭に置くこと。 */
function addVerifyCall(tx, cfg, att) {
  tx.moveCall({
    target: `${cfg.package}::suicash_gate::verify`,
    arguments: [
      tx.object(cfg.gate),
      tx.pure.vector("u8", Array.from(hexToBytes(att.idi))),
      tx.pure.u64(BigInt(att.attested_at)),
      tx.pure.vector("u8", Array.from(hexToBytes(att.proof))),
      tx.pure.vector("u8", Array.from(hexToBytes(att.public_inputs))),
      tx.pure.vector("u8", Array.from(hexToBytes(att.r1))),
      tx.object.clock(),
    ],
  });
}

/** Move アボートを利用者向けメッセージへ(ベストエフォート)。 */
function abortMessage(raw) {
  const s = String(raw || "");
  if (!/MoveAbort/.test(s)) return null;
  const mod = (s.match(/Identifier\("(\w+)"\)/) || [])[1] || "";
  const code = Number((s.match(/,\s*(\d+)\)/) || [])[1] ?? NaN);
  if (mod === "gate" && code === 1) {
    // gate::EReplay。同じ r1 は一度しか使えない
    return "この認証は使用済みです。カードを再タッチしてください";
  }
  if (mod === "gate" && code === 0) return "このカードは許可されていません";
  if (mod === "felica_auth" && code === 4) return "認証の有効期限が切れました。カードを再タッチしてください";
  if (mod === "felica_auth") return "認証データが不正です";
  if (mod === "zk_verifier") return "ZK証明の検証に失敗しました";
  return "ZK検証で拒否されました";
}

const client = new SuiClient({ url: RPC });

async function getBalanceMist(address) {
  const b = await client.getBalance({ owner: address });
  return BigInt(b.totalBalance);
}

function out(obj) {
  process.stdout.write(JSON.stringify(obj) + "\n");
}

async function main() {
  const [cmd, idi, a3, a4] = process.argv.slice(2);
  if (!cmd || !idi) {
    out({ ok: false, error: "usage: <address|balance|verify|pay> <idiHex> [amountMist] [merchant]" });
    process.exit(2);
  }
  const kp = await deriveKeypair(idi);
  const address = kp.getPublicKey().toSuiAddress();

  if (cmd === "address") {
    out({ ok: true, address });
    return;
  }
  if (cmd === "balance") {
    out({ ok: true, address, balance: (await getBalanceMist(address)).toString() });
    return;
  }

  if (cmd === "verify" || cmd === "pay") {
    // ZK 検証は必須。ゲート未設定・attestation 欠落は送金前に拒否する。
    const cfg = gateConfig();
    const att = attestation();
    if (!cfg) {
      out({ ok: false, error: "ZKゲート未設定(SUICASH_GATE_PKG/SUICASH_GATE_OBJ か gate.json)", address });
      process.exit(2);
    }
    if (!att) {
      out({ ok: false, error: "attestation がありません(SUICASH_ATTESTATION)。証明なしの決済は無効です", address });
      process.exit(2);
    }

    const tx = new Transaction();
    addVerifyCall(tx, cfg, att);

    let amount = 0n;
    let before = 0n;
    if (cmd === "pay") {
      amount = BigInt(a3);
      const merchant = a4;
      if (!merchant) {
        out({ ok: false, error: "merchant address required" });
        process.exit(2);
      }
      before = await getBalanceMist(address);
      const [coin] = tx.splitCoins(tx.gas, [amount]);
      tx.transferObjects([coin], merchant);
    }

    let res;
    try {
      res = await client.signAndExecuteTransaction({
        signer: kp,
        transaction: tx,
        options: { showEffects: true, showEvents: true },
      });
    } catch (e) {
      const msg = abortMessage(e?.message) || e?.message || String(e);
      out({ ok: false, error: msg, address });
      process.exit(1);
    }
    const status = res.effects?.status?.status;
    if (status !== "success") {
      const raw = res.effects?.status?.error || status || "tx failed";
      out({ ok: false, error: abortMessage(raw) || raw, address });
      process.exit(1);
    }
    const verifiedEvent = (res.events || []).find((ev) =>
      ev.type.endsWith("::suicash_gate::AttestationVerified")
    );

    if (cmd === "verify") {
      out({ ok: true, address, digest: res.digest, verified: Boolean(verifiedEvent) });
      return;
    }

    await new Promise((r) => setTimeout(r, 1500));
    let after;
    try {
      after = (await getBalanceMist(address)).toString();
    } catch {
      after = (before - amount).toString();
    }
    out({
      ok: true,
      address,
      amount: amount.toString(),
      balanceAfter: after,
      digest: res.digest,
      zkVerified: Boolean(verifiedEvent),
    });
    return;
  }
  out({ ok: false, error: `unknown command: ${cmd}` });
  process.exit(2);
}

main().catch((e) => {
  out({ ok: false, error: e?.message || String(e) });
  process.exit(1);
});
