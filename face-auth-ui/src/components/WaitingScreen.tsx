import { mistToSui } from "../terminal";

interface Props {
  /** null = idle (balance inquiry only) / when awaiting payment, the debit amount (MIST) */
  payment: { amount: string } | null;
}

/** Card waiting screen. Shows the host CLI setting (balance inquiry / payment N) at the top */
export function WaitingScreen({ payment }: Props) {
  return (
    <div className="waiting">
      <div className={`waiting__badge ${payment ? "waiting__badge--pay" : ""}`}>
        {payment ? `Payment: ${mistToSui(payment.amount)} SUI` : "Balance inquiry"}
      </div>

      <div className="waiting__mark" aria-hidden="true">
        <svg viewBox="0 0 120 120">
          <circle cx="60" cy="60" r="52" className="waiting__ring" />
          <path
            d="M42 54 h30 M42 62 h30 M42 70 h20"
            className="waiting__wave"
            fill="none"
            strokeLinecap="round"
          />
          <rect x="40" y="38" width="40" height="30" rx="5" className="waiting__card" />
        </svg>
      </div>

      <h1 className="waiting__title">Tap your card</h1>
      <p className="waiting__sub">
        Hold your Suica / PASMO over the reader
      </p>
    </div>
  );
}
