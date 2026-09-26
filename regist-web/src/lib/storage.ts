/** 登録状態の永続化(この端末の localStorage のみ) */

export interface Registration {
  /** 券面番号(表示用。空白なし) */
  cardNumber: string;
  /** 8 バイト IDi(16 hex 文字)。ウォレット導出とオンチェーン対応付けのキー */
  idi: string;
  /** IDi から AA ウォレットを導出(発行)済みか */
  walletCreated: boolean;
  /** 顔認証の利用登録が完了しているか(顔データは一切収集しない) */
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
    // 保存不可でもセッション内は動作させる
  }
}

export function clearRegistration(): void {
  try {
    localStorage.removeItem(KEY);
  } catch {
    // noop
  }
}
