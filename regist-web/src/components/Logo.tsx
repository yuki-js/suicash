/**
 * SuiCash wordmark. Reproduces the same setting as assets/logo/suicash-logo.svg
 * (REM 605, only "iC" outlined) inline in HTML.
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
