/**
 * SuiCash ワードマーク。assets/logo/suicash-logo.svg と同じ組み方
 * (REM 605、iC のみアウトライン)を HTML インラインで再現する。
 */
export function Logo({ inverted = false }: { inverted?: boolean }) {
  const cls = inverted ? "logo logo--inverted" : "logo";
  return (
    <span className={cls} aria-label="SuiCash">
      <span className="logo__solid">Su</span>
      <span className="logo__outline">iC</span>
      <span className="logo__solid">ash</span>
    </span>
  );
}
