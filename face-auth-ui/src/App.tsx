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
import { subscribe, report, setLed, type LedMode, type TerminalEvent } from "./terminal";

/**
 * UI state machine for the payment terminal (Hi-CARA).
 *
 * The host (PC) reads FeliCa, authenticates the IDi via the oracle, and handles on-chain payment.
 * The terminal switches screens on host events (card / mode / paymentResult):
 *
 *   waiting (waiting for card)
 *     └ card → unregistered: registerPrompt
 *             registered & no face on terminal: enroll (face registration) → proceed as authenticated
 *             registered & face on terminal:    auth (face auth)
 *        └ face OK → idle mode: balance (balance inquiry) / payment mode: paying → gateLcd
 *
 * Faces are kept on the terminal (keyed by IDi, per session). Face matching happens entirely
 * on the terminal and face data never leaves it (ZKP is local only).
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

  /** IDi whose face is enrolled on the terminal (per session). The SAFR store holds one person, so track the latest one */
  const enrolledIdi = useRef<string | null>(null);

  const log = useCallback((text: string) => {
    const time = new Date().toLocaleTimeString("en-US", { hour12: false });
    setLogs((prev) => [...prev.slice(-199), { time, text }]);
  }, []);

  // Wait for engine init (SAFR takes tens of seconds)
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

  // Drive the top LED according to screen transitions
  useEffect(() => {
    let mode: LedMode;
    switch (screen) {
      case "detecting":
      case "enroll":
      case "auth":
        mode = "blue_blink"; // blink blue while authenticating
        break;
      case "balance":
        mode = "green"; // identity verified
        break;
      case "registerPrompt":
        mode = "red"; // unregistered
        break;
      case "lcd":
        mode = lcd?.ok ? "green" : "red"; // payment success = green / failure (insufficient balance etc.) = red
        break;
      case "paying":
        mode = "blue_blink";
        break;
      default:
        mode = "off"; // idle
    }
    setLed(mode);
  }, [screen, lcd]);

  // Subscribe to host events
  useEffect(() => {
    return subscribe((e: TerminalEvent) => {
      switch (e.type) {
        case "detecting":
          // Immediate reaction to card detection (auth follows). Keep the screen if already processing
          setScreen((s) => (s === "waiting" ? "detecting" : s));
          break;
        case "mode":
          setPayMode(e.payment);
          log(`Host: ${e.payment ? `awaiting payment ${e.payment.amount} MIST` : "balance inquiry mode"}`);
          break;
        case "card": {
          log(`card idi=${e.idi} registered=${e.registered} bal=${e.balance}`);
          const c = { idi: e.idi, registered: e.registered, balance: e.balance };
          setCard(c);
          if (!c.registered) {
            setScreen("registerPrompt");
          } else if (enrolledIdi.current !== c.idi) {
            setScreen("enroll"); // registered on-chain + no face on terminal → enroll face
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

  // After face OK: idle = balance inquiry / payment = request transfer
  const onFaceResult = useCallback(
    (ok: boolean, _conf: number | null, mode: "enroll" | "auth") => {
      if (!card) return;
      if (!ok) {
        report({ type: "faceNg", idi: card.idi });
        log("Face auth failed");
        // On failure return to idle (a retry flow would also work in production)
        goWaiting();
        return;
      }
      if (mode === "enroll") {
        enrolledIdi.current = card.idi;
        report({ type: "enrolled", idi: card.idi });
      }
      report({ type: "faceOk", idi: card.idi });
      if (payMode) {
        // Host executes the transfer → wait for paymentResult
        setScreen("paying");
      } else {
        setScreen("balance"); // idle mode: balance inquiry only
      }
    },
    [card, payMode, log, goWaiting],
  );

  return (
    <div className="app app--terminal">
      <header className="app__header">
        <Logo inverted />
        <span className="app__headerTag">Payment Terminal</span>
        {!engineReady && <span className="app__badge app__badge--enroll">Engine starting…</span>}
      </header>

      <main className="app__main">
        {screen === "waiting" && <WaitingScreen payment={payMode} />}

        {screen === "detecting" && (
          <div className="processing">
            <div className="processing__spinner" />
            <p>Authenticating card…</p>
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
            <p>Processing payment…</p>
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
        SuiCash Payment Terminal — engine: {engine.kind}
        {engine.kind === "mock" ? " (real engine not connected)" : ""}
      </footer>
    </div>
  );
}

/** Debug panel, shown only with ?debug=1, for manually emitting host events (browser testing) */
function DebugPanel() {
  const emit = (event: TerminalEvent) =>
    window.postMessage({ source: "suicash-facepay", event }, "*");
  const demoIdi = "05d5807e28260205";
  return (
    <div className="debugPanel">
      <span className="debugPanel__label">DEBUG Host Simulator</span>
      <div className="debugPanel__row">
        <button onClick={() => emit({ type: "mode", payment: null })}>Balance mode</button>
        <button onClick={() => emit({ type: "mode", payment: { amount: "300000000" } })}>Payment 0.3</button>
      </div>
      <div className="debugPanel__row">
        <button onClick={() => emit({ type: "card", idi: demoIdi, registered: true, balance: "1000000000" })}>Registered card</button>
        <button onClick={() => emit({ type: "card", idi: "ffffffffffffffff", registered: false, balance: "0" })}>Unregistered card</button>
      </div>
      <div className="debugPanel__row">
        <button onClick={() => emit({ type: "paymentResult", ok: true, amount: "300000000", balanceAfter: "700000000", digest: "DEMO" })}>Payment OK</button>
        <button onClick={() => emit({ type: "cardRemoved" })}>Remove card</button>
      </div>
    </div>
  );
}
