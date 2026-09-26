import { useState } from "react";

interface Props {
  onCreate: () => void;
}

/** Sui ウォレット発行ステップ */
export function WalletStep({ onCreate }: Props) {
  const [busy, setBusy] = useState(false);

  return (
    <section className="card">
      <h2 className="card__title">2. SuiCash ウォレットを発行</h2>
      <p className="card__desc">
        お支払い用の Sui ウォレットを発行します。鍵はこの端末内で生成・保管され、
        カード番号(IDi)からは導出しません。
      </p>
      <button
        className="btn btn--primary btn--big"
        disabled={busy}
        onClick={() => {
          setBusy(true);
          onCreate();
        }}
      >
        {busy ? "発行中…" : "ウォレットを発行する"}
      </button>
      <p className="card__note">ネットワーク: Sui testnet(デモ)</p>
    </section>
  );
}
