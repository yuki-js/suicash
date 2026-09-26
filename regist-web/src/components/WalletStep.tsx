import { useState } from "react";

interface Props {
  idi: string;
  address: string | null;
  onCreate: () => void;
}

/** IDi から AA ウォレットを導出(発行)するステップ */
export function WalletStep({ address, onCreate }: Props) {
  const [busy, setBusy] = useState(false);

  return (
    <section className="card">
      <h2 className="card__title">2. ウォレットを発行</h2>
      <p className="card__desc">
        カード番号(IDi)から、お支払い用の Sui ウォレットを導出します。
        同じカードからは必ず同じウォレットが導出されるため、チャージは
        カード番号だけで届きます。
      </p>

      {address ? (
        <div className="field">
          <span className="field__label">ウォレットアドレス(IDi から導出)</span>
          <p className="field__check field__check--ok info__mono" style={{ wordBreak: "break-all" }}>
            {address}
          </p>
        </div>
      ) : (
        <p className="card__desc">アドレスを計算しています…</p>
      )}

      <button
        className="btn btn--primary btn--big"
        disabled={!address || busy}
        onClick={() => {
          setBusy(true);
          onCreate();
        }}
      >
        {busy ? "発行中…" : "このウォレットを発行する"}
      </button>

      <p className="card__note">
        ネットワーク: Sui testnet(デモ)。ウォレットはカード番号から決定的に
        導出されるため、資金の保護は決済時の「顔認証(端末内)+ ZKP 証明」が担います。
      </p>
    </section>
  );
}
