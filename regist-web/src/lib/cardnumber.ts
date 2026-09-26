/**
 * Conversion between a transit IC card's printed number and its 8-byte IDi (issuance ID).
 *
 * The rules are ported from `idi_bytes_to_str` / `issuer_id_info` in
 * `examples/dump_suica.rs` of felica-rs (MIT, soltia48/felica-rs).
 * Verified against a real nimoca sample: `NR807 E200 1060 0517` = IDi `05D5 807E 2826 0205`.
 *
 * IDi 8-byte layout:
 *   b0-b1 : issuer code (issuer_id, BE16) -> 2 letters (table lookup; else 4 hex digits)
 *   b2-b3 : extra ID -> uppercase hex as-is (4 chars)
 *   b4-b5 : issue date v (BE16) -> year (v>>9)&0x3F / month (v>>5)&0x0F / day v&0x1F -> YYMMDD
 *   b6-b7 : serial (BE16) -> 5 decimal digits, zero-padded
 *
 * Card number = "2 letters or 4 hex" + "4 hex" + "YYMMDD (6)" + "serial (5)".
 * 17 chars if the issuer is in the table (2+4+6+5), otherwise 19 chars (4+4+6+5).
 */

/** Issuer code (issuer_id) -> card number letter prefix */
const ISSUER_TO_PREFIX: Record<number, string> = {
  0x0102: "JH", // JR Hokkaido
  0x0103: "JE", // JR East
  0x0104: "JC", // JR Central
  0x0105: "JW", // JR West
  0x0107: "JK", // JR Kyushu
  0x0252: "PB", // PASMO
  0x0387: "TP", // Nagoya Transportation Development Organization / MIC
  0x04ad: "SU", // Surutto KANSAI
  0x05d5: "NR", // nimoca
  0x05d7: "FC", // Fukuoka City Transportation Bureau
};

/** Reverse lookup (letter prefix -> issuer_id) */
const PREFIX_TO_ISSUER: Record<string, number> = Object.fromEntries(
  Object.entries(ISSUER_TO_PREFIX).map(([k, v]) => [v, Number(k)]),
);

function hex2(n: number): string {
  return n.toString(16).toUpperCase().padStart(2, "0");
}

function hex4(n: number): string {
  return n.toString(16).toUpperCase().padStart(4, "0");
}

/**
 * 8-byte IDi (16 hex chars) -> card number (concatenated, no spaces).
 * null if the input is not 8 bytes.
 */
export function idiToCardNumber(idiHex: string): string | null {
  const b = hexToBytes(idiHex);
  if (!b || b.length !== 8) return null;

  const issuer = (b[0] << 8) | b[1];
  const remainder = hex2(b[2]) + hex2(b[3]);
  const prefix = ISSUER_TO_PREFIX[issuer];
  const head = prefix ? prefix + remainder : hex4(issuer) + remainder;

  const v = (b[4] << 8) | b[5];
  const year = (v >> 9) & 0x3f;
  const month = (v >> 5) & 0x0f;
  const day = v & 0x1f;
  const yy = year % 100;
  const date =
    String(yy).padStart(2, "0") +
    String(month).padStart(2, "0") +
    String(day).padStart(2, "0");

  const tail = String((b[6] << 8) | b[7]).padStart(5, "0");

  return head + date + tail;
}

/**
 * Card number (spaces removed) -> 8-byte IDi (16 hex chars).
 * Returns null if it can't be inverted (unknown issuer prefix, wrong length,
 * date or serial out of range).
 */
export function cardNumberToIdi(card: string): string | null {
  const s = card.replace(/[\s-]/g, "").toUpperCase();

  // Extract the head: a known 2-letter prefix if present,
  // otherwise treat the first 4 chars as a hex issuer_id.
  let issuer: number;
  let rest: string;
  const prefix2 = s.slice(0, 2);
  if (PREFIX_TO_ISSUER[prefix2] !== undefined) {
    issuer = PREFIX_TO_ISSUER[prefix2];
    rest = s.slice(2);
  } else if (/^[0-9A-F]{4}/.test(s)) {
    issuer = parseInt(s.slice(0, 4), 16);
    rest = s.slice(4);
  } else {
    return null;
  }

  // rest = remainder(hex4) + date(6) + tail(5) = 15 chars
  if (rest.length !== 15) return null;
  const remainderHex = rest.slice(0, 4);
  const dateStr = rest.slice(4, 10);
  const tailStr = rest.slice(10, 15);

  if (!/^[0-9A-F]{4}$/.test(remainderHex)) return null;
  if (!/^[0-9]{6}$/.test(dateStr)) return null;
  if (!/^[0-9]{5}$/.test(tailStr)) return null;

  const remainder = parseInt(remainderHex, 16);

  const yy = parseInt(dateStr.slice(0, 2), 10);
  const month = parseInt(dateStr.slice(2, 4), 10);
  const day = parseInt(dateStr.slice(4, 6), 10);
  if (month > 0x0f || day > 0x1f) return null; // doesn't fit the bit width
  const v = (yy << 9) | (month << 5) | day;

  const tail = parseInt(tailStr, 10);
  if (v > 0xffff || tail > 0xffff) return null;

  const bytes = [
    (issuer >> 8) & 0xff,
    issuer & 0xff,
    (remainder >> 8) & 0xff,
    remainder & 0xff,
    (v >> 8) & 0xff,
    v & 0xff,
    (tail >> 8) & 0xff,
    tail & 0xff,
  ];
  return bytesToHex(bytes);
}

/** Whether the card number uniquely inverts to an IDi (checked by round-trip) */
export function isConvertibleCardNumber(card: string): boolean {
  const idi = cardNumberToIdi(card);
  if (!idi) return false;
  const back = idiToCardNumber(idi);
  return back === card.replace(/[\s-]/g, "").toUpperCase();
}

/** Format a card number for display (head + groups of 4, matching the nimoca card look) */
export function formatCardNumber(card: string): string {
  const s = card.replace(/[\s-]/g, "").toUpperCase();
  // Transit cards print it as "NR807 E200 1060 0517" = 5-4-4-4.
  const groups = [s.slice(0, 5), s.slice(5, 9), s.slice(9, 13), s.slice(13, 17)];
  return groups.filter(Boolean).join(" ");
}

function hexToBytes(h: string): number[] | null {
  const s = h.trim().toLowerCase();
  if (!/^[0-9a-f]*$/.test(s) || s.length % 2 !== 0) return null;
  const out: number[] = [];
  for (let i = 0; i < s.length; i += 2) {
    out.push(parseInt(s.slice(i, i + 2), 16));
  }
  return out;
}

function bytesToHex(bytes: number[]): string {
  return bytes.map((b) => b.toString(16).padStart(2, "0")).join("");
}
