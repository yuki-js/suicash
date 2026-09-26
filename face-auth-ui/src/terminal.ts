/**
 * 決済端末の母艦↔端末プロトコル。
 *
 * FeliCa は母艦 PC が読み、オラクルで IDi を認証し、オンチェーン決済も母艦が行う。
 * 端末(この WebView)は UI 専任で、母艦とはイベント/レポートをやり取りする。
 *
 * ブリッジ(SAFR と同じ流儀):
 *  - 母艦 → 端末: WebView シェルが window.__facepayEvent(json) を呼ぶ
 *  - 端末 → 母艦: window.FacePayNative.report(json) を呼ぶ
 *  - 開発時(ブラウザ): postMessage({source:"suicash-facepay", ...}) で注入でき、
 *    レポートは console と postMessage に出す。?ws=<url> で WebSocket も使える。
 */

export const SOURCE = "suicash-facepay";

/** 母艦 → 端末 のイベント */
export type TerminalEvent =
  /** カードがタッチされ、オラクルで IDi 認証済み */
  | { type: "card"; idi: string; registered: boolean; balance: string }
  /** カードが離された/セッション終了 → 待機へ */
  | { type: "cardRemoved" }
  /** 端末モード: null=待機(残高照会のみ) / 決済待機(引き落とし額 amount[MIST 文字列]) */
  | { type: "mode"; payment: { amount: string } | null }
  /** 決済結果(母艦が送金を実行したあと) */
  | {
      type: "paymentResult";
      ok: boolean;
      amount: string; // MIST
      balanceAfter: string; // MIST
      digest?: string;
      error?: string;
    };

/** 端末 → 母艦 のレポート */
export type TerminalReport =
  /** 顔認証 or 顔登録が成功。母艦は決済待機なら送金へ進む */
  | { type: "faceOk"; idi: string }
  /** 顔認証に失敗 */
  | { type: "faceNg"; idi: string }
  /** その IDi の顔を端末にはじめて登録した */
  | { type: "enrolled"; idi: string }
  /** 利用者がキャンセル/離脱 */
  | { type: "cancel"; idi?: string };

declare global {
  interface Window {
    /** シェルが母艦イベントを流し込む入口 */
    __facepayEvent?: (json: string) => void;
    /** シェルが公開する母艦へのレポート口 */
    FacePayNative?: { report(json: string): void };
  }
}

/** WS 接続(あれば report もここから送る) */
let sharedWs: WebSocket | null = null;

/** 母艦イベントを購読する。戻り値は解除関数 */
export function subscribe(handler: (e: TerminalEvent) => void): () => void {
  const onObj = (d: unknown) => {
    if (d && typeof d === "object" && "type" in (d as object)) {
      handler(d as TerminalEvent);
    }
  };

  // 1) シェルからの直接注入
  window.__facepayEvent = (json: string) => {
    try {
      onObj(JSON.parse(json));
    } catch {
      /* ignore */
    }
  };

  // 2) 開発時の postMessage 注入
  const onMsg = (ev: MessageEvent) => {
    const d = ev.data;
    if (d && typeof d === "object" && d.source === SOURCE && d.event) {
      onObj(d.event);
    }
  };
  window.addEventListener("message", onMsg);

  // 3) 任意: WebSocket(?ws=<url>)
  let ws: WebSocket | null = null;
  try {
    const url = new URLSearchParams(window.location.search).get("ws");
    if (url) {
      ws = new WebSocket(url);
      sharedWs = ws;
      ws.onmessage = (m) => {
        try {
          onObj(JSON.parse(m.data));
        } catch {
          /* ignore */
        }
      };
      ws.onclose = () => {
        if (sharedWs === ws) sharedWs = null;
      };
    }
  } catch {
    /* ignore */
  }

  return () => {
    delete window.__facepayEvent;
    window.removeEventListener("message", onMsg);
    ws?.close();
  };
}

/** 母艦へレポートを送る */
export function report(msg: TerminalReport): void {
  const json = JSON.stringify(msg);
  // 1) WebSocket 接続があればそこへ(母艦が WS サーバの構成)
  try {
    if (sharedWs && sharedWs.readyState === WebSocket.OPEN) {
      sharedWs.send(json);
      return;
    }
  } catch {
    /* fallthrough */
  }
  // 2) シェルのネイティブブリッジ
  try {
    if (window.FacePayNative) {
      window.FacePayNative.report(json);
      return;
    }
  } catch {
    /* fallthrough */
  }
  // 開発時: 記録だけ残す
  // eslint-disable-next-line no-console
  console.log("[facepay report]", json);
  try {
    window.postMessage({ source: SOURCE, report: msg }, "*");
  } catch {
    /* ignore */
  }
}

/** MIST 文字列 → SUI 表示(小数2桁) */
export function mistToSui(mist: string): string {
  try {
    const m = BigInt(mist);
    const whole = m / 1_000_000_000n;
    const frac = (m % 1_000_000_000n).toString().padStart(9, "0").slice(0, 2);
    return `${whole}.${frac}`;
  } catch {
    return "--";
  }
}
