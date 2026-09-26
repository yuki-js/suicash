import { useState } from "react";

interface Props {
  idi: string;
  address: string | null;
  onCreate: () => void;
}

/** Step that derives (issues) the AA wallet from the IDi */
export function WalletStep({ address, onCreate }: Props) {
  const [busy, setBusy] = useState(false);

  return (
    <section className="card">
      <h2 className="card__title">2. Issue wallet</h2>
      <p className="card__desc">
        We derive a Sui wallet for payments from your card number (IDi).
        The same card always yields the same wallet, so top-ups reach you
        with just the card number.
      </p>

      {address ? (
        <div className="field">
          <span className="field__label">Wallet address (derived from IDi)</span>
          <p className="field__check field__check--ok info__mono" style={{ wordBreak: "break-all" }}>
            {address}
          </p>
        </div>
      ) : (
        <p className="card__desc">Computing address…</p>
      )}

      <button
        className="btn btn--primary btn--big"
        disabled={!address || busy}
        onClick={() => {
          setBusy(true);
          onCreate();
        }}
      >
        {busy ? "Issuing…" : "Issue this wallet"}
      </button>

      <p className="card__note">
        Network: Sui testnet (demo). Since the wallet is derived deterministically
        from the card number, funds are protected at payment time by on-terminal face authentication + a ZK proof.
      </p>
    </section>
  );
}
