import { useEffect, useRef, useState } from "react";
import type { DetectedFace } from "../sim";

interface Props {
  face: DetectedFace | null;
  /** 品質ゲートを通過しているか(ガイド枠の色に反映) */
  gateOk: boolean;
  hint: string;
  /**
   * true: ネイティブ側がカメラを所有し、window.__safrFrame で push される
   * プレビューフレームを表示する(端末上の実エンジン構成)。
   * false: ブラウザ開発時。getUserMedia の前面カメラを表示する。
   */
  nativePreview: boolean;
}

/**
 * カメラプレビュー。鏡像表示し、上に顔ガイド(楕円)と検出枠をオーバーレイする。
 * カメラが使えない環境ではシルエットのプレースホルダを出す。
 */
export function CameraView({ face, gateOk, hint, nativePreview }: Props) {
  const videoRef = useRef<HTMLVideoElement>(null);
  const imgRef = useRef<HTMLImageElement>(null);
  const [cameraState, setCameraState] = useState<"loading" | "on" | "off">("loading");

  // ネイティブ push プレビュー
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

  // ブラウザ開発時の getUserMedia プレビュー
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
          <p>カメラを利用できません(プレビューは省略)</p>
        </div>
      ) : nativePreview ? (
        // ネイティブ push フレームは Android が前面カメラに自動適用する
        // 水平ミラーで既に鏡像なので、CSS では反転しない
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

      {/* 顔ガイド楕円 */}
      <div className={`camera__guide ${gateOk ? "camera__guide--ok" : ""}`} />

      {/* 検出枠。ネイティブは表示フレームそのものを検出しているので座標を
          そのまま使う。getUserMedia パスは CSS で反転表示しているため X を反転 */}
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
