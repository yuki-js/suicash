import { useEffect, useRef, useState } from "react";
import { formatCardNumber, isValidCardNumber, normalizeCardNumber, toIdi } from "../lib/idi";
import { CARD_ASPECT, ID_ROI, ocrIdCanvas } from "../lib/ocr";
import type { Registration } from "../lib/storage";

interface Props {
  onDone: (r: Pick<Registration, "cardNumber" | "idi">) => void;
}

/** Whether camera OCR is available (secure context + getUserMedia) */
function cameraAvailable(): boolean {
  return !!(window.isSecureContext && navigator.mediaDevices?.getUserMedia);
}

/**
 * Card number (printed on the back) registration step.
 * The entered card number is converted to the 8-byte IDi and confirmed.
 * The camera can read the card to prefill the input (a human always confirms the result).
 * Manual entry only where the camera is unavailable (non-HTTPS, etc.).
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
      setError("Registration failed. Please try again.");
      setBusy(false);
    }
  };

  return (
    <section className="card">
      <h2 className="card__title">1. Register card number</h2>
      <p className="card__desc">
        Register the <strong>ID number</strong> printed at the
        <strong>bottom right of the back</strong> of your transit IC card (e.g. NR807 E200 1060 0517).
      </p>

      {mode === "camera" ? (
        <CardScanner
          onResult={(text) => {
            setRaw(formatCardNumber(text));
            setNotice("Please check the scanned result and fix any mistakes.");
            setMode("manual");
          }}
          onError={(msg) => setNotice(msg)}
          onManual={() => setMode("manual")}
        />
      ) : (
        <>
          <label className="field">
            <span className="field__label">Card number (ID at bottom right of back)</span>
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
                : "Check the ID number at the bottom right of the card back (the issuer may be unsupported)"}
            </p>
          )}

          {notice && <p className="field__notice">{notice}</p>}
          {error && <p className="field__error">{error}</p>}

          <button className="btn btn--primary btn--big" disabled={!valid || busy} onClick={submit}>
            {busy ? "Processing…" : "Register this number"}
          </button>

          {cameraAvailable() && (
            <button className="btn btn--ghost" onClick={() => setMode("camera")}>
              Scan with camera
            </button>
          )}
        </>
      )}

      <p className="card__note">
        The number is stored only on this device. Captured images are also processed
        only on this device and are never sent externally.
      </p>
    </section>
  );
}

/** Card scanner: rear camera + card guide frame + OCR of the ID line ROI */
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

  // Guide frame (container coords, fractions): 90% width, card aspect, centered
  const guide = { w: 0.9, x: 0.05, y: 0 }; // y is computed from the height at runtime

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
      // ID line (fractions of the guide frame -> container coords)
      const ix = gx + ID_ROI.x0 * gw;
      const iy = gy + ID_ROI.y0 * gh;
      const iw = (ID_ROI.x1 - ID_ROI.x0) * gw;
      const ih = (ID_ROI.y1 - ID_ROI.y0) * gh;
      // Map container coords -> video pixel coords via object-fit: cover
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
        onError("Couldn't read the card. Try again in a bright spot, aligned tightly with the frame.");
      }
    } catch {
      onError("Scan failed. You can also enter the number manually.");
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
          <div className="scan__failed">Couldn't start the camera</div>
        )}
        {/* Card guide frame + ID line highlight */}
        <div className="scan__guide">
          <div className="scan__idbox" />
        </div>
        <div className="scan__caption">
          Align the <strong>back</strong> of the card within the frame
        </div>
      </div>
      <div className="scan__actions">
        <button
          className="btn btn--primary btn--big"
          disabled={state !== "on" || reading}
          onClick={capture}
        >
          {reading ? "Scanning…" : "Scan"}
        </button>
        <button className="btn btn--ghost" onClick={onManual}>
          Enter manually
        </button>
      </div>
    </div>
  );
}
