import { createWorker, PSM, type Worker } from "tesseract.js";

/**
 * 券面 OCR(Web 版)。
 *
 * 認証端末(Hi-CARA)で実績のある読み方を Web に移植したもの:
 *   ID 行の固定 ROI → 反転/適応二値化で「白地×黒文字」に正規化
 *   → 英数字ホワイトリストで 1 行 OCR → 英字→数字の整形。
 * 画像はすべてブラウザ内で処理し、外部へ送信しない。
 */

/** 交通系 IC カードの縦横比(85.6mm × 54.0mm) */
export const CARD_ASPECT = 85.6 / 54;

/**
 * ID 行の位置(カードガイド枠に対する割合)。
 * 認証端末版のキャリブレーション値を初期値として流用し、少し余白を持たせている。
 */
export const ID_ROI = { x0: 0.42, x1: 0.97, y0: 0.69, y1: 0.84 };

let workerPromise: Promise<Worker> | null = null;

function getWorker(): Promise<Worker> {
  if (!workerPromise) {
    workerPromise = (async () => {
      const w = await createWorker("eng");
      await w.setParameters({
        tessedit_char_whitelist: "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 ",
        tessedit_pageseg_mode: PSM.SINGLE_LINE,
      });
      return w;
    })();
  }
  return workerPromise;
}

/** ID 行を切り出した canvas を OCR して整形済み文字列を返す */
export async function ocrIdCanvas(canvas: HTMLCanvasElement): Promise<string> {
  const prepared = preprocess(canvas);
  const worker = await getWorker();
  const { data } = await worker.recognize(prepared);
  return cleanIdi(data.text);
}

/**
 * 前処理: グレースケール → 適応二値化(積分画像による局所しきい値)
 * → 黒が過半なら反転(白抜き印字を黒文字に正規化) → 白余白を付与。
 */
function preprocess(src: HTMLCanvasElement): HTMLCanvasElement {
  const w = src.width;
  const h = src.height;
  const ctx = src.getContext("2d")!;
  const img = ctx.getImageData(0, 0, w, h);
  const d = img.data;

  // 輝度(端末版と同じ重み)
  const lum = new Uint8Array(w * h);
  for (let i = 0, p = 0; i < d.length; i += 4, p++) {
    lum[p] = (d[i] + d[i + 1] * 6 + d[i + 2] * 3) / 10;
  }

  // 積分画像
  const integ = new Float64Array((w + 1) * (h + 1));
  for (let y = 0; y < h; y++) {
    let row = 0;
    for (let x = 0; x < w; x++) {
      row += lum[y * w + x];
      integ[(y + 1) * (w + 1) + (x + 1)] = integ[y * (w + 1) + (x + 1)] + row;
    }
  }

  // 適応二値化(窓半径 max(10, w/32)、C=8)
  const r = Math.max(10, Math.floor(w / 32));
  const C = 8;
  const bin = new Uint8Array(w * h);
  let blacks = 0;
  for (let y = 0; y < h; y++) {
    const y0 = Math.max(0, y - r);
    const y1 = Math.min(h - 1, y + r);
    for (let x = 0; x < w; x++) {
      const x0 = Math.max(0, x - r);
      const x1 = Math.min(w - 1, x + r);
      const area = (x1 - x0 + 1) * (y1 - y0 + 1);
      const sum =
        integ[(y1 + 1) * (w + 1) + (x1 + 1)] -
        integ[y0 * (w + 1) + (x1 + 1)] -
        integ[(y1 + 1) * (w + 1) + x0] +
        integ[y0 * (w + 1) + x0];
      const white = lum[y * w + x] >= sum / area - C;
      bin[y * w + x] = white ? 255 : 0;
      if (!white) blacks++;
    }
  }

  // 黒が過半 = 白抜き印字なので反転して「白地×黒文字」へ
  if (blacks > (w * h) / 2) {
    for (let p = 0; p < bin.length; p++) {
      bin[p] = 255 - bin[p];
    }
  }

  // 白余白を付けて出力(余白が無いと検出に失敗しやすい)
  const pad = 24;
  const out = document.createElement("canvas");
  out.width = w + pad * 2;
  out.height = h + pad * 2;
  const octx = out.getContext("2d")!;
  octx.fillStyle = "#fff";
  octx.fillRect(0, 0, out.width, out.height);
  const oimg = octx.createImageData(w, h);
  for (let p = 0; p < bin.length; p++) {
    const v = bin[p];
    oimg.data[p * 4] = v;
    oimg.data[p * 4 + 1] = v;
    oimg.data[p * 4 + 2] = v;
    oimg.data[p * 4 + 3] = 255;
  }
  octx.putImageData(oimg, pad, pad);
  return out;
}

/** OCR が数字を英字に取り違えたときの補正表 */
const LETTER_TO_DIGIT: Record<string, string> = {
  O: "0", Q: "0", D: "0", C: "0", U: "0",
  I: "1", L: "1",
  Z: "2", A: "4", S: "5", G: "6", T: "7", B: "8", P: "9",
};

/**
 * OCR 結果を IDi 形式へ整形する。
 * 先頭 2 文字は英字プレフィックス(例 KA)として保持し、
 * 3 文字目以降の英字は数字の見間違いとして数字化。17 文字で切る。
 */
export function cleanIdi(rawText: string): string {
  const s = rawText.toUpperCase().replace(/[^A-Z0-9]/g, "");
  let out = s.slice(0, 2);
  for (const ch of s.slice(2)) {
    out += /[0-9]/.test(ch) ? ch : (LETTER_TO_DIGIT[ch] ?? ch);
  }
  return out.slice(0, 17);
}
