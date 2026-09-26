/**
 * 交通系 IC カードの券面番号 ⇔ 8 バイト IDi(発行 ID)の相互変換。
 *
 * 変換規則は felica-rs(MIT, soltia48/felica-rs)の `examples/dump_suica.rs`
 * にある `idi_bytes_to_str` / `issuer_id_info` を移植したもの。
 * nimoca の実サンプル `NR807 E200 1060 0517` = IDi `05D5 807E 2826 0205`
 * で一致を確認済み。
 *
 * IDi 8 バイトの構造:
 *   b0-b1 : 発行体コード(issuer_id, BE16)→ 英字2文字(表引き。無ければ 16進4桁)
 *   b2-b3 : 追加識別子 → 16進大文字そのまま(4 文字)
 *   b4-b5 : 発行日 v(BE16)→ 年 (v>>9)&0x3F / 月 (v>>5)&0x0F / 日 v&0x1F → YYMMDD
 *   b6-b7 : 連番(BE16)→ 10進 5 桁ゼロ詰め
 *
 * 券面番号は「英字2 or 16進4」+「16進4」+「YYMMDD(6)」+「連番(5)」。
 * 発行体が表にある場合 2+4+6+5 = 17 文字、無い場合 4+4+6+5 = 19 文字。
 */

/** 発行体コード(issuer_id)→ 券面プレフィックス英字 */
const ISSUER_TO_PREFIX: Record<number, string> = {
  0x0102: "JH", // 北海道旅客鉄道
  0x0103: "JE", // 東日本旅客鉄道
  0x0104: "JC", // 東海旅客鉄道
  0x0105: "JW", // 西日本旅客鉄道
  0x0107: "JK", // 九州旅客鉄道
  0x0252: "PB", // パスモ
  0x0387: "TP", // 名古屋交通開発機構・エムアイシー
  0x04ad: "SU", // スルッとKANSAI
  0x05d5: "NR", // ニモカ
  0x05d7: "FC", // 福岡市交通局
};

/** 逆引き(プレフィックス英字 → issuer_id) */
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
 * 8 バイト IDi(16 hex 文字)→ 券面番号(空白なしの連結文字列)。
 * 入力が 8 バイトでなければ null。
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
 * 券面番号(空白除去済み)→ 8 バイト IDi(16 hex 文字)。
 * 逆変換できない(発行体プレフィックス不明・桁数不一致・日付や連番が範囲外)
 * 場合は null を返す。
 */
export function cardNumberToIdi(card: string): string | null {
  const s = card.replace(/[\s-]/g, "").toUpperCase();

  // head の切り出し: 先頭が既知プレフィックス(英字2)ならそれ、
  // でなければ先頭 4 文字を 16進 issuer_id として扱う。
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

  // rest = remainder(hex4) + date(6) + tail(5) = 15 文字
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
  if (month > 0x0f || day > 0x1f) return null; // ビット幅に収まらない
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

/** 券面番号が IDi へ一意に逆変換できるか(往復一致で確認) */
export function isConvertibleCardNumber(card: string): boolean {
  const idi = cardNumberToIdi(card);
  if (!idi) return false;
  const back = idiToCardNumber(idi);
  return back === card.replace(/[\s-]/g, "").toUpperCase();
}

/** 券面番号を表示用に整形(先頭 head + 4桁区切り、nimoca 券面の見た目に寄せる) */
export function formatCardNumber(card: string): string {
  const s = card.replace(/[\s-]/g, "").toUpperCase();
  // 交通系の券面表記は「NR807 E200 1060 0517」= 5-4-4-4。
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
