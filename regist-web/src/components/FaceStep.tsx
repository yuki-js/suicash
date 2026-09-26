import { useEffect, useRef, useState } from "react";

interface Props {
  onDone: () => void;
}

type Phase = "intro" | "camera" | "scanning" | "fallback";

/** Whether the camera (getUserMedia) is available */
function cameraAvailable(): boolean {
  return !!(window.isSecureContext && navigator.mediaDevices?.getUserMedia);
}

/**
 * Face authentication enrollment step.
 *
 * Actually starts the front camera to show a live preview and a capture effect. Per our
 * privacy policy, however, captured frames are neither stored nor sent (discarded on the spot).
 * Real face matching happens only inside the auth terminal (Hi-CARA) at payment time.
 * Falls back to dummy progress where the camera is unavailable.
 */
export function FaceStep({ onDone }: Props) {
  const [phase, setPhase] = useState<Phase>("intro");
  const [scanPct, setScanPct] = useState(0);
  const videoRef = useRef<HTMLVideoElement>(null);
  const streamRef = useRef<MediaStream | null>(null);

  // Start/stop camera
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

  // Capture effect (scan). When done, discard the face frames and move on
  useEffect(() => {
    if (phase !== "scanning") return;
    if (scanPct >= 100) {
      // Frames are never stored or sent (discarded on the spot). Stop the camera and finish.
      streamRef.current?.getTracks().forEach((t) => t.stop());
      streamRef.current = null;
      const t = window.setTimeout(onDone, 500);
      return () => window.clearTimeout(t);
    }
    const t = window.setTimeout(() => setScanPct((p) => p + 5), 70);
    return () => window.clearTimeout(t);
  }, [phase, scanPct, onDone]);

  // Dummy progress (no camera)
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
      <h2 className="card__title">3. Enroll in Face ID</h2>

      {phase === "intro" && (
        <>
          <div className="facePreview">
            <svg viewBox="0 0 96 96" aria-hidden="true">
              <ellipse cx="48" cy="40" rx="16" ry="19" className="facePreview__head" />
              <path d="M24 88 C24 68 35 60 48 60 C61 60 72 68 72 88 Z" className="facePreview__head" />
            </svg>
          </div>
          <p className="card__desc">
            Let's set up face authentication. Face matching at payment happens
            <strong>only inside the store's authentication terminal</strong>.
          </p>
          <button
            className="btn btn--primary btn--big"
            onClick={() => setPhase(cameraAvailable() ? "camera" : "fallback")}
          >
            Start face enrollment
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
              {phase === "scanning" ? `Enrolling… ${scanPct}%` : "Fit your face in the frame"}
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
              Enroll this face
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
                The camera is unavailable, so we'll do a simplified enrollment.
              </p>
              <button className="btn btn--primary btn--big" onClick={() => setDummyPct(0)}>
                Start enrollment
              </button>
            </>
          ) : (
            <p className="card__desc">
              {dummyPct < 100 ? "Enrolling…" : "Enrollment complete"}
            </p>
          )}
        </>
      )}

      <p className="card__note">
        [Privacy] Captured face images and face data are never stored or sent (discarded on the spot).
        Real face matching happens only inside the auth terminal at payment time, and even there
        face data never leaves the terminal.
      </p>
    </section>
  );
}
