import { useEffect, useRef } from "react";

export interface LogEntry {
  time: string;
  text: string;
}

/** エンジンイベントのデバッグログ。折りたたみ式(?debug=1 のときのみ表示) */
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
        <span>デバッグログ (engine)</span>
        <span className="log__chevron">{open ? "▾" : "▸"}</span>
      </button>
      {open && (
        <div className="log__body" ref={bodyRef}>
          {entries.length === 0 ? (
            <p className="log__empty">まだログはありません</p>
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
