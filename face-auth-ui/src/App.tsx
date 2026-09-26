import { useCallback, useEffect, useRef, useState } from "react";
import { Logo } from "./components/Logo";
import { LogPanel, type LogEntry } from "./components/LogPanel";
import { WaitingScreen } from "./components/WaitingScreen";
import { RegisterPrompt } from "./components/RegisterPrompt";
import { FaceCapture } from "./components/FaceCapture";
import { BalanceInquiry } from "./components/BalanceInquiry";
import { GateLcd } from "./components/GateLcd";
import { createEngine } from "./engine";
import { DEFAULT_THRESHOLD } from "./sim";
import { subscribe, report, type TerminalEvent } from "./terminal";

/**
 * 決済端末(Hi-CARA)の UI 状態機械。
 *
 * 母艦(PC)が FeliCa を読み、オラクルで IDi を認証し、オンチェーン決済まで担う。
 * 端末は母艦からのイベント(card / mode / paymentResult)で画面を切り替える:
 *
 *   waiting(カード待ち)
 *     └ card → 未登録: registerPrompt
 *             登録済み & 端末に顔なし: enroll(顔登録)→ 認証済み扱いで次へ
 *             登録済み & 顔あり:       auth(顔認証)
 *        └ 顔OK → 待機モード: balance(残高照会) / 決済モード: paying → gateLcd
 *
 * 顔は端末内に保持(IDi キー、セッション内)。実際の顔照合は端末内で完結し、
 * 顔データは端末の外へ出ない(ZKP はローカルのみ)。
 */

type Card = { idi: string; registered: boolean; balance: string };
type PayMode = { amount: string } | null;
type Screen =
  | "waiting"
  | "detecting"
  | "registerPrompt"
  | "enroll"
  | "auth"
  | "balance"
  | "paying"
  | "lcd";

const DEBUG = new URLSearchParams(window.location.search).get("debug") === "1";

