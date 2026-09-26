import { mistToSui } from "../terminal";

interface Props {
  balance: string; // MIST
  onDone: () => void;
}

/** Balance inquiry (shown after passing face auth in idle mode) */
export function BalanceInquiry({ balance, onDone }: Props) {
  return (
    <div className="balanceView" onClick={onDone}>
      <div className="balanceView__icon">✓</div>
      <h2 className="balanceView__title">Identity verified</h2>
      <div className="balanceView__card">
        <span className="balanceView__label">Balance</span>
        <span className="balanceView__value">
          {mistToSui(balance)} <small>SUI</small>
        </span>
      </div>
      <p className="balanceView__hint">Tap the screen to go back</p>
    </div>
  );
}
