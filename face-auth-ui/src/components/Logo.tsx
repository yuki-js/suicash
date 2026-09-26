/**
 * SuiCash ワードマーク。assets/logo/suicash-logo.svg と同じ組み方
 * (REM 605、iC のみアウトライン)を HTML インラインで再現する。
 * 外部 SVG を <img> で読むと Web フォントが遮断されるため、ここで直接組む。
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
