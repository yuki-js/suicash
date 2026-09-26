/** Registration state persistence (this device's localStorage only) */

export interface Registration {
  /** Card number (for display; no spaces) */
  cardNumber: string;
  /** 8-byte IDi (16 hex chars). Key for wallet derivation and on-chain mapping */
  idi: string;
  /** Whether the AA wallet has been derived (issued) from the IDi */
  walletCreated: boolean;
  /** Whether face authentication enrollment is complete (no face data is ever collected) */
  faceEnrolled: boolean;
}

const KEY = "suicash.registration";

export function loadRegistration(): Registration | null {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return null;
    const r = JSON.parse(raw) as Registration;
    if (typeof r.idi !== "string" || typeof r.cardNumber !== "string") return null;
    return r;
  } catch {
    return null;
  }
}

export function saveRegistration(r: Registration): void {
  try {
    localStorage.setItem(KEY, JSON.stringify(r));
  } catch {
    // Keep working within the session even if saving fails
  }
}

export function clearRegistration(): void {
  try {
    localStorage.removeItem(KEY);
  } catch {
    // noop
  }
}
