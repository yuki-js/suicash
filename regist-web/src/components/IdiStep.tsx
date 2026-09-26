import { useEffect, useRef, useState } from "react";
import { formatCardNumber, isValidCardNumber, normalizeCardNumber, toIdi } from "../lib/idi";
import { CARD_ASPECT, ID_ROI, ocrIdCanvas } from "../lib/ocr";
import type { Registration } from "../lib/storage";

interface Props {
  onDone: (r: Pick<Registration, "cardNumber" | "idi">) => void;
}

/** カメラ OCR が使えるか(secure context + getUserMedia) */
function cameraAvailable(): boolean {
  return !!(window.isSecureContext && navigator.mediaDevices?.getUserMedia);
}

/**
 * カード番号(裏面の券面番号)の登録ステップ。
 * 入力された券面番号は 8 バイト IDi へ変換して確定する。
 * カメラで券面を読み取って入力欄にプリフィルできる(結果は必ず人が確認して確定)。
 * カメラが使えない環境(非 HTTPS 等)では手入力のみ。
 */
export function IdiStep({ onDone }: Props) {
  const [mode, setMode] = useState<"manual" | "camera">(() =>
    cameraAvailable() ? "camera" : "manual",
  );
  const [raw, setRaw] = useState("");
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const cardNumber = normalizeCardNumber(raw);
  const idi = toIdi(cardNumber);
  const valid = isValidCardNumber(cardNumber);

  const submit = () => {
    if (!valid || !idi || busy) return;
    setBusy(true);
    setError(null);
    try {
      onDone({ cardNumber, idi });
    } catch {
      setError("登録処理に失敗しました。もう一度お試しください。");
      setBusy(false);
    }
  };

  return (
    <section className="card">
      <h2 className="card__title">1. カード番号を登録</h2>
      <p className="card__desc">
        お手持ちの交通系 IC カード<strong>裏面右下</strong>に記載の
        <strong>ID 番号</strong>(例 NR807 E200 1060 0517)を登録します。
      </p>

      {mode === "camera" ? (
        <CardScanner
          onResult={(text) => {
            setRaw(formatCardNumber(text));
            setNotice("読み取り結果を確認し、間違いがあれば修正してください。");
            setMode("manual");
          }}
          onError={(msg) => setNotice(msg)}
          onManual={() => setMode("manual")}
        />
      ) : (
        <>
          <label className="field">
            <span className="field__label">カード番号(裏面右下の ID)</span>
            <input
              className="field__input"
              inputMode="text"
              autoCapitalize="characters"
              autoCorrect="off"
              spellCheck={false}
              placeholder="NR807 E200 1060 0517"
              value={raw}
              onChange={(e) => {
                setRaw(e.target.value);
                setNotice(null);
              }}
            />
          </label>

          {cardNumber.length > 0 && (
            <p className={`field__check ${valid ? "field__check--ok" : ""}`}>
              {valid
                ? `✓ ${formatCardNumber(cardNumber)}`
                : "カード裏面右下の ID 番号を確認してください(対応外の発行会社の可能性)"}
            </p>
          )}

          {notice && <p className="field__notice">{notice}</p>}
          {error && <p className="field__error">{error}</p>}

          <button className="btn btn--primary btn--big" disabled={!valid || busy} onClick={submit}>
            {busy ? "処理中…" : "この番号で登録する"}
          </button>

          {cameraAvailable() && (
            <button className="btn btn--ghost" onClick={() => setMode("camera")}>
              カメラで読み取る
            </button>
          )}
        </>
      )}

      <p className="card__note">
        番号はこの端末内にのみ保存されます。撮影画像もこの端末内でのみ処理され、
        外部へ送信されることはありません。
      </p>
    </section>
  );
}

