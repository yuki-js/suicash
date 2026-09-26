import {
  MockSafrEngine,
  type DetectedFace,
  type RecognizeResult,
} from "./sim";

/**
 * 顔認証エンジンの抽象層。
 *
 * 端末上では WebView シェルが公開する JS ブリッジ(window.SafrNative)経由で
 * 実エンジン(SAFR eSDK)を呼び、ブリッジが無い環境(ブラウザ開発時)では
 * モックへ自動フォールバックする。
 *
 * カメラはネイティブ側が所有する。probe / register / recognize は
 * ネイティブが自分で現在フレームを取るため、JS から画像は送らない。
 * プレビュー映像は window.__safrFrame(dataUrl) として push されてくる
 * (CameraView が受けて表示する)。
 */

export type EngineKind = "safr" | "mock";

export interface EngineStatus {
  ready: boolean;
  registered: boolean;
}

export interface FaceEngine {
  readonly kind: EngineKind;
  status(): Promise<EngineStatus>;
  /** 検出のみ(プレビューの品質メーター用)。顔が無ければ null */
  probe(): Promise<DetectedFace | null>;
  register(): Promise<RecognizeResult>;
  recognize(threshold: number): Promise<RecognizeResult>;
  clearStore(): Promise<void>;
  /** カメラ画面に入る/出るときに呼ぶ(ネイティブのプレビュー起動・停止) */
  cameraStart(): Promise<void>;
  cameraStop(): Promise<void>;
}

declare global {
  interface Window {
    /** WebView シェルが addJavascriptInterface で公開するブリッジ */
    SafrNative?: {
      request(id: string, method: string, payload: string): void;
    };
    /** シェル側が evaluateJavascript で呼ぶ応答コールバック */
    __safrResolve?: (id: string, json: string) => void;
    /** シェル側が push するプレビューフレーム(dataURL) */
    __safrFrame?: (dataUrl: string) => void;
  }
}

/* eslint-disable @typescript-eslint/no-explicit-any */

class NativeEngine implements FaceEngine {
  readonly kind = "safr" as const;
  private seq = 0;
  private pending = new Map<string, (v: any) => void>();

  constructor() {
    window.__safrResolve = (id, json) => {
      const resolve = this.pending.get(id);
      if (!resolve) return;
      this.pending.delete(id);
      try {
        resolve(JSON.parse(json));
      } catch {
        resolve(null);
      }
    };
  }

  private call(method: string): Promise<any> {
    return new Promise((resolve) => {
      const id = String(++this.seq);
      this.pending.set(id, resolve);
      try {
        window.SafrNative!.request(id, method, "");
      } catch {
        this.pending.delete(id);
        resolve(null);
      }
    });
  }

  async status(): Promise<EngineStatus> {
    const r = await this.call("status");
    return { ready: !!(r && r.ready), registered: !!(r && r.registered) };
  }

  async probe(): Promise<DetectedFace | null> {
    const r = await this.call("probe");
    if (!r || !r.found) return null;
    return {
      confidence: 0,
      centerPoseQuality: num(r.cpq),
      contrastQuality: num(r.contrast),
      sharpnessQuality: num(r.sharpness),
      mask: num(r.mask),
      bounds: { x: num(r.x), y: num(r.y), w: num(r.w), h: num(r.h) },
      extraRotation: 0,
    };
  }

  async register(): Promise<RecognizeResult> {
    const r = await this.call("register");
    return r && r.ok ? { code: 0, face: null } : { code: 65, face: null };
  }

  async recognize(threshold: number): Promise<RecognizeResult> {
    const r = await this.call("match");
    if (!r || !r.ok) return { code: 160, face: null };
    const confidence = num(r.confidence);
    const face: DetectedFace = {
      confidence,
      centerPoseQuality: 0,
      contrastQuality: 0,
      sharpnessQuality: 0,
      mask: num(r.mask),
      bounds: { x: 0, y: 0, w: 0, h: 0 },
      extraRotation: 0,
    };
    return { code: confidence >= threshold ? 0 : 66, face };
  }

  async clearStore(): Promise<void> {
    await this.call("clearStore");
  }

  async cameraStart(): Promise<void> {
    await this.call("cameraStart");
  }

  async cameraStop(): Promise<void> {
    await this.call("cameraStop");
  }
}

function num(v: unknown): number {
  const n = Number(v);
  return Number.isFinite(n) ? n : 0;
}

/**
 * モックのアダプタ。実エンジンの「直前の detectFaces のキャッシュに対して
 * learn/recognize が動く」挙動に合わせ、最後の probe 結果を使う。
 */
class MockAdapter implements FaceEngine {
  readonly kind = "mock" as const;
  private engine = new MockSafrEngine();
  private last: DetectedFace | null = null;

  async status(): Promise<EngineStatus> {
    return { ready: true, registered: this.engine.hasPerson };
  }

  async probe(): Promise<DetectedFace | null> {
    this.last = this.engine.detectFaces();
    return this.last;
  }

  async register(): Promise<RecognizeResult> {
    if (!this.last) return { code: 65, face: null };
    this.engine.clearPersonStore();
    return this.engine.learnPerson(this.last);
  }

  async recognize(threshold: number): Promise<RecognizeResult> {
    if (!this.last) return { code: 160, face: null };
    return this.engine.recognizePerson(this.last, threshold);
  }

  async clearStore(): Promise<void> {
    this.engine.clearPersonStore();
  }

  async cameraStart(): Promise<void> {}

  async cameraStop(): Promise<void> {}
}

export function createEngine(): FaceEngine {
  return window.SafrNative ? new NativeEngine() : new MockAdapter();
}
