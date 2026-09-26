import { createWorker, PSM, type Worker } from "tesseract.js";

/**
 * Card face OCR (web version).
 *
 * A web port of the approach proven on the auth terminal (Hi-CARA):
 *   fixed ROI for the ID line -> invert/adaptive binarization to normalize to black-on-white
 *   -> single-line OCR with an alphanumeric whitelist -> letter-to-digit cleanup.
 * All images are processed in the browser and never sent anywhere.
 */

/** Aspect ratio of transit IC cards (85.6mm x 54.0mm) */
export const CARD_ASPECT = 85.6 / 54;

/**
 * Position of the ID line (as fractions of the card guide frame).
 * Starts from the auth terminal's calibration values, with a bit of extra margin.
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

/** OCR a canvas cropped to the ID line and return the cleaned card number */
export async function ocrIdCanvas(canvas: HTMLCanvasElement): Promise<string> {
  const prepared = preprocess(canvas);
  const worker = await getWorker();
  const { data } = await worker.recognize(prepared);
  return cleanCardNumber(data.text);
}

/**
 * Preprocess: grayscale -> adaptive binarization (local threshold via integral image)
 * -> invert if mostly black (normalize white-on-dark print to black text) -> add white padding.
 */
function preprocess(src: HTMLCanvasElement): HTMLCanvasElement {
  const w = src.width;
  const h = src.height;
  const ctx = src.getContext("2d")!;
  const img = ctx.getImageData(0, 0, w, h);
  const d = img.data;

  // Luminance (same weights as the terminal version)
  const lum = new Uint8Array(w * h);
  for (let i = 0, p = 0; i < d.length; i += 4, p++) {
    lum[p] = (d[i] + d[i + 1] * 6 + d[i + 2] * 3) / 10;
  }

  // Integral image
  const integ = new Float64Array((w + 1) * (h + 1));
  for (let y = 0; y < h; y++) {
    let row = 0;
    for (let x = 0; x < w; x++) {
      row += lum[y * w + x];
      integ[(y + 1) * (w + 1) + (x + 1)] = integ[y * (w + 1) + (x + 1)] + row;
    }
  }

  // Adaptive binarization (window radius max(10, w/32), C=8)
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

  // Mostly black = white-on-dark print, so invert to black-on-white
  if (blacks > (w * h) / 2) {
    for (let p = 0; p < bin.length; p++) {
      bin[p] = 255 - bin[p];
    }
  }

  // Output with white padding (detection often fails without it)
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

/** Correction table for digits the OCR misreads as letters */
const LETTER_TO_DIGIT: Record<string, string> = {
  O: "0", Q: "0", D: "0", C: "0", U: "0",
  I: "1", L: "1",
  Z: "2", A: "4", S: "5", G: "6", T: "7", B: "8", P: "9",
};

/**
 * Shape the OCR result into a card number.
 * A card number is "2-letter issuer + 4-digit extra ID (hex) + 6-digit issue date + 5-digit serial"
 * = 17 chars. The first 2 chars are kept as the letter prefix (e.g. NR / KA); after that,
 * letters that can't appear in hex (misread digits) are converted to digits.
 * Note: chars 3-6 are hex, so A-F are kept as-is.
 */
export function cleanCardNumber(rawText: string): string {
  const s = rawText.toUpperCase().replace(/[^A-Z0-9]/g, "");
  let out = s.slice(0, 2); // Issuer prefix
  s.slice(2, 6).split("").forEach((ch) => {
    // Extra ID (4 hex digits): keep A-F, convert only other letters to digits
    out += /[0-9A-F]/.test(ch) ? ch : (LETTER_TO_DIGIT[ch] ?? ch);
  });
  for (const ch of s.slice(6)) {
    // 6-digit issue date + 5-digit serial: all decimal digits
    out += /[0-9]/.test(ch) ? ch : (LETTER_TO_DIGIT[ch] ?? ch);
  }
  return out.slice(0, 17);
}
