import { mistToSui } from "../terminal";

interface Props {
  balance: string; // MIST
  onDone: () => void;
}

/** 残高照会(待機モードで顔認証を通したあとの表示) */
export function BalanceInquiry({ balance, onDone }: Props) {
  return (
    <div className="balanceView" onClick={onDone}>
      <div className="balanceView__icon">✓</div>
      <h2 className="balanceView__title">本人確認できました</h2>
      <div className="balanceView__card">
        <span className="balanceView__label">残高</span>
        <span className="balanceView__value">
          {mistToSui(balance)} <small>SUI</small>
        </span>
      </div>
      <p className="balanceView__hint">画面をタッチで戻る</p>
    </div>
  );
}
