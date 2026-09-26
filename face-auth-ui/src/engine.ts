import {
  MockSafrEngine,
  type DetectedFace,
  type RecognizeResult,
} from "./sim";

/**
 * Abstraction layer for the face recognition engine.
 *
 * On the device it calls the real engine (SAFR eSDK) via the JS bridge
 * (window.SafrNative) exposed by the WebView shell; where there is no bridge
 * (browser development) it automatically falls back to the mock.
 *
 * The native side owns the camera. For probe / register / recognize the native
 * side grabs the current frame itself, so JS never sends images.
 * Preview frames are pushed as window.__safrFrame(dataUrl)
 * (received and displayed by CameraView).
 */

export type EngineKind = "safr" | "mock";

export interface EngineStatus {
  ready: boolean;
  registered: boolean;
}

export interface FaceEngine {
  readonly kind: EngineKind;
  status(): Promise<EngineStatus>;
  /** Detection only (for the preview quality meters). null if no face */
  probe(): Promise<DetectedFace | null>;
  register(): Promise<RecognizeResult>;
  recognize(threshold: number): Promise<RecognizeResult>;
  clearStore(): Promise<void>;
  /** Call when entering/leaving the camera screen (starts/stops the native preview) */
  cameraStart(): Promise<void>;
  cameraStop(): Promise<void>;
}

declare global {
  interface Window {
    /** Bridge exposed by the WebView shell via addJavascriptInterface */
    SafrNative?: {
      request(id: string, method: string, payload: string): void;
    };
    /** Response callback the shell invokes via evaluateJavascript */
    __safrResolve?: (id: string, json: string) => void;
    /** Preview frame (dataURL) pushed by the shell */
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
 * Mock adapter. Uses the last probe result, matching the real engine's behavior
 * where learn/recognize operate on the cache from the preceding detectFaces.
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
