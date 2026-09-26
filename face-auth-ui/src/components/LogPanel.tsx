import { useEffect, useRef } from "react";

export interface LogEntry {
  time: string;
  text: string;
}

/** Debug log of engine events. Collapsible (shown only with ?debug=1) */
export function LogPanel({ entries, open, onToggle }: {
  entries: LogEntry[];
  open: boolean;
  onToggle: () => void;
}) {
  const bodyRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (open && bodyRef.current) {
      bodyRef.current.scrollTop = bodyRef.current.scrollHeight;
    }
  }, [entries, open]);

  return (
    <section className={`log ${open ? "log--open" : ""}`}>
      <button className="log__toggle" onClick={onToggle}>
        <span>Debug log (engine)</span>
        <span className="log__chevron">{open ? "▾" : "▸"}</span>
      </button>
      {open && (
        <div className="log__body" ref={bodyRef}>
          {entries.length === 0 ? (
            <p className="log__empty">No logs yet</p>
          ) : (
            entries.map((e, i) => (
              <p key={i} className="log__line">
                <span className="log__time">{e.time}</span> {e.text}
              </p>
            ))
          )}
        </div>
      )}
    </section>
  );
}
