import type { RecognizeResult } from "../sim";

const CODE_LABEL: Record<number, string> = {
  0: "成功",
  65: "登録失敗",
  66: "精度不足",
  160: "顔検出失敗",
};

interface Props {
  mode: "register" | "verify";
  result: RecognizeResult;
  threshold: number;
  /** 通常運用モードのみ true。登録モードの確認照合ではアンロック導線を出さない */
  showUnlock: boolean;
  onRetry: () => void;
  onHome: () => void;
  onUnlock: () => void;
}

/** 照合結果画面。confidence を最も大きく表示する */
export function ResultView({ mode, result, threshold, showUnlock, onRetry, onHome, onUnlock }: Props) {
  const ok = result.code === 0;
  const conf = result.face?.confidence ?? null;

  if (mode === "register") {
    return (
      <div className={`result ${ok ? "result--ok" : "result--ng"}`}>
        <div className="result__icon">{ok ? "✓" : "✕"}</div>
        <h2 className="result__title">{ok ? "顔を登録しました" : "登録できませんでした"}</h2>
        <p className="result__note">
          {ok
            ? "登録はこの端末のメモリ内のみに保持され、アプリを閉じると消去されます。"
            : `品質しきい値を満たしませんでした(コード ${result.code}: ${CODE_LABEL[result.code]})。明るい場所で正面から撮り直してください。`}
        </p>
        <div className="result__actions">
          {ok ? (
            <button className="btn btn--primary" onClick={onHome}>ホームへ戻る</button>
          ) : (
            <button className="btn btn--primary" onClick={onRetry}>もう一度</button>
          )}
          {!ok && <button className="btn btn--ghost" onClick={onHome}>ホームへ戻る</button>}
        </div>
      </div>
    );
  }

  return (
    <div className={`result ${ok ? "result--ok" : "result--ng"}`}>
      <div className="result__icon">{ok ? "✓" : "✕"}</div>
      <h2 className="result__title">{ok ? "本人確認できました" : "一致しませんでした"}</h2>

      {conf !== null ? (
        <div className="result__score">
          <div className="result__confidence">{conf.toFixed(3)}</div>
          <div className="result__thresholdRow">
            <span>confidence</span>
            <span className="result__vs">{conf >= threshold ? "≥" : "<"}</span>
            <span>閾値 {threshold.toFixed(2)}</span>
          </div>
        </div>
      ) : (
        <p className="result__note">
          顔を検出できませんでした(コード {result.code}: {CODE_LABEL[result.code]})。
        </p>
      )}

      {ok && showUnlock ? (
        <div className="result__unlock">
          <p>本人確認が端末内で完了しました。ウォレットをアンロックできます。</p>
          <button className="btn btn--accent" onClick={onUnlock}>
            ウォレットをアンロック
          </button>
        </div>
      ) : (
        result.code === 66 && (
          <p className="result__note">
            スコアが閾値に届きませんでした。顔を登録済みか確認し、正面から再度お試しください。
          </p>
        )
      )}

      <div className="result__actions">
        <button className="btn btn--primary" onClick={onRetry}>もう一度</button>
        <button className="btn btn--ghost" onClick={onHome}>ホームへ戻る</button>
      </div>
    </div>
  );
}