export default function App() {
  const [engine] = useState(createEngine);
  const [engineReady, setEngineReady] = useState(engine.kind === "mock");
  const [payMode, setPayMode] = useState<PayMode>(null);
  const [screen, setScreen] = useState<Screen>("waiting");
  const [card, setCard] = useState<Card | null>(null);
  const [lcd, setLcd] = useState<{ ok: boolean; amount: string; balanceAfter: string; error?: string } | null>(null);
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [logOpen, setLogOpen] = useState(false);

  /** 端末内に顔を登録済みの IDi(セッション内)。SAFR ストアは1名なので直近1件を追跡 */
  const enrolledIdi = useRef<string | null>(null);

  const log = useCallback((text: string) => {
    const time = new Date().toLocaleTimeString("ja-JP", { hour12: false });
    setLogs((prev) => [...prev.slice(-199), { time, text }]);
  }, []);

  // エンジン初期化待ち(SAFR は数十秒)
  useEffect(() => {
    log(`engine: ${engine.kind}`);
    let stop = false;
    const tick = async () => {
      const s = await engine.status();
      if (!stop) setEngineReady((prev) => (prev || s.ready));
    };
    tick();
    const id = window.setInterval(tick, 2000);
    return () => {
      stop = true;
      window.clearInterval(id);
    };
  }, [engine, log]);

  const goWaiting = useCallback(() => {
    setScreen("waiting");
    setCard(null);
    setLcd(null);
  }, []);

  // 母艦イベントの購読
  useEffect(() => {
    return subscribe((e: TerminalEvent) => {
      switch (e.type) {
        case "detecting":
          // カード検出の即時反応(認証はこの後)。既に処理中の画面なら維持
          setScreen((s) => (s === "waiting" ? "detecting" : s));
          break;
        case "mode":
          setPayMode(e.payment);
          log(`母艦: ${e.payment ? `決済待機 ${e.payment.amount} MIST` : "残高照会モード"}`);
          break;
        case "card": {
          log(`card idi=${e.idi} registered=${e.registered} bal=${e.balance}`);
          const c = { idi: e.idi, registered: e.registered, balance: e.balance };
          setCard(c);
          if (!c.registered) {
            setScreen("registerPrompt");
          } else if (enrolledIdi.current !== c.idi) {
            setScreen("enroll"); // オンチェーン登録あり + 端末に顔なし → 顔登録
          } else {
            setScreen("auth");
          }
          break;
        }
        case "cardRemoved":
          log("card removed");
          goWaiting();
          break;
        case "paymentResult":
          log(`payment ok=${e.ok} amount=${e.amount} after=${e.balanceAfter} ${e.digest ?? e.error ?? ""}`);
          setLcd({ ok: e.ok, amount: e.amount, balanceAfter: e.balanceAfter, error: e.error });
          setScreen("lcd");
          break;
      }
    });
  }, [log, goWaiting]);

  // 顔OK後の分岐: 待機=残高照会 / 決済=送金依頼
  const onFaceResult = useCallback(
    (ok: boolean, _conf: number | null, mode: "enroll" | "auth") => {
      if (!card) return;
      if (!ok) {
        report({ type: "faceNg", idi: card.idi });
        log("顔認証NG");
        // NG は待機へ戻す(実運用はリトライ導線でもよい)
        goWaiting();
        return;
      }
      if (mode === "enroll") {
        enrolledIdi.current = card.idi;
        report({ type: "enrolled", idi: card.idi });
      }
      report({ type: "faceOk", idi: card.idi });
      if (payMode) {
        // 母艦が送金を実行 → paymentResult を待つ
        setScreen("paying");
      } else {
        setScreen("balance"); // 待機モード: 残高照会のみ
      }
    },
    [card, payMode, log, goWaiting],
  );

  return (
    <div className="app app--terminal">
      <header className="app__header">
        <Logo inverted />
        <span className="app__headerTag">決済端末</span>
        {!engineReady && <span className="app__badge app__badge--enroll">エンジン初期化中</span>}
      </header>

      <main className="app__main">
        {screen === "waiting" && <WaitingScreen payment={payMode} />}

        {screen === "detecting" && (
          <div className="processing">
            <div className="processing__spinner" />
            <p>カードを認証しています…</p>
          </div>
        )}

        {screen === "registerPrompt" && <RegisterPrompt onDone={goWaiting} />}

        {screen === "enroll" && card && (
          <FaceCapture
            engine={engine}
            mode="enroll"
            threshold={DEFAULT_THRESHOLD}
            onResult={(ok, conf) => onFaceResult(ok, conf, "enroll")}
            onCancel={() => {
              report({ type: "cancel", idi: card.idi });
              goWaiting();
            }}
            log={log}
          />
        )}

        {screen === "auth" && card && (
          <FaceCapture
            engine={engine}
            mode="auth"
            threshold={DEFAULT_THRESHOLD}
            onResult={(ok, conf) => onFaceResult(ok, conf, "auth")}
            onCancel={() => {
              report({ type: "cancel", idi: card.idi });
              goWaiting();
            }}
            log={log}
          />
        )}

        {screen === "balance" && card && (
          <BalanceInquiry balance={card.balance} onDone={goWaiting} />
        )}

        {screen === "paying" && (
          <div className="processing">
            <div className="processing__spinner" />
            <p>決済しています…</p>
          </div>
        )}

        {screen === "lcd" && lcd && (
          <GateLcd
            ok={lcd.ok}
            amount={lcd.amount}
            balanceAfter={lcd.balanceAfter}
            error={lcd.error}
            onDone={goWaiting}
          />
        )}
      </main>

      {DEBUG && <DebugPanel />}
      {DEBUG && <LogPanel entries={logs} open={logOpen} onToggle={() => setLogOpen((o) => !o)} />}

      <footer className="app__footer">
        SuiCash 決済端末 — engine: {engine.kind}
        {engine.kind === "mock" ? "(実エンジン未接続)" : ""}
      </footer>
    </div>
  );
}

/** ?debug=1 のときだけ出る、母艦イベントを手で流すデバッグパネル(ブラウザ検証用) */
function DebugPanel() {
  const emit = (event: TerminalEvent) =>
    window.postMessage({ source: "suicash-facepay", event }, "*");
  const demoIdi = "05d5807e28260205";
  return (
    <div className="debugPanel">
      <span className="debugPanel__label">DEBUG 母艦シミュレータ</span>
      <div className="debugPanel__row">
        <button onClick={() => emit({ type: "mode", payment: null })}>残高照会モード</button>
        <button onClick={() => emit({ type: "mode", payment: { amount: "300000000" } })}>決済待機 0.3</button>
      </div>
      <div className="debugPanel__row">
        <button onClick={() => emit({ type: "card", idi: demoIdi, registered: true, balance: "1000000000" })}>登録済みカード</button>
        <button onClick={() => emit({ type: "card", idi: "ffffffffffffffff", registered: false, balance: "0" })}>未登録カード</button>
      </div>
      <div className="debugPanel__row">
        <button onClick={() => emit({ type: "paymentResult", ok: true, amount: "300000000", balanceAfter: "700000000", digest: "DEMO" })}>決済成功</button>
        <button onClick={() => emit({ type: "cardRemoved" })}>カード離す</button>
      </div>
    </div>
  );
}
