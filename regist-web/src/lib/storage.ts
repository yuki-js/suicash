/** 登録状態の永続化(この端末の localStorage のみ。外部送信はしない) */

export interface Registration {
  /** 生の IDi(この端末にのみ保持) */
  idi: string;
  salt: string;
  /** hash(IDi, salt)。外部に渡してよいのはこれだけ */
  commitment: string;
  /** ダミー顔登録が完了しているか(顔データは一切収集しない) */
  faceEnrolled: boolean;
}

const KEY = "suicash.registration";

export function loadRegistration(): Registration | null {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return null;
    const r = JSON.parse(raw) as Registration;
    if (typeof r.idi !== "string" || typeof r.commitment !== "string") return null;
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
