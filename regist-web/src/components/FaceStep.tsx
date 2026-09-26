import { useEffect, useRef, useState } from "react";

interface Props {
  onDone: () => void;
}

type Phase = "intro" | "camera" | "scanning" | "fallback";

/** カメラ(getUserMedia)が使えるか */
function cameraAvailable(): boolean {
  return !!(window.isSecureContext && navigator.mediaDevices?.getUserMedia);
}

/**
 * 顔認証の利用登録ステップ。
 *
 * 実際に前面カメラを起動してライブプレビューと撮影演出を見せる。ただし
 * プライバシー方針により、撮ったフレームは保存も送信もしない(その場で破棄)。
 * 実際の顔照合は決済時に認証端末(Hi-CARA)の内部だけで行われる。
 * カメラが使えない環境ではダミー進捗にフォールバックする。
 */
export function FaceStep({ onDone }: Props) {
  const [phase, setPhase] = useState<Phase>("intro");
  const [scanPct, setScanPct] = useState(0);
  const videoRef = useRef<HTMLVideoElement>(null);
  const streamRef = useRef<MediaStream | null>(null);

  // カメラ起動/停止
  useEffect(() => {
    if (phase !== "camera" && phase !== "scanning") return;
    let cancelled = false;
    (async () => {
      try {
        const stream = await navigator.mediaDevices.getUserMedia({
          video: { facingMode: "user", width: { ideal: 720 }, height: { ideal: 960 } },
          audio: false,
        });
        if (cancelled) {
          stream.getTracks().forEach((t) => t.stop());
          return;
        }
        streamRef.current = stream;
        const v = videoRef.current;
        if (v) {
          v.srcObject = stream;
          v.muted = true;
          v.play().catch(() => {});
        }
      } catch {
        if (!cancelled) setPhase("fallback");
      }
    })();
    return () => {
      cancelled = true;
      streamRef.current?.getTracks().forEach((t) => t.stop());
      streamRef.current = null;
    };
  }, [phase]);

  // 撮影演出(スキャン)。完了したら顔フレームは破棄して次へ
  useEffect(() => {
    if (phase !== "scanning") return;
    if (scanPct >= 100) {
      // フレームは保存・送信しない(その場で破棄)。カメラを止めて完了。
      streamRef.current?.getTracks().forEach((t) => t.stop());
      streamRef.current = null;
      const t = window.setTimeout(onDone, 500);
      return () => window.clearTimeout(t);
    }
    const t = window.setTimeout(() => setScanPct((p) => p + 5), 70);
    return () => window.clearTimeout(t);
  }, [phase, scanPct, onDone]);

  // ダミー進捗(カメラ不可)
  const [dummyPct, setDummyPct] = useState<number | null>(null);
  useEffect(() => {
    if (dummyPct === null) return;
    if (dummyPct >= 100) {
      const t = window.setTimeout(onDone, 500);
      return () => window.clearTimeout(t);
    }
    const t = window.setTimeout(() => setDummyPct((p) => (p ?? 0) + 4), 80);
    return () => window.clearTimeout(t);
  }, [dummyPct, onDone]);

  return (
    <section className="card">
      <h2 className="card__title">3. 顔認証の利用登録</h2>

      {phase === "intro" && (
        <>
          <div className="facePreview">
            <svg viewBox="0 0 96 96" aria-hidden="true">
              <ellipse cx="48" cy="40" rx="16" ry="19" className="facePreview__head" />
              <path d="M24 88 C24 68 35 60 48 60 C61 60 72 68 72 88 Z" className="facePreview__head" />
            </svg>
          </div>
          <p className="card__desc">
            顔認証のご利用手続きを行います。お支払い時の顔照合は
            <strong>お店の認証端末の中だけ</strong>で行われます。
          </p>
          <button
            className="btn btn--primary btn--big"
            onClick={() => setPhase(cameraAvailable() ? "camera" : "fallback")}
          >
            顔の登録をはじめる
          </button>
        </>
      )}

      {(phase === "camera" || phase === "scanning") && (
        <>
          <div className="faceCam">
            <video ref={videoRef} className="faceCam__video" autoPlay playsInline muted />
            <div className={`faceCam__guide ${phase === "scanning" ? "faceCam__guide--scan" : ""}`} />
            {phase === "scanning" && (
              <div className="faceCam__scanline" style={{ top: `${scanPct}%` }} />
            )}
            <div className="faceCam__hint">
              {phase === "scanning" ? `登録しています… ${scanPct}%` : "顔をワクに合わせてください"}
            </div>
          </div>
          {phase === "camera" && (
            <button
              className="btn btn--primary btn--big"
              onClick={() => {
                setScanPct(0);
                setPhase("scanning");
              }}
            >
              この顔で登録する
            </button>
          )}
        </>
      )}

      {phase === "fallback" && (
        <>
          <div className={`facePreview ${dummyPct !== null ? "facePreview--active" : ""}`}>
            <svg viewBox="0 0 96 96" aria-hidden="true">
              <ellipse cx="48" cy="40" rx="16" ry="19" className="facePreview__head" />
              <path d="M24 88 C24 68 35 60 48 60 C61 60 72 68 72 88 Z" className="facePreview__head" />
            </svg>
            {dummyPct !== null && (
              <div className="facePreview__bar">
                <div className="facePreview__fill" style={{ width: `${Math.min(100, dummyPct)}%` }} />
              </div>
            )}
          </div>
          {dummyPct === null ? (
            <>
              <p className="card__desc">
                カメラを利用できないため、簡易登録を行います。
              </p>
              <button className="btn btn--primary btn--big" onClick={() => setDummyPct(0)}>
                利用登録をはじめる
              </button>
            </>
          ) : (
            <p className="card__desc">
              {dummyPct < 100 ? "登録手続きを実行しています…" : "登録手続きが完了しました"}
            </p>
          )}
        </>
      )}

      <p className="card__note">
        【プライバシー】撮影した顔画像・顔データは保存も送信もしません(その場で破棄)。
        実際の顔照合は決済時に認証端末の内部だけで行われ、そこでも顔データは
        端末の外へ出ません。
      </p>
    </section>
  );
}
