import { useEffect, useRef, useState } from "react";
import type { DetectedFace } from "../sim";

interface Props {
  face: DetectedFace | null;
  /** Whether the quality gate is passing (reflected in the guide color) */
  gateOk: boolean;
  hint: string;
  /**
   * true: the native side owns the camera; show preview frames pushed via
   * window.__safrFrame (real-engine setup on the device).
   * false: browser development; show the front camera via getUserMedia.
   */
  nativePreview: boolean;
}

/**
 * Camera preview. Mirrored, with the face guide (ellipse) and detection box overlaid.
 * Shows a silhouette placeholder where no camera is available.
 */
export function CameraView({ face, gateOk, hint, nativePreview }: Props) {
  const videoRef = useRef<HTMLVideoElement>(null);
  const imgRef = useRef<HTMLImageElement>(null);
  const [cameraState, setCameraState] = useState<"loading" | "on" | "off">("loading");

  // Native pushed preview
  useEffect(() => {
    if (!nativePreview) return;
    window.__safrFrame = (dataUrl: string) => {
      if (imgRef.current) {
        imgRef.current.src = dataUrl;
      }
      setCameraState("on");
    };
    return () => {
      delete window.__safrFrame;
    };
  }, [nativePreview]);

  // getUserMedia preview for browser development
  useEffect(() => {
    if (nativePreview) return;
    let stream: MediaStream | null = null;
    let cancelled = false;
    (async () => {
      try {
        stream = await navigator.mediaDevices.getUserMedia({
          video: { facingMode: "user", width: { ideal: 720 }, height: { ideal: 960 } },
          audio: false,
        });
        if (cancelled) {
          stream.getTracks().forEach((t) => t.stop());
          return;
        }
        if (videoRef.current) {
          const v = videoRef.current;
          v.srcObject = stream;
          v.muted = true;
          const tryPlay = () => v.play().catch(() => {});
          tryPlay();
          v.addEventListener("loadedmetadata", tryPlay);
          setCameraState("on");
        }
      } catch {
        if (!cancelled) setCameraState("off");
      }
    })();
    return () => {
      cancelled = true;
      stream?.getTracks().forEach((t) => t.stop());
    };
  }, [nativePreview]);

  const b = face?.bounds;

  return (
    <div className="camera">
      {cameraState === "off" ? (
        <div className="camera__placeholder">
          <svg viewBox="0 0 100 125" className="camera__silhouette" aria-hidden="true">
            <ellipse cx="50" cy="46" rx="24" ry="30" />
            <path d="M14 125 C14 95 30 84 50 84 C70 84 86 95 86 125 Z" />
          </svg>
          <p>Camera unavailable (preview skipped)</p>
        </div>
      ) : nativePreview ? (
        // Native pushed frames are already mirrored by Android's automatic
        // front-camera horizontal flip, so don't flip them in CSS
        <img ref={imgRef} className="camera__video" alt="" />
      ) : (
        <video
          ref={videoRef}
          className="camera__video camera__video--mirror"
          autoPlay
          playsInline
          muted
        />
      )}

      {/* Face guide ellipse */}
      <div className={`camera__guide ${gateOk ? "camera__guide--ok" : ""}`} />

      {/* Detection box. Native detects on the displayed frame itself, so use the
          coordinates as is. The getUserMedia path is flipped in CSS, so flip X */}
      {b && (
        <div
          className={`camera__box ${gateOk ? "camera__box--ok" : ""}`}
          style={{
            left: `${(nativePreview ? b.x : 1 - b.x - b.w) * 100}%`,
            top: `${b.y * 100}%`,
            width: `${b.w * 100}%`,
            height: `${b.h * 100}%`,
          }}
        >
          {face.extraRotation !== 0 && (
            <span className="camera__rotBadge">+{face.extraRotation}°</span>
          )}
        </div>
      )}

      <div className="camera__hint">{hint}</div>
    </div>
  );
}
