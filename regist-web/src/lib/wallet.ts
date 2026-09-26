import { Ed25519Keypair } from "@mysten/sui/keypairs/ed25519";
import { SuiClient, getFullnodeUrl } from "@mysten/sui/client";
import { getFaucetHost, requestSuiFromFaucetV2 } from "@mysten/sui/faucet";
import { Transaction } from "@mysten/sui/transactions";

/**
 * Sui wallet (Account Abstraction wallet seeded by the IDi).
 *
 * The wallet is derived deterministically from the IDi. The same IDi always maps to
 * the same Sui address, so the top-up router can resolve "IDi -> wallet address".
 * The IDi is treated not as a secret but as a public account identifier (like an account number).
 *
 * ⚠ Because derivation is deterministic, anyone who reads the card can reproduce the key.
 * Funds are therefore protected not by keeping this key secret but by the "IDi + face ZK proof"
 * at unlock time (face ZKP is local to Hi-CARA only) = the on-chain ZKP gate.
 * What we create here is a deterministic wallet that fixes that account address.
 *
 * Network is Sui testnet. Top-ups use the testnet faucet.
 *
 * Endpoints are overridable (public endpoints often hit 429 rate limits):
 *   - fullnode RPC : ?rpc=<url> / localStorage suicash.rpc / VITE_SUI_RPC
 *   - faucet       : ?faucet=<url> / localStorage suicash.faucet / VITE_SUI_FAUCET
 */

const NETWORK = "testnet" as const;

/** Domain separation tag (so other uses of the same IDi yield different keys) */
const SEED_DOMAIN = "suicash-aa-wallet:v1:";

/** Resolve a setting from URL query -> localStorage -> build-time env -> default */
function setting(key: string, envKey: string, fallback: string): string {
  try {
    const q = new URLSearchParams(window.location.search).get(key);
    if (q) {
      localStorage.setItem(`suicash.${key}`, q);
      return q;
    }
    const stored = localStorage.getItem(`suicash.${key}`);
    if (stored) return stored;
  } catch {
    // Continue even if localStorage is unavailable
  }
  const env = (import.meta.env as Record<string, string | undefined>)[envKey];
  return env || fallback;
}

const RPC_URL = setting("rpc", "VITE_SUI_RPC", getFullnodeUrl(NETWORK));
const FAUCET_URL = setting("faucet", "VITE_SUI_FAUCET", getFaucetHost(NETWORK));

/**
 * Treasury secret key (optional). If set, transfers come from this pre-funded
 * account instead of the faucet (fully avoiding the public faucet's 429s).
 * Format is `suiprivkey1...` (sui keytool bech32). Testnet only, for demos.
 * Never commit the value; inject it at runtime via URL/localStorage/env.
 */
const TREASURY_SECRET = setting("treasury", "VITE_TREASURY_SECRET", "");

/** Amount per top-up (MIST). Default 0.2 SUI */
const CHARGE_MIST = 200_000_000n;

export const client = new SuiClient({ url: RPC_URL });

export function hasTreasury(): boolean {
  return TREASURY_SECRET.trim().length > 0;
}

/** IDi (16 hex chars) -> 32-byte seed (SHA-256(domain || idiBytes)) */
async function seedFromIdi(idiHex: string): Promise<Uint8Array> {
  const idiBytes = hexToBytes(idiHex);
  const domain = new TextEncoder().encode(SEED_DOMAIN);
  const input = new Uint8Array(domain.length + idiBytes.length);
  input.set(domain, 0);
  input.set(idiBytes, domain.length);
  const digest = await crypto.subtle.digest("SHA-256", input);
  return new Uint8Array(digest); // 32 bytes
}

/** Deterministically derive the AA wallet key from the IDi */
export async function deriveWallet(idiHex: string): Promise<Ed25519Keypair> {
  const seed = await seedFromIdi(idiHex);
  return Ed25519Keypair.fromSecretKey(seed);
}

export function walletAddress(kp: Ed25519Keypair): string {
  return kp.getPublicKey().toSuiAddress();
}

