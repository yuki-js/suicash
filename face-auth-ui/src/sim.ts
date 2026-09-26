/**
 * Mock of the face recognition engine (SAFR eSDK).
 * Reproduces only the engine's call order (detectFaces → learn / recognize) and response
 * spec (confidence can exceed 1.0, quality thresholds, result codes) for UI testing.
 * No actual face detection or feature extraction is performed.
 */

export interface DetectedFace {
  confidence: number; // Match score. Not a 0-1 probability; can exceed 1.0
  centerPoseQuality: number; // passes at cpq >= 0.59
  contrastQuality: number; // passes at >= 0.45
  sharpnessQuality: number; // passes at >= 0.45
  mask: number; // < 0.30 means no mask
  bounds: { x: number; y: number; w: number; h: number }; // normalized 0..1
  extraRotation: 0 | 90 | 180 | 270; // extra rotation adopted by the multi-orientation retry
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

/** Generator that jitters plausible quality values with a smoothed random walk */
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
  /** The person store is in-memory and volatile (same as the real engine) */
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

  /** Detection result for one preview frame. Occasionally returns no face (null) */
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

  /** Enroll: equivalent to clearPersonStore → learnPerson */
  learnPerson(face: DetectedFace): RecognizeResult {
    if (!MockSafrEngine.passesGate(face)) return { code: 65, face };
    this.registered = true;
    return { code: 0, face };
  }

  /** Match: equivalent to recognizePerson */
  recognizePerson(face: DetectedFace, threshold: number): RecognizeResult {
    if (!MockSafrEngine.passesGate(face)) return { code: 160, face: null };
    // Measured ranges (on our device): same person 0.998-1.30 / poor conditions or unregistered 0.55-0.70
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

/** Box-Muller normal random numbers */
function gauss(mean: number, sd: number): number {
  const u = 1 - Math.random();
  const v = Math.random();
  return mean + sd * Math.sqrt(-2 * Math.log(u)) * Math.cos(2 * Math.PI * v);
}
