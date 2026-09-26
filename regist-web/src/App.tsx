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
  ["idi", "カード番号"],
  ["wallet", "ウォレット"],
  ["face", "顔認証"],
  ["done", "完了"],
];

/**
 * SuiCash 登録サイト(利用者のスマホ向け)。
 * カード番号(IDi)の登録 → IDi から AA ウォレットを導出 → 顔認証の利用登録
 * (ダミー)→ チャージ。決済時の顔照合は店舗の認証端末内で行われる。
 */
export default function App() {
  const [reg, setReg] = useState<Registration | null>(() => loadRegistration());
  const [address, setAddress] = useState<string | null>(null);

  // ウォレットは IDi から決定的に導出する(ルータが IDi→アドレスを解決できる)。
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
        <span className="app__headerTag">登録サイト</span>
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
        SuiCash 登録サイト(デモ)— 顔データ・カード番号が外部へ送信されることはありません
      </footer>
    </div>
  );
}
