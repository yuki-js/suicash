#!/usr/bin/env node
/**
 * 母艦の Sui 決済ヘルパー。
 * IDi から決定的にウォレットを導出(regist-web と同一方式)し、残高照会・送金を行う。
 * facepay デーモンからサブコマンドで呼ばれ、JSON を1行返す。
 *
 *   node sui-pay.mjs address <idiHex>
 *   node sui-pay.mjs balance <idiHex>
 *   node sui-pay.mjs pay     <idiHex> <amountMist> <merchantAddr>
 *
 * 環境変数 SUI_RPC: fullnode JSON-RPC(既定は publicnode の testnet)
 *
 * ⚠ 導出方式は regist-web/src/lib/wallet.ts と厳密に一致させること:
 *   seed = SHA-256("suicash-aa-wallet:v1:" || idiBytes(8)) → Ed25519 秘密鍵
 */
import { Ed25519Keypair } from "@mysten/sui/keypairs/ed25519";
import { SuiClient } from "@mysten/sui/client";
import { Transaction } from "@mysten/sui/transactions";
import { webcrypto } from "node:crypto";

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
    out({ ok: false, error: "usage: <address|balance|pay> <idiHex> [amountMist] [merchant]" });
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
  if (cmd === "pay") {
    const amount = BigInt(a3);
    const merchant = a4;
    if (!merchant) {
      out({ ok: false, error: "merchant address required" });
      process.exit(2);
    }
    const before = await getBalanceMist(address);
    const tx = new Transaction();
    const [coin] = tx.splitCoins(tx.gas, [amount]);
    tx.transferObjects([coin], merchant);
    const res = await client.signAndExecuteTransaction({
      signer: kp,
      transaction: tx,
      options: { showEffects: true },
    });
    const status = res.effects?.status?.status;
    if (status !== "success") {
      out({ ok: false, error: res.effects?.status?.error || status || "tx failed", address });
      process.exit(1);
    }
    await new Promise((r) => setTimeout(r, 1500));
    let after;
    try {
      after = (await getBalanceMist(address)).toString();
    } catch {
      after = (before - amount).toString();
    }
    out({ ok: true, address, amount: amount.toString(), balanceAfter: after, digest: res.digest });
    return;
  }
  out({ ok: false, error: `unknown command: ${cmd}` });
  process.exit(2);
}

main().catch((e) => {
  out({ ok: false, error: e?.message || String(e) });
  process.exit(1);
});
