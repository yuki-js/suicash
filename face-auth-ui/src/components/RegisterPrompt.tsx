interface Props {
  onDone: () => void;
}

/**
 * 未登録カード向けの案内。利用者は自分のスマホ(regist-web)で
 * IDi 登録・ウォレット発行・顔登録を済ませてから、再度タッチする。
 */
export function RegisterPrompt({ onDone }: Props) {
  return (
    <div className="registerPrompt" onClick={onDone}>
      <div className="registerPrompt__icon" aria-hidden="true">!</div>
      <h2 className="registerPrompt__title">未登録のカードです</h2>
      <p className="registerPrompt__sub">
        お手持ちのスマートフォンで登録を済ませてから、
        もう一度カードをタッチしてください。
      </p>
      <div className="registerPrompt__url">SuiCash 登録サイト</div>
      <p className="registerPrompt__hint">画面をタッチで戻る</p>
    </div>
  );
}
