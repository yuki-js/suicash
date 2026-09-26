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
 */

const NETWORK = "testnet" as const;

/** ドメイン分離タグ(用途違いで同じ IDi から別鍵が出ないように) */
const SEED_DOMAIN = "suicash-aa-wallet:v1:";

export const client = new SuiClient({ url: getFullnodeUrl(NETWORK) });

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

/** testnet faucet からチャージ。成功で true */
export async function requestCharge(address: string): Promise<boolean> {
  try {
    await requestSuiFromFaucetV2({
      host: getFaucetHost(NETWORK),
      recipient: address,
    });
    return true;
  } catch {
    return false;
  }
}
