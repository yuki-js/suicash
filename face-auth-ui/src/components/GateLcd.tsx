import { mistToSui } from "../terminal";

interface Props {
  ok: boolean;
  amount: string; // MIST
  balanceAfter: string; // MIST
  error?: string;
  onDone: () => void;
}

/**
 * 決済結果を自動改札の LCD 風に表示する。
 * 上段に処理種別、中央に引き去り額(大)、右下に残額 —— 交通系改札の表示を踏襲。
 */
export function GateLcd({ ok, amount, balanceAfter, error, onDone }: Props) {
  return (
    <div className={`lcd ${ok ? "lcd--ok" : "lcd--ng"}`} onClick={onDone}>
      <div className="lcd__panel">
        <div className="lcd__top">
          <span className="lcd__kind">{ok ? "支払い" : "エラー"}</span>
          <span className="lcd__mark">{ok ? "○" : "×"}</span>
        </div>

        {ok ? (
          <div className="lcd__mid">
            <span className="lcd__amountLabel">引去</span>
            <span className="lcd__amount">
              {mistToSui(amount)}
              <span className="lcd__unit">SUI</span>
            </span>
          </div>
        ) : (
          <div className="lcd__mid lcd__mid--error">
            <span className="lcd__errText">{error || "処理できませんでした"}</span>
          </div>
        )}

        <div className="lcd__bottom">
          <span className="lcd__balLabel">残額</span>
          <span className="lcd__bal">
            {mistToSui(balanceAfter)}
            <span className="lcd__unit lcd__unit--sm">SUI</span>
          </span>
        </div>
      </div>
      <p className="lcd__hint">画面をタッチで戻る</p>
    </div>
  );
}
