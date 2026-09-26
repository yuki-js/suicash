import { DEFAULT_THRESHOLD, MAX_THRESHOLD } from "./sim";
import type { OpMode } from "./types";

/**
 * 母艦 CLI(PC 側)からの制御の受け口。
 *
 * 端末 UI 自身にはモード切替・閾値変更の操作を置かない。制御はすべて
 * 母艦 CLI → ブリッジ → このモジュール、の一方向で入ってくる。
 *
 * 現段階のブリッジ実装:
 *  - 起動時パラメータ: URL クエリ (?mode=enroll&threshold=0.85&debug=1)
 *  - 実行中の切替:      window.postMessage({ source: "suicash-facectl", ... })
 *
 * 実運用では WebSocket 等で受けたコマンドを同じ ControlCommand に変換して
 * postMessage する薄いブリッジを足すだけでよい。
 */

export type ControlCommand =
  | { cmd: "mode"; value: OpMode }
  | { cmd: "threshold"; value: number }
  | { cmd: "store.clear" };

export const CONTROL_SOURCE = "suicash-facectl";

export interface ControlState {
  mode: OpMode;
  threshold: number;
  debug: boolean;
}

/** 起動時の制御状態を URL クエリから読む */
export function initialControl(): ControlState {
  const q = new URLSearchParams(window.location.search);
  const debug = q.get("debug") === "1";
  // 顔登録は本来、利用者自身の端末で事前に行う(この端末は認証専用)。
  // そのため登録モードは製品導線から隠し、開発時(debug=1)のみ入れる。
  // 登録まわりの開発は face-regist ブランチで扱う。
  const mode: OpMode = debug && q.get("mode") === "enroll" ? "enroll" : "normal";
  const t = Number(q.get("threshold"));
  const threshold =
    Number.isFinite(t) && t > 0 && t <= MAX_THRESHOLD ? t : DEFAULT_THRESHOLD;
  return { mode, threshold, debug };
}

/** 実行中の制御コマンドを購読する。戻り値は解除関数 */
export function onControlMessage(
  handler: (c: ControlCommand) => void,
): () => void {
  const listener = (e: MessageEvent) => {
    const d = e.data;
    if (!d || typeof d !== "object" || d.source !== CONTROL_SOURCE) return;
    if (d.cmd === "mode" && (d.value === "enroll" || d.value === "normal")) {
      handler({ cmd: "mode", value: d.value });
    } else if (
      d.cmd === "threshold" &&
      typeof d.value === "number" &&
      d.value >= 0 &&
      d.value <= MAX_THRESHOLD
    ) {
      handler({ cmd: "threshold", value: d.value });
    } else if (d.cmd === "store.clear") {
      handler({ cmd: "store.clear" });
    }
  };
  window.addEventListener("message", listener);
  return () => window.removeEventListener("message", listener);
}
