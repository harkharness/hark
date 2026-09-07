/**
 * The two shapes every cost surface reads through: a stacked bar (what a
 * total is made of) and a legend row (name, share, absolute value). Numbers
 * alone don't answer "where did it go?" — proportion does.
 */

export type Segment = {
  label: string;
  value: number;
  /** CSS color; use the palette tokens in styles.css. */
  color: string;
  /** Right-aligned text for the legend (tokens, USD, turns…). */
  detail?: string;
};

export const fmtTok = (n: number) =>
  n >= 1e6 ? `${(n / 1e6).toFixed(1)}M` : n >= 1e3 ? `${(n / 1e3).toFixed(1)}k` : `${n}`;

export const fmtUsd = (n: number) => (n >= 0.01 ? `$${n.toFixed(2)}` : `$${n.toFixed(4)}`);

const pct = (part: number, whole: number) => (whole > 0 ? (part / whole) * 100 : 0);

/** One stacked bar. `capacity` renders unused space (a context window). */
export function StackedBar({
  segments,
  capacity,
  height = 8,
}: {
  segments: Segment[];
  capacity?: number;
  height?: number;
}) {
  const used = segments.reduce((a, s) => a + Math.max(0, s.value), 0);
  const whole = capacity && capacity > used ? capacity : used;
  return (
    <div className="meter-track" style={{ height }}>
      {segments
        .filter((s) => s.value > 0)
        .map((s) => (
          <div
            key={s.label}
            className="meter-fill"
            style={{ width: `${pct(s.value, whole)}%`, background: s.color }}
            title={`${s.label}: ${fmtTok(s.value)}`}
          />
        ))}
    </div>
  );
}

/** Legend under a stacked bar: dot, name, absolute, share of the whole. */
export function Legend({
  segments,
  capacity,
  free,
}: {
  segments: Segment[];
  capacity?: number;
  /** Label for the unused remainder (omit to hide it). */
  free?: string;
}) {
  const used = segments.reduce((a, s) => a + Math.max(0, s.value), 0);
  const whole = capacity && capacity > used ? capacity : used;
  const rest = whole - used;
  return (
    <ul className="meter-legend">
      {segments
        .filter((s) => s.value > 0)
        .map((s) => (
          <li key={s.label}>
            <i className="dot" style={{ background: s.color }} />
            <span className="name">{s.label}</span>
            <span className="val">{s.detail ?? fmtTok(s.value)}</span>
            <span className="share">{pct(s.value, whole).toFixed(1)}%</span>
          </li>
        ))}
      {free && rest > 0 && (
        <li className="muted">
          <i className="dot free" />
          <span className="name">{free}</span>
          <span className="val">{fmtTok(rest)}</span>
          <span className="share">{pct(rest, whole).toFixed(1)}%</span>
        </li>
      )}
    </ul>
  );
}

/** A single measured row: name, value, and a bar relative to the biggest. */
export function MeterRow({
  name,
  value,
  max,
  detail,
  color = "var(--accent)",
  note,
  title,
}: {
  name: string;
  value: number;
  max: number;
  /** Main number, already formatted. */
  detail: string;
  color?: string;
  /** Secondary text under the name. */
  note?: string;
  title?: string;
}) {
  return (
    <div className="meter-row" title={title ?? name}>
      <div className="meter-row-head">
        <span className="name">{name}</span>
        <b className="val">{detail}</b>
      </div>
      <div className="meter-track" style={{ height: 6 }}>
        <div
          className="meter-fill"
          style={{ width: `${Math.max(pct(value, max), value > 0 ? 2 : 0)}%`, background: color }}
        />
      </div>
      {note && <div className="meter-note">{note}</div>}
    </div>
  );
}

/** A percentage gauge with a caption on the right (usage windows). */
export function Gauge({
  name,
  used,
  caption,
}: {
  name: string;
  /** 0..1 */
  used: number;
  caption?: string;
}) {
  const tone = used >= 0.9 ? "var(--err)" : used >= 0.7 ? "var(--warn)" : "var(--accent)";
  return (
    <div className="meter-row">
      <div className="meter-row-head">
        <span className="name">{name}</span>
        <span className="caption">{caption}</span>
        <b className="val" style={{ color: tone }}>
          {Math.round(used * 100)}%
        </b>
      </div>
      <div className="meter-track" style={{ height: 6 }}>
        <div className="meter-fill" style={{ width: `${used * 100}%`, background: tone }} />
      </div>
    </div>
  );
}

/** The context window as a 14px ring — the smallest honest form of a
 *  percentage, for a title bar where a labelled bar would shout. */
export function ContextRing({ used }: { used: number }) {
  const tone = used >= 0.9 ? "var(--err)" : used >= 0.7 ? "var(--warn)" : "var(--accent)";
  const r = 6;
  const circumference = 2 * Math.PI * r;
  return (
    <span className="ctx-ring" title={`janela de contexto · ${Math.round(used * 100)}%`}>
      <svg width="14" height="14" viewBox="0 0 16 16">
        <circle cx="8" cy="8" r={r} fill="none" stroke="var(--panel-2)" strokeWidth="2.6" />
        <circle
          cx="8"
          cy="8"
          r={r}
          fill="none"
          stroke={tone}
          strokeWidth="2.6"
          strokeLinecap="round"
          strokeDasharray={`${used * circumference} ${circumference}`}
          transform="rotate(-90 8 8)"
        />
      </svg>
      {Math.round(used * 100)}%
    </span>
  );
}
