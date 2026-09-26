import { mistToSui } from "../terminal";

interface Props {
  ok: boolean;
  amount: string; // MIST
  balanceAfter: string; // MIST
  error?: string;
  onDone: () => void;
}

/**
 * Shows the payment result like a ticket gate LCD.
 * Transaction type on top, deducted amount (large) in the middle, balance at bottom right, following transit gate displays.
 */
export function GateLcd({ ok, amount, balanceAfter, error, onDone }: Props) {
  return (
    <div className={`lcd ${ok ? "lcd--ok" : "lcd--ng"}`} onClick={onDone}>
      <div className="lcd__panel">
        <div className="lcd__top">
          <span className="lcd__kind">{ok ? "Payment" : "Error"}</span>
          <span className="lcd__mark">{ok ? "○" : "×"}</span>
        </div>

        {ok ? (
          <div className="lcd__mid">
            <span className="lcd__amountLabel">Paid</span>
            <span className="lcd__amount">
              {mistToSui(amount)}
              <span className="lcd__unit">SUI</span>
            </span>
          </div>
        ) : (
          <div className="lcd__mid lcd__mid--error">
            <span className="lcd__errText">{error || "Could not process"}</span>
          </div>
        )}

        <div className="lcd__bottom">
          <span className="lcd__balLabel">Balance</span>
          <span className="lcd__bal">
            {mistToSui(balanceAfter)}
            <span className="lcd__unit lcd__unit--sm">SUI</span>
          </span>
        </div>
      </div>
      <p className="lcd__hint">Tap the screen to go back</p>
    </div>
  );
}