/** 券面スキャナ: 背面カメラ + カードガイド枠 + ID 行 ROI の OCR */
function CardScanner({
  onResult,
  onError,
  onManual,
}: {
  onResult: (text: string) => void;
  onError: (msg: string) => void;
  onManual: () => void;
}) {
  const videoRef = useRef<HTMLVideoElement>(null);
  const boxRef = useRef<HTMLDivElement>(null);
  const [state, setState] = useState<"starting" | "on" | "failed">("starting");
  const [reading, setReading] = useState(false);

  useEffect(() => {
    let stream: MediaStream | null = null;
    let cancelled = false;
    (async () => {
      try {
        stream = await navigator.mediaDevices.getUserMedia({
          video: { facingMode: "environment", width: { ideal: 1920 }, height: { ideal: 1080 } },
          audio: false,
        });
        if (cancelled) {
          stream.getTracks().forEach((t) => t.stop());
          return;
        }
        const v = videoRef.current;
        if (v) {
          v.srcObject = stream;
          v.muted = true;
          v.play().catch(() => {});
          setState("on");
        }
      } catch {
        if (!cancelled) setState("failed");
      }
    })();
    return () => {
      cancelled = true;
      stream?.getTracks().forEach((t) => t.stop());
    };
  }, []);

  // ガイド枠(コンテナ座標系・割合): 幅 90%、カード比率、中央
  const guide = { w: 0.9, x: 0.05, y: 0 }; // y は実行時に高さから算出

  const capture = async () => {
    const v = videoRef.current;
    const box = boxRef.current;
    if (!v || !box || v.videoWidth === 0 || reading) return;
    setReading(true);
    try {
      const cw = box.clientWidth;
      const ch = box.clientHeight;
      const gw = guide.w * cw;
      const gh = gw / CARD_ASPECT;
      const gx = guide.x * cw;
      const gy = (ch - gh) / 2;
      // ID 行(ガイド枠に対する割合 → コンテナ座標)
      const ix = gx + ID_ROI.x0 * gw;
      const iy = gy + ID_ROI.y0 * gh;
      const iw = (ID_ROI.x1 - ID_ROI.x0) * gw;
      const ih = (ID_ROI.y1 - ID_ROI.y0) * gh;
      // object-fit: cover の写像でコンテナ座標 → 動画ピクセル座標
      const s = Math.max(cw / v.videoWidth, ch / v.videoHeight);
      const dx = (v.videoWidth * s - cw) / 2;
      const dy = (v.videoHeight * s - ch) / 2;
      const vx = (ix + dx) / s;
      const vy = (iy + dy) / s;
      const vw = iw / s;
      const vh = ih / s;

      const outW = 800;
      const outH = Math.round((outW * vh) / vw);
      const canvas = document.createElement("canvas");
      canvas.width = outW;
      canvas.height = outH;
      canvas.getContext("2d")!.drawImage(v, vx, vy, vw, vh, 0, 0, outW, outH);

      const text = await ocrIdCanvas(canvas);
      if (text.length >= 8) {
        onResult(text);
      } else {
        onError("読み取れませんでした。明るい場所で、枠にぴったり合わせてもう一度お試しください。");
      }
    } catch {
      onError("読み取りに失敗しました。手入力もご利用いただけます。");
    } finally {
      setReading(false);
    }
  };

  return (
    <div className="scan">
      <div className="scan__box" ref={boxRef}>
        {state !== "failed" ? (
          <video ref={videoRef} className="scan__video" autoPlay playsInline muted />
        ) : (
          <div className="scan__failed">カメラを起動できませんでした</div>
        )}
        {/* カードガイド枠 + ID 行ハイライト */}
        <div className="scan__guide">
          <div className="scan__idbox" />
        </div>
        <div className="scan__caption">
          カード<strong>裏面</strong>を枠の中に合わせてください
        </div>
      </div>
      <div className="scan__actions">
        <button
          className="btn btn--primary btn--big"
          disabled={state !== "on" || reading}
          onClick={capture}
        >
          {reading ? "読み取り中…" : "読み取る"}
        </button>
        <button className="btn btn--ghost" onClick={onManual}>
          手で入力する
        </button>
      </div>
    </div>
  );
}
