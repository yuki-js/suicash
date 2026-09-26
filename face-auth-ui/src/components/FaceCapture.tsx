import { useCallback, useEffect, useState } from "react";
import { CameraView } from "./CameraView";
import { QualityMeter } from "./QualityMeter";
import { MockSafrEngine, QUALITY_GATE, type DetectedFace } from "../sim";
import type { FaceEngine } from "../engine";

const PROBE_MS = 150;
const STABLE_FRAMES = 1; // auto-capture after passing the quality gate this many consecutive times (~1.5s)

interface Props {
  engine: FaceEngine;
  mode: "enroll" | "auth";
  threshold: number;
  onResult: (ok: boolean, confidence: number | null) => void;
  onCancel: () => void;
  log?: (s: string) => void;
}

/** Face enrollment (enroll) / matching (auth). Auto-captures once the quality gate holds for a while */
export function FaceCapture({ engine, mode, threshold, onResult, onCancel, log }: Props) {
  const [face, setFace] = useState<DetectedFace | null>(null);
  const [stable, setStable] = useState(0);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    engine.cameraStart();
    return () => {
      engine.cameraStop();
    };
  }, [engine]);

  const capture = useCallback(async () => {
    if (busy) return;
    setBusy(true);
    try {
      if (mode === "enroll") {
        const r = await engine.register();
        log?.(`register → code=${r.code}`);
        onResult(r.code === 0, null);
      } else {
        const r = await engine.recognize(threshold);
        const conf = r.face?.confidence ?? null;
        log?.(`recognize → code=${r.code}` + (conf !== null ? ` conf=${conf.toFixed(3)}` : ""));
        onResult(r.code === 0, conf);
      }
    } finally {
      setBusy(false);
    }
  }, [busy, mode, engine, threshold, onResult, log]);

  // Detection loop
  useEffect(() => {
    let stopped = false;
    let inflight = false;
    const id = window.setInterval(async () => {
      if (inflight || stopped || busy) return;
      inflight = true;
      try {
        const f = await engine.probe();
        if (!stopped) {
          setFace(f);
          setStable((s) => (f && MockSafrEngine.passesGate(f) ? s + 1 : 0));
        }
      } finally {
        inflight = false;
      }
    }, PROBE_MS);
    return () => {
      stopped = true;
      window.clearInterval(id);
    };
  }, [engine, busy]);

  // Auto-capture while the gate holds
  useEffect(() => {
    if (!busy && stable >= STABLE_FRAMES) {
      setStable(0);
      capture();
    }
  }, [stable, busy, capture]);

  const gateOk = face !== null && MockSafrEngine.passesGate(face);
  const hint = busy
    ? mode === "enroll" ? "Registering…" : "Verifying…"
    : !face
      ? "Fit your face inside the frame"
      : !gateOk
        ? face.mask >= QUALITY_GATE.mask
          ? "Please remove your mask"
          : "Face forward in a well-lit place"
        : `Hold still… ${Math.min(100, Math.round((stable / STABLE_FRAMES) * 100))}%`;

  return (
    <div className="capture">
      <div className="capture__head">
        <span className="capture__title">{mode === "enroll" ? "Face Registration" : "Face Authentication"}</span>
      </div>
      <CameraView face={face} gateOk={gateOk} hint={hint} nativePreview={engine.kind === "safr"} />
      <div className="capture__meters">
        <QualityMeter label="Pose (cpq)" value={face?.centerPoseQuality ?? null} gate={QUALITY_GATE.cpq} />
        <QualityMeter label="Contrast" value={face?.contrastQuality ?? null} gate={QUALITY_GATE.contrast} />
        <QualityMeter label="Sharpness" value={face?.sharpnessQuality ?? null} gate={QUALITY_GATE.sharpness} />
        <QualityMeter label="Mask" value={face?.mask ?? null} gate={QUALITY_GATE.mask} invert />
      </div>
      <div className="capture__actions">
        <button className="btn btn--primary" disabled={!gateOk || busy} onClick={capture}>
          {mode === "enroll" ? "Register this face" : "Verify this face"}
        </button>
        <button className="btn btn--ghost" onClick={onCancel}>Cancel</button>
      </div>
    </div>
  );
}
