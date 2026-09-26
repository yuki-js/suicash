import { useCallback, useEffect, useState } from "react";
import { CameraView } from "./CameraView";
import { QualityMeter } from "./QualityMeter";
import { MockSafrEngine, QUALITY_GATE, type DetectedFace } from "../sim";
import type { FaceEngine } from "../engine";

const PROBE_MS = 300;
const STABLE_FRAMES = 5; // 品質ゲートをこの回数連続で通ると自動キャプチャ(約1.5秒)

interface Props {
  engine: FaceEngine;
  mode: "enroll" | "auth";
  threshold: number;
  onResult: (ok: boolean, confidence: number | null) => void;
  onCancel: () => void;
  log?: (s: string) => void;
}

/** 顔の登録(enroll)/照合(auth)。品質ゲートを一定時間維持で自動キャプチャ */
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

  // 検出ループ
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

  // ゲート維持で自動キャプチャ
  useEffect(() => {
    if (!busy && stable >= STABLE_FRAMES) {
      setStable(0);
      capture();
    }
  }, [stable, busy, capture]);

  const gateOk = face !== null && MockSafrEngine.passesGate(face);
  const hint = busy
    ? mode === "enroll" ? "登録しています…" : "照合しています…"
    : !face
      ? "顔をワクの中に合わせてください"
      : !gateOk
        ? face.mask >= QUALITY_GATE.mask
          ? "マスクを外してください"
          : "明るい場所で、正面を向いてください"
        : `そのままお待ちください… ${Math.min(100, Math.round((stable / STABLE_FRAMES) * 100))}%`;

  return (
    <div className="capture">
      <div className="capture__head">
        <span className="capture__title">{mode === "enroll" ? "顔を登録します" : "顔認証"}</span>
      </div>
      <CameraView face={face} gateOk={gateOk} hint={hint} nativePreview={engine.kind === "safr"} />
      <div className="capture__meters">
        <QualityMeter label="姿勢 (cpq)" value={face?.centerPoseQuality ?? null} gate={QUALITY_GATE.cpq} />
        <QualityMeter label="コントラスト" value={face?.contrastQuality ?? null} gate={QUALITY_GATE.contrast} />
        <QualityMeter label="鮮明さ" value={face?.sharpnessQuality ?? null} gate={QUALITY_GATE.sharpness} />
        <QualityMeter label="マスク" value={face?.mask ?? null} gate={QUALITY_GATE.mask} invert />
      </div>
      <div className="capture__actions">
        <button className="btn btn--primary" disabled={!gateOk || busy} onClick={capture}>
          {mode === "enroll" ? "この顔を登録" : "この顔で認証"}
        </button>
        <button className="btn btn--ghost" onClick={onCancel}>キャンセル</button>
      </div>
    </div>
  );
}
