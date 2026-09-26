/**
 * SuiCash wordmark. Reproduces the setting of assets/logo/suicash-logo.svg
 * (REM 605, only iC outlined) inline in HTML.
 * Loading external SVG via <img> blocks web fonts, so it is built directly here.
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
