interface Props {
  onDone: () => void;
}

/**
 * Guidance for unregistered cards. The user completes IDi registration, wallet
 * issuance and face registration on their own phone (regist-web), then taps again.
 */
export function RegisterPrompt({ onDone }: Props) {
  return (
    <div className="registerPrompt" onClick={onDone}>
      <div className="registerPrompt__icon" aria-hidden="true">!</div>
      <h2 className="registerPrompt__title">This card is not registered</h2>
      <p className="registerPrompt__sub">
        Please register on your smartphone,
        then tap your card again.
      </p>
      <div className="registerPrompt__url">SuiCash registration site</div>
      <p className="registerPrompt__hint">Tap the screen to go back</p>
    </div>
  );
}
