/**
 * IDi(交通系 IC カード裏面の券面番号。"KA" で始まる 17 文字の英数字)の
 * 取り扱い。
 *
 * プライバシー方針: 生の IDi はカードの追跡子になり得るため、外部
 * (チェーン・検証サーバー)にはコミットメント hash(IDi, salt) だけを渡す。
 * 生の IDi と salt はこの端末(localStorage)にのみ保持する。
 *
 * コミットメントの方式は暫定で SHA-256。IDi 検証サーバー(別担当)の
 * ZKP 回路が決まったら Poseidon 等に合わせて差し替える。
 */

/** 空白・ハイフンを除去して大文字化 */
export function normalizeIdi(raw: string): string {
  return raw.replace(/[\s-]/g, "").toUpperCase();
}

/** "KA" + 英数字 15 文字(計 17 文字) */
export function isValidIdi(idi: string): boolean {
  return /^KA[0-9A-Z]{15}$/.test(idi);
}

/** 表示用に 4 文字区切りへ整形 */
export function formatIdi(idi: string): string {
  return idi.replace(/(.{4})/g, "$1 ").trim();
}

/** 表示用マスク(先頭 4・末尾 2 以外を伏せる) */
export function maskIdi(idi: string): string {
  if (idi.length < 8) return idi;
  return `${idi.slice(0, 4)}************${idi.slice(-2)}`;
}

/** ランダム salt(hex 32 文字) */
export function randomSalt(): string {
  const b = new Uint8Array(16);
  crypto.getRandomValues(b);
  return hex(b);
}

/** commitment = SHA-256(`${idi}:${salt}`) を hex で返す(暫定方式) */
export async function idiCommitment(idi: string, salt: string): Promise<string> {
  const data = new TextEncoder().encode(`${idi}:${salt}`);
  const digest = await crypto.subtle.digest("SHA-256", data);
  return hex(new Uint8Array(digest));
}

function hex(bytes: Uint8Array): string {
  return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}
