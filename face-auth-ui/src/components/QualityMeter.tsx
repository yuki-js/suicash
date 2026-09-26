interface Props {
  label: string;
  value: number | null;
  /** Pass threshold (0..1). With invert=true, values below it pass */
  gate: number;
  invert?: boolean;
}

/** Shows one cpq / contrast / sharpness / mask quality gate as a single bar */
export function QualityMeter({ label, value, gate, invert = false }: Props) {
  const has = value !== null;
  const v = has ? Math.min(1, Math.max(0, value)) : 0;
  const pass = has && (invert ? value < gate : value >= gate);
  return (
    <div className={`meter ${has ? (pass ? "meter--pass" : "meter--fail") : ""}`}>
      <div className="meter__head">
        <span className="meter__label">{label}</span>
        <span className="meter__value">{has ? value.toFixed(2) : "--"}</span>
      </div>
      <div className="meter__track">
        <div className="meter__fill" style={{ width: `${v * 100}%` }} />
        <div className="meter__gate" style={{ left: `${gate * 100}%` }} />
      </div>
    </div>
  );
}
