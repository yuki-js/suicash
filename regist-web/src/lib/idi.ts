/**
 * IDi(FeliCa 発行 ID、8 バイト)の取り扱い。
 *
 * カード裏面の券面番号(例 "NR807 E200 1060 0517")は、この 8 バイト IDi を
 * felica-rs 由来の規則で文字列化したもの。券面 ⇔ IDi の変換は cardnumber.ts。
 *
 * オンチェーンでの扱い: IDi 検証オラクル(別担当・oracle ブランチ)の Groth16
 * 回路は IDi を公開入力 pi0 として扱い、8 バイトを little-endian の
 * BN254 スカラーにエンコードする。トランスクリプトと鍵は private witness で、
 * IDi 自体は「公開の同一性主張」。したがってウォレットの対応付けキーは
 * この 8 バイト IDi(の正規表現 = 16 hex 文字)である。
 *
 * 初期登録では券面番号の手入力/OCR から IDi を復元してこの端末に保持し、
 * 実際の FeliCa 相互認証による検証は、リーダーのある場所(決済端末・母艦)で
 * オラクル経由で行う。
 */

import { cardNumberToIdi, idiToCardNumber, isConvertibleCardNumber } from "./cardnumber";

export { formatCardNumber } from "./cardnumber";

/** 空白・ハイフンを除去して大文字化(券面番号の正規化) */
export function normalizeCardNumber(raw: string): string {
  return raw.replace(/[\s-]/g, "").toUpperCase();
}

/**
 * 入力された券面番号が有効か(IDi へ一意に逆変換でき、往復一致する)。
 * 発行体プレフィックス(JE/PB/NR…)が既知、または先頭 4 文字が 16 進の
 * issuer_id として解釈でき、日付・連番がビット幅に収まる場合のみ真。
 */
export function isValidCardNumber(card: string): boolean {
  return isConvertibleCardNumber(card);
}

/** 券面番号 → IDi(16 hex 文字)。失敗時 null */
export function toIdi(card: string): string | null {
  return cardNumberToIdi(card);
}

/** IDi(16 hex 文字)→ 券面番号(空白なし)。失敗時 null */
export function fromIdi(idiHex: string): string | null {
  return idiToCardNumber(idiHex);
}

/** IDi(16 hex)の表示用マスク(先頭 4・末尾 4 以外を伏せる) */
export function maskIdi(idiHex: string): string {
  if (idiHex.length < 8) return idiHex;
  return `${idiHex.slice(0, 4)}????????${idiHex.slice(-4)}`;
}
