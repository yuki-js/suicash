/**
 * Handling of the IDi (FeliCa issuance ID, 8 bytes).
 *
 * The card number printed on the back (e.g. "NR807 E200 1060 0517") is this 8-byte
 * IDi stringified by rules derived from felica-rs. Card number <-> IDi conversion lives in cardnumber.ts.
 *
 * On-chain: the Groth16 circuit of the IDi verification oracle (separate owner,
 * oracle branch) treats the IDi as public input pi0, encoding the 8 bytes as a
 * little-endian BN254 scalar. The transcript and keys are private witnesses, and
 * the IDi itself is a "public identity claim". So the wallet mapping key is this
 * 8-byte IDi (canonical form = 16 hex chars).
 *
 * At initial registration, the IDi is recovered from manual entry / OCR of the card
 * number and kept on this device; actual verification via FeliCa mutual authentication
 * happens through the oracle wherever a reader exists (payment terminal / host).
 */

import { cardNumberToIdi, idiToCardNumber, isConvertibleCardNumber } from "./cardnumber";

export { formatCardNumber } from "./cardnumber";

/** Strip spaces/hyphens and uppercase (card number normalization) */
export function normalizeCardNumber(raw: string): string {
  return raw.replace(/[\s-]/g, "").toUpperCase();
}

/**
 * Whether the entered card number is valid (uniquely invertible to an IDi and round-trips).
 * True only if the issuer prefix (JE/PB/NR…) is known or the first 4 chars parse as a
 * hex issuer_id, and the date and serial fit their bit widths.
 */
export function isValidCardNumber(card: string): boolean {
  return isConvertibleCardNumber(card);
}

/** Card number -> IDi (16 hex chars). null on failure */
export function toIdi(card: string): string | null {
  return cardNumberToIdi(card);
}

/** IDi (16 hex chars) -> card number (no spaces). null on failure */
export function fromIdi(idiHex: string): string | null {
  return idiToCardNumber(idiHex);
}

/** Display mask for an IDi (16 hex): hide all but the first 4 and last 4 */
export function maskIdi(idiHex: string): string {
  if (idiHex.length < 8) return idiHex;
  return `${idiHex.slice(0, 4)}????????${idiHex.slice(-4)}`;
}