/** Deterministically resolve the wallet address from the IDi (router equivalent) */
export async function resolveAddress(idiHex: string): Promise<string> {
  return walletAddress(await deriveWallet(idiHex));
}

function hexToBytes(h: string): Uint8Array {
  const s = h.trim().toLowerCase().replace(/[^0-9a-f]/g, "");
  const out = new Uint8Array(s.length / 2);
  for (let i = 0; i < out.length; i++) {
    out[i] = parseInt(s.slice(i * 2, i * 2 + 2), 16);
  }
  return out;
}

/** Balance (string in SUI). null on failure */
export async function fetchBalance(address: string): Promise<string | null> {
  try {
    const b = await client.getBalance({ owner: address });
    const mist = BigInt(b.totalBalance);
    const whole = mist / 1_000_000_000n;
    const frac = (mist % 1_000_000_000n).toString().padStart(9, "0").slice(0, 2);
    return `${whole}.${frac}`;
  } catch {
    return null;
  }
}

export interface ChargeResult {
  ok: boolean;
  /** User-facing message */
  message: string;
  /** Cooldown seconds after a 429 etc. (if known) */
  retryAfterSec?: number;
}

/**
 * Top up.
 * If a treasury key is set, transfer from it (skipping the faucet to avoid 429s).
 * Otherwise fall back to the public testnet faucet (prone to 429s).
 */
export async function requestCharge(address: string): Promise<ChargeResult> {
  if (hasTreasury()) {
    return chargeFromTreasury(address);
  }
  try {
    await requestSuiFromFaucetV2({ host: FAUCET_URL, recipient: address });
    return { ok: true, message: "Topped up (testnet faucet)" };
  } catch (e) {
    const status = extractStatus(e);
    if (status === 429) {
      return {
        ok: false,
        message:
          "The faucet is busy (rate limited). Set a treasury key to avoid 429s (see README).",
        retryAfterSec: 60,
      };
    }
    const detail = e instanceof Error ? e.message : String(e);
    return { ok: false, message: `Top-up failed: ${detail}` };
  }
}

/** Transfer CHARGE_MIST from the treasury (pre-funded account) */
async function chargeFromTreasury(address: string): Promise<ChargeResult> {
  try {
    const treasury = Ed25519Keypair.fromSecretKey(TREASURY_SECRET.trim());
    const tx = new Transaction();
    const [coin] = tx.splitCoins(tx.gas, [CHARGE_MIST]);
    tx.transferObjects([coin], address);
    const res = await client.signAndExecuteTransaction({
      signer: treasury,
      transaction: tx,
      options: { showEffects: true },
    });
    const status = res.effects?.status?.status;
    if (status !== "success") {
      return { ok: false, message: `Transfer failed: ${res.effects?.status?.error ?? status}` };
    }
    return { ok: true, message: `Topped up (${Number(CHARGE_MIST) / 1e9} SUI)` };
  } catch (e) {
    const detail = e instanceof Error ? e.message : String(e);
    if (/insufficient|gas|balance/i.test(detail)) {
      return { ok: false, message: "Treasury balance is insufficient. Please refill it with testnet SUI." };
    }
    return { ok: false, message: `Top-up failed: ${detail}` };
  }
}

/** Treasury address (shown for refilling). null if not set */
export function treasuryAddress(): string | null {
  if (!hasTreasury()) return null;
  try {
    return Ed25519Keypair.fromSecretKey(TREASURY_SECRET.trim()).getPublicKey().toSuiAddress();
  } catch {
    return null;
  }
}

/** Pick out something that looks like an HTTP status from an exception */
function extractStatus(e: unknown): number | undefined {
  if (typeof e === "object" && e !== null) {
    const anyE = e as Record<string, unknown>;
    if (typeof anyE.status === "number") return anyE.status;
    const msg = e instanceof Error ? e.message : "";
    const m = msg.match(/\b(429|4\d\d|5\d\d)\b/);
    if (m) return Number(m[1]);
  }
  return undefined;
}
