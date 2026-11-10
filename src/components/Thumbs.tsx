import { boardPreview, dashPreview, docPreview, sheetPreview } from "../lib/thumbs";

const NOTE_BG: Record<string, string> = { yellow: "#f1d77a", blue: "#9fd3f6", green: "#b9e6a5", pink: "#f4b8c6", gray: "#c9ced6" };

/** A sheet's first cells as a tiny grid; formulas stay text. */
export function ThumbSheet({ text }: { text: string }) {
  const p = sheetPreview(text);
  if (!p) return <div className="thumb-empty dim">{"{ }"}</div>;
  return (
    <div className="thumb-box thumb-sheet">
      <div className="thumb-sheet-row head" style={{ gridTemplateColumns: `1.4fr repeat(${Math.max(0, p.columns.length - 1)}, 1fr)` }}>
        {p.columns.map((c, i) => (
          <span key={i}>{c}</span>
        ))}
      </div>
      {p.rows.map((r, i) => (
        <div key={i} className="thumb-sheet-row" style={{ gridTemplateColumns: `1.4fr repeat(${Math.max(0, p.columns.length - 1)}, 1fr)` }}>
          {r.map((c, j) => (
            <span key={j} className={`${c.num ? "num" : ""} ${c.formula ? "f" : ""}`}>{c.text}</span>
          ))}
        </div>
      ))}
      {p.tabs.length > 1 && <div className="thumb-sheet-tabs dim">{p.tabs.map((t, i) => <span key={i} className={i === 0 ? "on" : ""}>{t}</span>)}</div>}
    </div>
  );
}

/** A board's notes and groups as shapes at their places; no words. */
export function ThumbBoard({ text, w = 240, h = 80 }: { text: string; w?: number; h?: number }) {
  const p = boardPreview(text, w, h);
  if (!p) return <div className="thumb-empty dim">{"{ }"}</div>;
  return (
    <div className="thumb-box thumb-board" style={{ width: w, height: h }}>
      {p.groups.map((g, i) => (
        <div key={i} className="thumb-group" style={{ left: g.x, top: g.y, width: g.w, height: g.h }}>
          {g.title && <span>{g.title}</span>}
        </div>
      ))}
      <svg width={w} height={h} className="thumb-links">
        {p.links.map((l, i) => (
          <line key={i} x1={l.x1} y1={l.y1} x2={l.x2} y2={l.y2} />
        ))}
      </svg>
      {p.notes.map((n) => (
        <div key={n.id} className="thumb-note" style={{ left: n.x, top: n.y, width: n.w, height: n.h, background: NOTE_BG[n.color] ?? NOTE_BG.yellow }} />
      ))}
      {p.milestones > 0 && (
        <div className="thumb-timeline">
          {Array.from({ length: Math.min(p.milestones, 8) }, (_, i) => (
            <span key={i} style={{ left: `${p.milestones > 1 ? (i / (Math.min(p.milestones, 8) - 1)) * 100 : 0}%` }} />
          ))}
        </div>
      )}
    </div>
  );
}

/** A dashboard as one tile per panel, by its chart; no numbers. */
export function ThumbDash({ text }: { text: string }) {
  const tiles = dashPreview(text);
  if (!tiles) return <div className="thumb-empty dim">{"{ }"}</div>;
  if (tiles.length === 0) return <div className="thumb-box thumb-dash" />;
  return (
    <div className="thumb-box thumb-dash">
      {tiles.map((t, i) => (
        <div key={i} className={`thumb-tile ${t.chart}`} title={t.title}>
          <span className="thumb-tile-title">{t.title}</span>
          {t.chart === "number" && <span className="thumb-tile-num">—</span>}
          {t.chart === "bar" && (
            <span className="thumb-tile-bars">
              {[30, 55, 40, 70, 60, 85].map((v, j) => (
                <i key={j} style={{ height: `${v}%` }} className={j === 5 ? "last" : ""} />
              ))}
            </span>
          )}
          {t.chart === "line" && (
            <svg viewBox="0 0 60 30" preserveAspectRatio="none" className="thumb-tile-line">
              <path d="M0,24 L12,20 L24,22 L36,12 L48,14 L60,6 L60,30 L0,30 Z" className="fill" />
              <path d="M0,24 L12,20 L24,22 L36,12 L48,14 L60,6" className="stroke" fill="none" />
            </svg>
          )}
          {t.chart === "table" && (
            <span className="thumb-tile-table">
              <i /><i /><i />
            </span>
          )}
        </div>
      ))}
    </div>
  );
}

/** A document's title and opening lines, as the reading column shows them. */
export function ThumbDoc({ text, title }: { text: string; title: string }) {
  const p = docPreview(text, title);
  return (
    <div className="thumb-box thumb-doc">
      <b>{p.title}</b>
      <p>{p.lines.join(" ")}</p>
    </div>
  );
}
