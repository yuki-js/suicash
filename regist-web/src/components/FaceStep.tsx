import { useEffect, useState } from "react";

interface Props {
  onDone: () => void;
}

/**
 * 顔登録ステップ(ダミー)。
 *
 * プライバシー方針により、このデモでは顔データを一切収集しない
 * (カメラも起動しない)。実際の顔照合は決済時に認証端末(Hi-CARA)の
 * 内部だけで行われる。ここでは「登録手続きが済んだ」というフラグのみ立てる。
 */
export function FaceStep({ onDone }: Props) {
  const [progress, setProgress] = useState<number | null>(null);

  useEffect(() => {
    if (progress === null) return;
    if (progress >= 100) {
      const t = window.setTimeout(onDone, 600);
      return () => window.clearTimeout(t);
    }
    const t = window.setTimeout(() => setProgress((p) => (p ?? 0) + 4), 80);
    return () => window.clearTimeout(t);
  }, [progress, onDone]);

  return (
    <section className="card">
      <h2 className="card__title">3. 顔認証の利用登録</h2>

      <div className={`facePreview ${progress !== null ? "facePreview--active" : ""}`}>
        <svg viewBox="0 0 96 96" aria-hidden="true">
          <ellipse cx="48" cy="40" rx="16" ry="19" className="facePreview__head" />
          <path d="M24 88 C24 68 35 60 48 60 C61 60 72 68 72 88 Z" className="facePreview__head" />
        </svg>
        {progress !== null && (
          <div className="facePreview__bar">
            <div className="facePreview__fill" style={{ width: `${Math.min(100, progress)}%` }} />
          </div>
        )}
      </div>

      {progress === null ? (
        <>
          <p className="card__desc">
            顔認証のご利用手続きを行います。お支払い時の顔照合は
            <strong>お店の認証端末の中だけ</strong>で行われます。
          </p>
          <button className="btn btn--primary btn--big" onClick={() => setProgress(0)}>
            利用登録をはじめる
          </button>
        </>
      ) : (
        <p className="card__desc">
          {progress < 100 ? "登録手続きを実行しています…" : "登録手続きが完了しました"}
        </p>
      )}

      <p className="card__note">
        【デモ版のご案内】プライバシー保護のため、この画面では顔の撮影・
        顔データの収集を行いません(ダミー登録)。製品版でも顔データが
        端末の外へ送信・保存されることはありません。
      </p>
    </section>
  );
}
