import { useCallback, useEffect, useState } from "react";
import type { Registration } from "../lib/storage";
import { fetchBalance, requestCharge } from "../lib/wallet";
import { formatCardNumber, maskIdi } from "../lib/idi";

interface Props {
  reg: Registration;
  address: string;
  onReset: () => void;
}

/** Dashboard after registration (balance, top-up) */
export function Dashboard({ reg, address, onReset }: Props) {
  const [balance, setBalance] = useState<string | null>(null);
  const [charging, setCharging] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [cooldown, setCooldown] = useState(0);
  const [copied, setCopied] = useState(false);

  const refresh = useCallback(async () => {
    setBalance(await fetchBalance(address));
  }, [address]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  // Cooldown countdown
  useEffect(() => {
    if (cooldown <= 0) return;
    const t = window.setTimeout(() => setCooldown((c) => c - 1), 1000);
    return () => window.clearTimeout(t);
  }, [cooldown]);

  const charge = async () => {
    setCharging(true);
    setMessage(null);
    const res = await requestCharge(address);
    setMessage(res.message);
    if (res.ok) {
      // Faucet funds take a moment to show up
      window.setTimeout(refresh, 3000);
    } else if (res.retryAfterSec) {
      setCooldown(res.retryAfterSec);
    }
    setCharging(false);
  };

  const copyAddress = async () => {
    try {
      await navigator.clipboard.writeText(address);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1500);
    } catch {
      // Do nothing where the clipboard is unavailable
    }
  };

  return (
    <div className="dashboard">
      <section className="card card--balance">
        <span className="balance__label">Balance (Sui testnet)</span>
        <div className="balance__value">
          {balance === null ? "--" : balance} <small>SUI</small>
        </div>
        <div className="balance__actions">
          <button
            className="btn btn--primary"
            disabled={charging || cooldown > 0}
            onClick={charge}
          >
            {charging ? "Topping up…" : cooldown > 0 ? `Retry in ${cooldown}s` : "Top up"}
          </button>
          <button className="btn btn--ghost" onClick={refresh}>
            Refresh
          </button>
        </div>
        {message && <p className="balance__message">{message}</p>}
      </section>

      <section className="card">
        <h2 className="card__title">Registration</h2>
        <dl className="info">
          <div className="info__row">
            <dt>Card No.</dt>
            <dd>{formatCardNumber(reg.cardNumber)}</dd>
          </div>
          <div className="info__row">
            <dt>IDi (issuance ID)</dt>
            <dd className="info__mono">{maskIdi(reg.idi)}</dd>
          </div>
          <div className="info__row">
            <dt>Face ID</dt>
            <dd className="info__ok">Enrolled (no face data collected)</dd>
          </div>
          <div className="info__row">
            <dt>Wallet</dt>
            <dd className="info__mono">
              {address.slice(0, 10)}…{address.slice(-6)}
              <button className="info__copy" onClick={copyAddress}>
                {copied ? "Copied" : "Copy"}
              </button>
            </dd>
          </div>
        </dl>
        <p className="card__note">
          To pay, just show your face to the store's authentication terminal.
        </p>
      </section>

      <button className="btn btn--danger-ghost" onClick={onReset}>
        Start over (erase registration on this device)
      </button>
    </div>
  );
}
