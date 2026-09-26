import { useEffect, useState } from "react";
import { Logo } from "./components/Logo";
import { IdiStep } from "./components/IdiStep";
import { WalletStep } from "./components/WalletStep";
import { FaceStep } from "./components/FaceStep";
import { Dashboard } from "./components/Dashboard";
import {
  clearRegistration,
  loadRegistration,
  saveRegistration,
  type Registration,
} from "./lib/storage";
import { resolveAddress } from "./lib/wallet";

type Step = "idi" | "wallet" | "face" | "done";

const STEP_LABELS: [Step, string][] = [
  ["idi", "Card No."],
  ["wallet", "Wallet"],
  ["face", "Face ID"],
  ["done", "Done"],
];

/**
 * SuiCash registration site (for users' smartphones).
 * Register card number (IDi) -> derive AA wallet from IDi -> enroll in face
 * authentication (dummy) -> top up. Face matching at payment happens inside the store terminal.
 */
export default function App() {
  const [reg, setReg] = useState<Registration | null>(() => loadRegistration());
  const [address, setAddress] = useState<string | null>(null);

  // The wallet is derived deterministically from the IDi (so the router can resolve IDi -> address).
  useEffect(() => {
    if (!reg) {
      setAddress(null);
      return;
    }
    let alive = true;
    resolveAddress(reg.idi).then((addr) => {
      if (alive) setAddress(addr);
    });
    return () => {
      alive = false;
    };
  }, [reg]);

  const step: Step = !reg
    ? "idi"
    : !reg.walletCreated
      ? "wallet"
      : !reg.faceEnrolled
        ? "face"
        : "done";
  const stepIndex = STEP_LABELS.findIndex(([s]) => s === step);

  const reset = () => {
    clearRegistration();
    setReg(null);
    setAddress(null);
  };

  return (
    <div className="app">
      <header className="app__header">
        <Logo inverted />
        <span className="app__headerTag">Registration</span>
      </header>

      {step !== "done" && (
        <ol className="stepper">
          {STEP_LABELS.map(([s, label], i) => (
            <li
              key={s}
              className={`stepper__item ${i < stepIndex ? "stepper__item--done" : ""} ${i === stepIndex ? "stepper__item--now" : ""}`}
            >
              <span className="stepper__dot">{i < stepIndex ? "✓" : i + 1}</span>
              <span className="stepper__label">{label}</span>
            </li>
          ))}
        </ol>
      )}

      <main className="app__main">
        {step === "idi" && (
          <IdiStep
            onDone={(r) => {
              const full: Registration = { ...r, walletCreated: false, faceEnrolled: false };
              saveRegistration(full);
              setReg(full);
            }}
          />
        )}

        {step === "wallet" && reg && (
          <WalletStep
            idi={reg.idi}
            address={address}
            onCreate={() => {
              const next = { ...reg, walletCreated: true };
              saveRegistration(next);
              setReg(next);
            }}
          />
        )}

        {step === "face" && reg && (
          <FaceStep
            onDone={() => {
              const next = { ...reg, faceEnrolled: true };
              saveRegistration(next);
              setReg(next);
            }}
          />
        )}

        {step === "done" && reg && address && (
          <Dashboard reg={reg} address={address} onReset={reset} />
        )}
      </main>

      <footer className="app__footer">
        SuiCash registration (demo) — face data and card numbers are never sent externally
      </footer>
    </div>
  );
}
