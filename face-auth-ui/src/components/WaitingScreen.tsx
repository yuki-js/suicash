import { mistToSui } from "../terminal";
import { SuiCashCard } from "./SuiCashCard";

interface Props {
  /** null=待機(残高照会のみ)/ 決済待機なら引き落とし額(MIST) */
  payment: { amount: string } | null;
}

/** カード待ち受け画面。母艦 CLI の設定(残高照会 / 決済 N)を上部に示す */
export function WaitingScreen({ payment }: Props) {
  return (
    <div className="waiting">
      <div className={`waiting__badge ${payment ? "waiting__badge--pay" : ""}`}>
        {payment ? `決済待機 ・ ${mistToSui(payment.amount)} SUI` : "残高照会モード"}
      </div>

      <div className="waiting__mark" aria-hidden="true">
        <svg viewBox="0 0 120 120" className="waiting__ringSvg">
          <circle cx="60" cy="60" r="52" className="waiting__ring" />
        </svg>
        <SuiCashCard className="waiting__cardArt" />
      </div>

      <h1 className="waiting__title">カードをタッチ</h1>
      <p className="waiting__sub">
        Suica / PASMO をリーダーにかざしてください
      </p>
    </div>
  );
}
