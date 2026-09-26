import { Ed25519Keypair } from "@mysten/sui/keypairs/ed25519";
import { SuiClient, getFullnodeUrl } from "@mysten/sui/client";
import { getFaucetHost, requestSuiFromFaucetV2 } from "@mysten/sui/faucet";

/**
 * Sui ウォレット(IDi シードの Account Abstraction ウォレット)。
 *
 * ウォレットは IDi から決定的に導出する。同じ IDi は必ず同じ Sui アドレスに
 * なるため、チャージ経路のルータが「IDi → ウォレットアドレス」を解決できる。
 * IDi は秘密ではなく公開のアカウント識別子(口座番号に相当)として扱う。
 *
 * ⚠ 決定的導出なので、カードを読めば誰でも同じ鍵を再現できる。したがって
 * 資金の保護はこの鍵の秘匿ではなく、アンロック時の「IDi + 顔の ZKP 証明」
 * (顔 ZKP は Hi-CARA ローカルのみ)= オンチェーンの ZKP ゲートが担う。
 * ここで作るのは、その口座アドレスを決めるための決定的ウォレット。
 *
 * ネットワークは Sui testnet。チャージは testnet faucet を使う。
 *
 * エンドポイントは差し替え可能(公開エンドポイントは 429 レート制限が出やすい):
 *   - fullnode RPC : ?rpc=<url> / localStorage suicash.rpc / VITE_SUI_RPC
 *   - faucet       : ?faucet=<url> / localStorage suicash.faucet / VITE_SUI_FAUCET
 */

const NETWORK = "testnet" as const;

/** ドメイン分離タグ(用途違いで同じ IDi から別鍵が出ないように) */
const SEED_DOMAIN = "suicash-aa-wallet:v1:";

/** 設定値を URL クエリ → localStorage → ビルド時 env → 既定 の順で解決 */
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
    // localStorage 不可でも続行
  }
  const env = (import.meta.env as Record<string, string | undefined>)[envKey];
  return env || fallback;
}

const RPC_URL = setting("rpc", "VITE_SUI_RPC", getFullnodeUrl(NETWORK));
const FAUCET_URL = setting("faucet", "VITE_SUI_FAUCET", getFaucetHost(NETWORK));

export const client = new SuiClient({ url: RPC_URL });

/** IDi(16 hex 文字)→ 32 バイトのシード(SHA-256(domain || idiBytes)) */
async function seedFromIdi(idiHex: string): Promise<Uint8Array> {
  const idiBytes = hexToBytes(idiHex);
  const domain = new TextEncoder().encode(SEED_DOMAIN);
  const input = new Uint8Array(domain.length + idiBytes.length);
  input.set(domain, 0);
  input.set(idiBytes, domain.length);
  const digest = await crypto.subtle.digest("SHA-256", input);
  return new Uint8Array(digest); // 32 bytes
}

/** IDi から決定的に AA ウォレット鍵を導出する */
export async function deriveWallet(idiHex: string): Promise<Ed25519Keypair> {
  const seed = await seedFromIdi(idiHex);
  return Ed25519Keypair.fromSecretKey(seed);
}

export function walletAddress(kp: Ed25519Keypair): string {
  return kp.getPublicKey().toSuiAddress();
}

/** IDi から決定的にウォレットアドレスを解決する(ルータ相当) */
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

/** 残高(SUI 単位の文字列)。取得失敗時は null */
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
  /** ユーザー向けメッセージ */
  message: string;
  /** 429 等でのクールダウン秒数(分かれば) */
  retryAfterSec?: number;
}

/**
 * testnet faucet からチャージ。
 * 公開 faucet はレート制限(429)が出やすいので、理由とクールダウンを返す。
 */
export async function requestCharge(address: string): Promise<ChargeResult> {
  try {
    await requestSuiFromFaucetV2({ host: FAUCET_URL, recipient: address });
    return { ok: true, message: "チャージしました(testnet faucet)" };
  } catch (e) {
    const status = extractStatus(e);
    if (status === 429) {
      return {
        ok: false,
        message:
          "faucet が混雑しています(レート制限)。時間をおくか、同じアドレスは一定時間あけて再度お試しください。",
        retryAfterSec: 60,
      };
    }
    const detail = e instanceof Error ? e.message : String(e);
    return { ok: false, message: `チャージに失敗しました: ${detail}` };
  }
}

/** 例外オブジェクトから HTTP ステータスらしき数値を拾う */
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
