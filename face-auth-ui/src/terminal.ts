/**
 * Host ↔ terminal protocol for the payment terminal.
 *
 * The host PC reads FeliCa, authenticates the IDi via the oracle, and performs on-chain payment.
 * The terminal (this WebView) is UI-only and exchanges events/reports with the host.
 *
 * Bridge (same style as SAFR):
 *  - host → terminal: the WebView shell calls window.__facepayEvent(json)
 *  - terminal → host: call window.FacePayNative.report(json)
 *  - development (browser): events can be injected via postMessage({source:"suicash-facepay", ...});
 *    reports go to the console and postMessage. ?ws=<url> enables WebSocket too.
 */

export const SOURCE = "suicash-facepay";

/** Host → terminal events */
export type TerminalEvent =
  /** Immediate notice on card detection (oracle auth follows, a few seconds). For UI feedback */
  | { type: "detecting" }
  /** Card tapped and IDi authenticated by the oracle */
  | { type: "card"; idi: string; registered: boolean; balance: string }
  /** Card removed / session ended → back to idle */
  | { type: "cardRemoved" }
  /** Terminal mode: null = idle (balance inquiry only) / awaiting payment (debit amount[MIST string]) */
  | { type: "mode"; payment: { amount: string } | null }
  /** Payment result (after the host executes the transfer) */
  | {
      type: "paymentResult";
      ok: boolean;
      amount: string; // MIST
      balanceAfter: string; // MIST
      digest?: string;
      error?: string;
    };

/** Terminal → host reports */
export type TerminalReport =
  /** Face auth or enrollment succeeded. If awaiting payment, the host proceeds to transfer */
  | { type: "faceOk"; idi: string }
  /** Face auth failed */
  | { type: "faceNg"; idi: string }
  /** Face for this IDi enrolled on the terminal for the first time */
  | { type: "enrolled"; idi: string }
  /** User cancelled / left */
  | { type: "cancel"; idi?: string };

declare global {
  interface Window {
    /** Entry point where the shell injects host events */
    __facepayEvent?: (json: string) => void;
    /** Report channel to the host exposed by the shell */
    FacePayNative?: { report(json: string): void };
    /** Top RGB LED control exposed by the shell */
    LedNative?: { set(mode: string): void };
  }
}

/** Set the top LED (no-op in development without the shell) */
export type LedMode = "off" | "blue_blink" | "green" | "red" | "blue";
export function setLed(mode: LedMode): void {
  try {
    window.LedNative?.set(mode);
  } catch {
    /* ignore */
  }
}

/** WS connection (reports are also sent through it if present) */
let sharedWs: WebSocket | null = null;

/** Subscribe to host events. Returns an unsubscribe function */
export function subscribe(handler: (e: TerminalEvent) => void): () => void {
  const onObj = (d: unknown) => {
    if (d && typeof d === "object" && "type" in (d as object)) {
      handler(d as TerminalEvent);
    }
  };

  // 1) Direct injection from the shell
  window.__facepayEvent = (json: string) => {
    try {
      onObj(JSON.parse(json));
    } catch {
      /* ignore */
    }
  };

  // 2) postMessage injection during development
  const onMsg = (ev: MessageEvent) => {
    const d = ev.data;
    if (d && typeof d === "object" && d.source === SOURCE && d.event) {
      onObj(d.event);
    }
  };
  window.addEventListener("message", onMsg);

  // 3) Optional: WebSocket (?ws=<url>)
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

/** Send a report to the host */
export function report(msg: TerminalReport): void {
  const json = JSON.stringify(msg);
  // 1) Via WebSocket if connected (setup where the host is the WS server)
  try {
    if (sharedWs && sharedWs.readyState === WebSocket.OPEN) {
      sharedWs.send(json);
      return;
    }
  } catch {
    /* fallthrough */
  }
  // 2) The shell's native bridge
  try {
    if (window.FacePayNative) {
      window.FacePayNative.report(json);
      return;
    }
  } catch {
    /* fallthrough */
  }
  // Development: just log it
  // eslint-disable-next-line no-console
  console.log("[facepay report]", json);
  try {
    window.postMessage({ source: SOURCE, report: msg }, "*");
  } catch {
    /* ignore */
  }
}

/** MIST string → SUI display (2 decimals) */
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
