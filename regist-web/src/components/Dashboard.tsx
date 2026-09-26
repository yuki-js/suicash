import { useCallback, useEffect, useState } from "react";
import type { Registration } from "../lib/storage";
import { fetchBalance, requestCharge } from "../lib/wallet";
import { formatCardNumber, maskIdi } from "../lib/idi";

interface Props {
  reg: Registration;
  address: string;
  onReset: () => void;
}

/** 登録完了後のダッシュボード(残高・チャージ) */
export function Dashboard({ reg, address, onReset }: Props) {
  const [balance, setBalance] = useState<string | null>(null);
  const [charging, setCharging] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  const refresh = useCallback(async () => {
    setBalance(await fetchBalance(address));
  }, [address]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const charge = async () => {
    setCharging(true);
    setMessage(null);
    const ok = await requestCharge(address);
    if (ok) {
      setMessage("チャージしました(testnet faucet)");
      // faucet の反映には少し時間がかかる
      window.setTimeout(refresh, 3000);
    } else {
      setMessage("チャージに失敗しました(faucet が混雑中の可能性)。時間をおいてお試しください。");
    }
    setCharging(false);
  };

  const copyAddress = async () => {
    try {
      await navigator.clipboard.writeText(address);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1500);
    } catch {
      // クリップボード不可の環境では何もしない
    }
  };

  return (
    <div className="dashboard">
      <section className="card card--balance">
        <span className="balance__label">残高(Sui testnet)</span>
        <div className="balance__value">
          {balance === null ? "--" : balance} <small>SUI</small>
        </div>
        <div className="balance__actions">
          <button className="btn btn--primary" disabled={charging} onClick={charge}>
            {charging ? "チャージ中…" : "チャージする"}
          </button>
          <button className="btn btn--ghost" onClick={refresh}>
            残高を更新
          </button>
        </div>
        {message && <p className="balance__message">{message}</p>}
      </section>

      <section className="card">
        <h2 className="card__title">登録情報</h2>
        <dl className="info">
          <div className="info__row">
            <dt>カード番号</dt>
            <dd>{formatCardNumber(reg.cardNumber)}</dd>
          </div>
          <div className="info__row">
            <dt>IDi(発行ID)</dt>
            <dd className="info__mono">{maskIdi(reg.idi)}</dd>
          </div>
          <div className="info__row">
            <dt>顔認証</dt>
            <dd className="info__ok">利用登録済み(顔データ非収集)</dd>
          </div>
          <div className="info__row">
            <dt>ウォレット</dt>
            <dd className="info__mono">
              {address.slice(0, 10)}…{address.slice(-6)}
              <button className="info__copy" onClick={copyAddress}>
                {copied ? "コピーしました" : "コピー"}
              </button>
            </dd>
          </div>
        </dl>
        <p className="card__note">
          お支払いは、お店の認証端末に顔をかざしてご利用いただけます。
        </p>
      </section>

      <button className="btn btn--danger-ghost" onClick={onReset}>
        登録をやり直す(この端末の登録情報を消去)
      </button>
    </div>
  );
}
