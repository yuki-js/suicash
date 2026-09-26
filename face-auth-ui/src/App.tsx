import { useCallback, useEffect, useState } from "react";
import { CameraView } from "./components/CameraView";
import { QualityMeter } from "./components/QualityMeter";
import { ResultView } from "./components/ResultView";
import { LogPanel, type LogEntry } from "./components/LogPanel";
import { Logo } from "./components/Logo";
import { initialControl, onControlMessage } from "./control";
import { createEngine } from "./engine";
import type { OpMode } from "./types";
import {
  MockSafrEngine,
  QUALITY_GATE,
  type DetectedFace,
  type RecognizeResult,
} from "./sim";

type Screen = "home" | "camera" | "processing" | "result" | "unlocked";
type FlowMode = "register" | "verify";

const PROBE_MS = 300; // プレビュー中の probe(検出のみ)間隔
const STABLE_FRAMES = 5; // 品質ゲートをこの回数連続で通ると自動キャプチャ(約1.5秒)

const initial = initialControl();

export default function App() {
  // createEngine は一度だけ呼ぶこと。NativeEngine は window.__safrResolve を
  // 自分の pending マップに向けるため、再レンダーで作り直すと応答が迷子になる。
  const [engine] = useState(createEngine);
  const [opMode, setOpMode] = useState<OpMode>(initial.mode);
  const [threshold, setThreshold] = useState(initial.threshold);
  const [screen, setScreen] = useState<Screen>("home");
  const [flow, setFlow] = useState<FlowMode>("verify");
  const [face, setFace] = useState<DetectedFace | null>(null);
  const [result, setResult] = useState<RecognizeResult | null>(null);
  const [engineReady, setEngineReady] = useState(engine.kind === "mock");
  const [registered, setRegistered] = useState(false);
  const [stable, setStable] = useState(0);
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [logOpen, setLogOpen] = useState(false);

  const log = useCallback((text: string) => {
    const time = new Date().toLocaleTimeString("ja-JP", { hour12: false });
    setLogs((prev) => [...prev.slice(-199), { time, text }]);
  }, []);

  // エンジン状態のポーリング(SAFR は初期化に数十秒かかることがある)
  useEffect(() => {
    log(`engine: ${engine.kind}`);
    let stopped = false;
    const tick = async () => {
      const s = await engine.status();
      if (stopped) return;
      setEngineReady((prev) => {
        if (!prev && s.ready) log("エンジン初期化完了");
        return s.ready;
      });
      setRegistered(s.registered);
    };
    tick();
    const id = window.setInterval(tick, 2000);
    return () => {
      stopped = true;
      window.clearInterval(id);
    };
  }, [log]);

  // 母艦 CLI からの制御コマンド(モード切替・閾値・store 消去)
  useEffect(() => {
    return onControlMessage((c) => {
      switch (c.cmd) {
        case "mode":
          setOpMode(c.value);
          setScreen("home");
          setResult(null);
          setFace(null);
          log(`facectl: mode ${c.value}`);
          break;
        case "threshold":
          setThreshold(c.value);
          log(`facectl: threshold ${c.value.toFixed(2)}`);
          break;
        case "store.clear":
          engine.clearStore().then(() => {
            setRegistered(false);
            log("facectl: store.clear");
          });
          break;
      }
    });
  }, [log]);

  // カメラ画面に入ったらネイティブカメラを起動、出たら停止
  useEffect(() => {
    if (screen !== "camera") return;
    engine.cameraStart();
    return () => {
      engine.cameraStop();
    };
  }, [screen, engine]);

  // カメラ画面中の検出ループ(probe)。応答待ちの間は次を出さない
  useEffect(() => {
    if (screen !== "camera") return;
    let stopped = false;
    let busy = false;
    const id = window.setInterval(async () => {
      if (busy || stopped) return;
      busy = true;
      try {
        const f = await engine.probe();
        if (!stopped) {
          setFace(f);
          setStable((s) => (f && MockSafrEngine.passesGate(f) ? s + 1 : 0));
        }
      } finally {
        busy = false;
      }
    }, PROBE_MS);
    return () => {
      stopped = true;
      window.clearInterval(id);
    };
  }, [screen, engine]);

  const capture = useCallback(async () => {
    setScreen("processing");
    log(flow === "register" ? "register 実行" : "recognize 実行");
    const started = Date.now();
    let r: RecognizeResult;
    if (flow === "register") {
      r = await engine.register();
      const s = await engine.status();
      setRegistered(s.registered);
      log(`register → code=${r.code}`);
    } else {
      r = await engine.recognize(threshold);
      log(
        `recognize → code=${r.code}` +
          (r.face ? ` confidence=${r.face.confidence.toFixed(3)}` : ""),
      );
    }
    // 結果画面が一瞬で切り替わらないよう最低表示時間を確保
    const wait = Math.max(0, 600 - (Date.now() - started));
    window.setTimeout(() => {
      setResult(r);
      setScreen("result");
    }, wait);
  }, [flow, threshold, log, engine]);

  // 品質ゲートを一定時間維持したら自動キャプチャ
  useEffect(() => {
    if (screen === "camera" && stable >= STABLE_FRAMES) {
      setStable(0);
      capture();
    }
  }, [screen, stable, capture]);

  const startCamera = (m: FlowMode) => {
    setFlow(m);
    setFace(null);
    setStable(0);
    setScreen("camera");
    log(m === "register" ? "登録フロー開始" : "照合フロー開始");
  };

  const goHome = () => {
    setScreen("home");
    setResult(null);
    setFace(null);
  };

  const gateOk = face !== null && MockSafrEngine.passesGate(face);
  const hint = !face
    ? "顔をワクの中に合わせてください"
    : !gateOk
      ? face.mask >= QUALITY_GATE.mask
        ? "マスクを外してください"
        : "明るい場所で、正面を向いてください"
      : stable > 0
        ? `そのままお待ちください… ${Math.min(100, Math.round((stable / STABLE_FRAMES) * 100))}%`
        : "顔を検出しました";

  return (
    <div className="app">
      <header className="app__header">
        <Logo inverted />
        <span className="app__headerTag">顔認証</span>
        {opMode === "enroll" && <span className="app__badge app__badge--enroll">登録モード</span>}
      </header>

      {opMode === "enroll" && screen === "home" && (
        <div className="modeBanner">
          係員操作中:顔登録モードです。切替は母艦 CLI から行います。
        </div>
      )}

      <main className="app__main">
        {screen === "home" && opMode === "normal" && (
          <div className="home">
            <div className="home__hero">
              <div className="home__faceMark" aria-hidden="true">
                <svg viewBox="0 0 96 96">
                  <rect x="6" y="6" width="20" height="6" rx="3" />
                  <rect x="6" y="6" width="6" height="20" rx="3" />
                  <rect x="70" y="6" width="20" height="6" rx="3" />
                  <rect x="84" y="6" width="6" height="20" rx="3" />
                  <rect x="6" y="84" width="20" height="6" rx="3" />
                  <rect x="6" y="70" width="6" height="20" rx="3" />
                  <rect x="70" y="84" width="20" height="6" rx="3" />
                  <rect x="84" y="70" width="6" height="20" rx="3" />
                  <ellipse cx="48" cy="42" rx="15" ry="18" className="home__faceHead" />
                  <path d="M26 88 C26 70 36 63 48 63 C60 63 70 70 70 88 Z" className="home__faceHead" />
                </svg>
              </div>
              <h1>顔でウォレットを開く</h1>
              <p>
                顔と IDi を紐づけて、ZKP 証明でウォレットをアンロックします。
                顔画像そのものがブロックチェーンに載ることはありません。
              </p>
            </div>

            <div className="home__actions">
              <button
                className="btn btn--primary btn--big"
                disabled={!engineReady}
                onClick={() => startCamera("verify")}
              >
                {engineReady ? "顔で認証する" : "エンジン初期化中…"}
              </button>
            </div>

            <p className="home__note">
              顔の登録がお済みでない方は、係員にお声がけください。
            </p>
          </div>
        )}

        {screen === "home" && opMode === "enroll" && (
          <div className="home">
            <div className="home__hero">
              <div className="home__faceMark home__faceMark--enroll" aria-hidden="true">
                <svg viewBox="0 0 96 96">
                  <ellipse cx="48" cy="42" rx="15" ry="18" className="home__faceHead" />
                  <path d="M26 88 C26 70 36 63 48 63 C60 63 70 70 70 88 Z" className="home__faceHead" />
                  <g className="home__plus">
                    <rect x="66" y="14" width="20" height="6" rx="3" />
                    <rect x="73" y="7" width="6" height="20" rx="3" />
                  </g>
                </svg>
              </div>
              <h1>顔を登録する</h1>
              <p>
                利用者の顔を撮影し、IDi と紐づけて登録します。
                登録データは端末メモリ内のみに保持され、アプリ終了で消えます。
              </p>
            </div>

            <div className="home__actions">
              <button
                className="btn btn--primary btn--big"
                disabled={!engineReady}
                onClick={() => startCamera("register")}
              >
                {engineReady ? "撮影して登録" : "エンジン初期化中…"}
              </button>
              <button
                className="btn btn--outline btn--big"
                disabled={!engineReady || !registered}
                onClick={() => startCamera("verify")}
              >
                登録した顔で確認照合
              </button>
            </div>

            <div className="enrollStatus">
              <span>engine {engine.kind}{engineReady ? "" : "(初期化中)"}</span>
              <span className={registered ? "enrollStatus--ok" : ""}>
                {registered ? "1 件登録済み" : "未登録"}
              </span>
              <span>閾値 {threshold.toFixed(2)}</span>
            </div>
          </div>
        )}

        {screen === "camera" && (
          <div className="capture">
            <CameraView
              face={face}
              gateOk={gateOk}
              hint={hint}
              nativePreview={engine.kind === "safr"}
            />
            <div className="capture__meters">
              <QualityMeter label="姿勢 (cpq)" value={face?.centerPoseQuality ?? null} gate={QUALITY_GATE.cpq} />
              <QualityMeter label="コントラスト" value={face?.contrastQuality ?? null} gate={QUALITY_GATE.contrast} />
              <QualityMeter label="鮮明さ" value={face?.sharpnessQuality ?? null} gate={QUALITY_GATE.sharpness} />
              <QualityMeter label="マスク" value={face?.mask ?? null} gate={QUALITY_GATE.mask} invert />
            </div>
            <div className="capture__actions">
              <button className="btn btn--primary" disabled={!gateOk} onClick={() => capture()}>
                {flow === "register" ? "この顔を登録" : "この顔で照合"}
              </button>
              <button className="btn btn--ghost" onClick={goHome}>キャンセル</button>
            </div>
          </div>
        )}

        {screen === "processing" && (
          <div className="processing">
            <div className="processing__spinner" />
            <p>{flow === "register" ? "顔を登録しています…" : "照合しています…"}</p>
          </div>
        )}

        {screen === "result" && result && (
          <ResultView
            mode={flow}
            result={result}
            threshold={threshold}
            showUnlock={opMode === "normal"}
            onRetry={() => startCamera(flow)}
            onHome={goHome}
            onUnlock={() => {
              log("ZKP 証明を生成 → ウォレットをアンロック(モック)");
              setScreen("unlocked");
            }}
          />
        )}

        {screen === "unlocked" && (
          <div className="unlocked">
            <div className="unlocked__ring">
              <svg viewBox="0 0 48 48" aria-hidden="true">
                <path d="M14 22 v-6 a10 10 0 0 1 20 0 v6" fill="none" strokeWidth="4" strokeLinecap="round" className="unlocked__shackle" />
                <rect x="10" y="22" width="28" height="20" rx="5" className="unlocked__body" />
              </svg>
            </div>
            <h2>ウォレットをアンロックしました</h2>
            <p className="unlocked__sub">
              IDi と顔の ZKP 証明が検証されました。残高照会・チャージ・送金が利用できます。
            </p>
            <div className="unlocked__balance">
              <span className="unlocked__balanceLabel">残高(デモ)</span>
              <span className="unlocked__balanceValue">1,250 <small>SUI</small></span>
            </div>
            <div className="result__actions">
              <button className="btn btn--primary" onClick={goHome}>ホームへ戻る</button>
            </div>
          </div>
        )}
      </main>

      {initial.debug && (
        <LogPanel entries={logs} open={logOpen} onToggle={() => setLogOpen((o) => !o)} />
      )}

      <footer className="app__footer">
        <span>
          SuiCash face-auth UI — engine: {engine.kind}
          {engine.kind === "mock" ? "(実エンジン未接続)" : ""}
        </span>
      </footer>
    </div>
  );
}
