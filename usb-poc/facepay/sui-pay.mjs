#!/usr/bin/env node
/**
 * Sui payment helper for the host.
 * Deterministically derives a wallet from the IDi (same scheme as regist-web) and does balance inquiry / transfers.
 * Invoked by the facepay daemon via subcommands; prints one line of JSON.
 *
 *   node sui-pay.mjs address <idiHex>
 *   node sui-pay.mjs balance <idiHex>
 *   node sui-pay.mjs pay     <idiHex> <amountMist> <merchantAddr>
 *
 * Env var SUI_RPC: fullnode JSON-RPC (defaults to publicnode testnet)
 *
 * ⚠ The derivation must exactly match regist-web/src/lib/wallet.ts:
 *   seed = SHA-256("suicash-aa-wallet:v1:" || idiBytes(8)) → Ed25519 private key
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
    // Fix the gas budget so the tx is deterministic (avoids equivocation from
    // estimate jitter contending for the same coin). Identical txs are idempotent on Sui.
    const GAS_BUDGET = 20_000_000n; // 0.02 SUI
    const buildTx = () => {
      const tx = new Transaction();
      tx.setGasBudget(GAS_BUDGET);
      const [coin] = tx.splitCoins(tx.gas, [amount]);
      tx.transferObjects([coin], merchant);
      return tx;
    };

    // Retry transient object conflicts (lock/version mismatch) once
    const isTransient = (m) =>
      /lock|equivocat|version|not available|reserved|conflict|deadline|timeout|fetch failed|network|ECONN/i.test(
        m || ""
      );
    let res;
    let lastErr = "";
    for (let attempt = 0; attempt < 2; attempt++) {
      try {
        res = await client.signAndExecuteTransaction({
          signer: kp,
          transaction: buildTx(),
          options: { showEffects: true },
        });
        const status = res.effects?.status?.status;
        if (status === "success") {
          lastErr = "";
          break;
        }
        lastErr = res.effects?.status?.error || status || "tx failed";
        res = undefined;
      } catch (e) {
        lastErr = e?.message || String(e);
      }
      if (attempt === 0 && isTransient(lastErr)) {
        await new Promise((r) => setTimeout(r, 1200));
        continue;
      }
      break;
    }
    if (!res) {
      out({ ok: false, error: lastErr || "tx failed", address });
      process.exit(1);
    }
    // Wait for finality before reading coin state (so the next payment has no version conflict)
    try {
      await client.waitForTransaction({ digest: res.digest });
    } catch {}
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
