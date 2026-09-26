import { Ed25519Keypair } from "@mysten/sui/keypairs/ed25519";
import { SuiClient, getFullnodeUrl } from "@mysten/sui/client";
import { getFaucetHost, requestSuiFromFaucetV2 } from "@mysten/sui/faucet";

/**
 * Sui ウォレット(デモ用)。
 *
 * 鍵はブラウザ内でランダム生成し localStorage に保持する。
 * IDi は鍵導出に使わない(IDi はリーダーで誰でも読める値であり秘密ではない)。
 * IDi(のコミットメント)→ ウォレットの対応付けは、後段のルータ/検証サーバー
 * (別担当)がチェーン上で解決する設計。
 *
 * ネットワークは Sui testnet。チャージは testnet faucet を使う。
 */

const NETWORK = "testnet" as const;
const KEY_STORAGE = "suicash.wallet.secret";

export const client = new SuiClient({ url: getFullnodeUrl(NETWORK) });

/** 保存済みの鍵を読む。無ければ null */
export function loadWallet(): Ed25519Keypair | null {
  try {
    const secret = localStorage.getItem(KEY_STORAGE);
    if (!secret) return null;
    return Ed25519Keypair.fromSecretKey(secret);
  } catch {
    return null;
  }
}

/** 新しい鍵を生成して保存する */
export function createWallet(): Ed25519Keypair {
  const kp = new Ed25519Keypair();
  try {
    localStorage.setItem(KEY_STORAGE, kp.getSecretKey());
  } catch {
    // 保存できない環境(プライベートブラウズ等)でもセッション内では使える
  }
  return kp;
}

/** 保存済みの鍵を消す(登録やり直し用) */
export function removeWallet(): void {
  try {
    localStorage.removeItem(KEY_STORAGE);
  } catch {
    // noop
  }
}

export function walletAddress(kp: Ed25519Keypair): string {
  return kp.getPublicKey().toSuiAddress();
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
