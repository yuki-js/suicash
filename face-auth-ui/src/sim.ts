/**
 * 顔認証エンジン(SAFR eSDK)のモック。
 * エンジンの呼び出し順(detectFaces → learn / recognize)と応答仕様
 * (confidence 1.0 超あり・品質しきい値・結果コード)だけを UI 検証用に再現する。
 * 実際の顔検出・特徴量抽出は行わない。
 */

export interface DetectedFace {
  confidence: number; // 照合スコア。0〜1 の確率ではなく 1.0 超あり
  centerPoseQuality: number; // cpq >= 0.59 が合格
  contrastQuality: number; // >= 0.45 が合格
  sharpnessQuality: number; // >= 0.45 が合格
  mask: number; // < 0.30 でマスクなし判定
  bounds: { x: number; y: number; w: number; h: number }; // 0..1 正規化
  extraRotation: 0 | 90 | 180 | 270; // 多方向リトライで採用された追加回転
}

export interface RecognizeResult {
  code: 0 | 65 | 66 | 160;
  face: DetectedFace | null;
}

export const QUALITY_GATE = {
  cpq: 0.59,
  contrast: 0.45,
  sharpness: 0.45,
  mask: 0.3,
} as const;

export const DEFAULT_THRESHOLD = 0.8;
export const MAX_THRESHOLD = 2.0;

/** 平滑ランダムウォークで「それらしい」品質値を揺らすジェネレータ */
class Walk {
  private v: number;
  constructor(
    initial: number,
    private min: number,
    private max: number,
    private step: number,
  ) {
    this.v = initial;
  }
  next(): number {
    this.v += (Math.random() - 0.5) * 2 * this.step;
    if (this.v < this.min) this.v = this.min + (this.min - this.v);
    if (this.v > this.max) this.v = this.max - (this.v - this.max);
    return this.v;
  }
}

export class MockSafrEngine {
  /** person store はメモリ内・揮発(実エンジンと同じ) */
  private registered = false;
  private cpq = new Walk(0.72, 0.35, 0.95, 0.045);
  private contrast = new Walk(0.62, 0.3, 0.9, 0.035);
  private sharpness = new Walk(0.66, 0.3, 0.9, 0.035);
  private mask = new Walk(0.08, 0.0, 0.28, 0.02);
  private cx = new Walk(0.5, 0.36, 0.64, 0.012);
  private cy = new Walk(0.44, 0.32, 0.56, 0.012);
  private size = new Walk(0.46, 0.34, 0.62, 0.01);

  get hasPerson(): boolean {
    return this.registered;
  }

  /** プレビュー 1 フレームぶんの検出結果。まれに未検出(null)を返す */
  detectFaces(): DetectedFace | null {
    if (Math.random() < 0.04) return null;
    const w = this.size.next();
    return {
      confidence: 0,
      centerPoseQuality: this.cpq.next(),
      contrastQuality: this.contrast.next(),
      sharpnessQuality: this.sharpness.next(),
      mask: this.mask.next(),
      bounds: {
        x: this.cx.next() - w / 2,
        y: this.cy.next() - (w * 1.25) / 2,
        w,
        h: w * 1.25,
      },
      extraRotation: Math.random() < 0.12 ? 90 : 0,
    };
  }

  static passesGate(f: DetectedFace): boolean {
    return (
      f.centerPoseQuality >= QUALITY_GATE.cpq &&
      f.contrastQuality >= QUALITY_GATE.contrast &&
      f.sharpnessQuality >= QUALITY_GATE.sharpness &&
      f.mask < QUALITY_GATE.mask
    );
  }

  /** 登録: clearPersonStore → learnPerson 相当 */
  learnPerson(face: DetectedFace): RecognizeResult {
    if (!MockSafrEngine.passesGate(face)) return { code: 65, face };
    this.registered = true;
    return { code: 0, face };
  }

  /** 照合: recognizePerson 相当 */
  recognizePerson(face: DetectedFace, threshold: number): RecognizeResult {
    if (!MockSafrEngine.passesGate(face)) return { code: 160, face: null };
    // 実測レンジ(自機での検証値): 同一人物 0.998〜1.30 / 条件が悪い・未登録 0.55〜0.70
    const confidence = this.registered
      ? clamp(gauss(1.08, 0.11), 0.92, 1.34)
      : clamp(gauss(0.62, 0.05), 0.5, 0.72);
    const scored = { ...face, confidence };
    return { code: confidence >= threshold ? 0 : 66, face: scored };
  }

  clearPersonStore(): void {
    this.registered = false;
  }
}

function clamp(v: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, v));
}

/** Box-Muller 正規乱数 */
function gauss(mean: number, sd: number): number {
  const u = 1 - Math.random();
  const v = Math.random();
  return mean + sd * Math.sqrt(-2 * Math.log(u)) * Math.cos(2 * Math.PI * v);
}
