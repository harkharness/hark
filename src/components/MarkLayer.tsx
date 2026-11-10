import { useRef, useState } from "react";
import { MousePointer2, Pencil, Square, Type, X } from "lucide-react";
import { t } from "../lib/i18n";
import type { CommentAnchor } from "../types";

export type MarkTool = "select" | "pen" | "rect" | "text";
export type RegionAnchor = Extract<CommentAnchor, { kind: "region" }>;
export type Mark = { id: string; anchor: RegionAnchor; n: number };

/** The markup bar (fase 14.3b): select, pen, rectangle, text, close. */
export function MarkBar({ tool, onTool, onClose }: { tool: MarkTool; onTool: (t: MarkTool) => void; onClose: () => void }) {
  const btn = (k: MarkTool, icon: React.ReactNode, label: string, hint: string) => (
    <button className={`mark-btn ${tool === k ? "on" : ""}`} title={`${label} · ${hint}`} aria-label={label} onClick={() => onTool(k)}>
      {icon}
    </button>
  );
  return (
    <div className="mark-bar" role="toolbar">
      {btn("select", <MousePointer2 size={14} />, t("mk_select"), t("mk_select_hint"))}
      {btn("pen", <Pencil size={14} />, t("mk_pen"), t("mk_pen_hint"))}
      {btn("rect", <Square size={14} />, t("mk_rect"), t("mk_rect_hint"))}
      {btn("text", <Type size={14} />, t("mk_text"), t("mk_text_hint"))}
      <span className="mark-sep" />
      <button className="mark-btn" title={t("mk_close")} aria-label={t("mk_close")} onClick={onClose}>
        <X size={14} />
      </button>
    </div>
  );
}

/** A stroke's box: the rectangle that holds every point. */
export function boxOf(points: [number, number][]): { x: number; y: number; w: number; h: number } {
  const xs = points.map((p) => p[0]);
  const ys = points.map((p) => p[1]);
  const x = Math.min(...xs);
  const y = Math.min(...ys);
  return { x, y, w: Math.max(...xs) - x, h: Math.max(...ys) - y };
}

type Draft = { tool: "pen" | "rect"; points: [number, number][] };

/**
 * The layer over an html page's frame, the frame's own size (it sits in
 * the same scaled box, so page pixels are its pixels): the open marks
 * drawn with their number, and the one being drawn. With the select
 * tool it lets clicks through to the page; with pen, rectangle or text
 * it takes them. A finished mark is handed up as a region anchor.
 */
export default function MarkLayer({
  w,
  h,
  tool,
  marks,
  slide,
  onMark,
  onPick,
  pending,
}: {
  w: number;
  h: number;
  tool: MarkTool;
  marks: Mark[];
  /** The slide on screen: a deck shows only the marks of that slide and tags new ones with it. */
  slide?: number | null;
  onMark: (a: RegionAnchor) => void;
  /** An existing mark clicked: its comment is the anchor again. */
  onPick?: (m: Mark) => void;
  /** The mark just drawn, not yet a comment: shown dashed until it is. */
  pending?: RegionAnchor | null;
}) {
  const ref = useRef<SVGSVGElement>(null);
  const [draft, setDraft] = useState<Draft | null>(null);
  const shown = marks.filter((m) => slide == null || m.anchor.slide == null || m.anchor.slide === slide);
  const drawing = tool !== "select";

  /** Page coordinates of a pointer event: the layer may be scaled on screen. */
  const at = (e: { clientX: number; clientY: number }): [number, number] => {
    const el = ref.current;
    if (!el) return [0, 0];
    const r = el.getBoundingClientRect();
    const sx = r.width > 0 ? w / r.width : 1;
    const sy = r.height > 0 ? h / r.height : 1;
    return [Math.max(0, Math.min(w, (e.clientX - r.left) * sx)), Math.max(0, Math.min(h, (e.clientY - r.top) * sy))];
  };
  const finish = (points: [number, number][], kind: "pen" | "rect" | "text") => {
    setDraft(null);
    const base = { kind: "region" as const, slide: slide ?? null };
    if (kind === "text") {
      const [x, y] = points[0];
      onMark({ ...base, x, y, w: 0, h: 0 });
      return;
    }
    const box = boxOf(points);
    if (kind === "pen") {
      if (points.length < 2) return;
      onMark({ ...base, ...box, points: points.map(([x, y]) => [Math.round(x), Math.round(y)] as [number, number]) });
      return;
    }
    if (box.w < 3 && box.h < 3) return;
    onMark({ ...base, ...box });
  };

  return (
    <svg
      ref={ref}
      className={`mark-layer ${drawing ? "drawing" : ""} tool-${tool}`}
      width={w}
      height={h}
      viewBox={`0 0 ${w} ${h}`}
      onMouseDown={(e) => {
        if (!drawing) return;
        e.preventDefault();
        e.stopPropagation();
        const p = at(e);
        if (tool === "text") {
          finish([p], "text");
          return;
        }
        setDraft({ tool, points: [p] });
      }}
      onMouseMove={(e) => {
        if (!draft) return;
        const p = at(e);
        setDraft(draft.tool === "rect" ? { ...draft, points: [draft.points[0], p] } : { ...draft, points: [...draft.points, p] });
      }}
      onMouseUp={() => draft && finish(draft.points, draft.tool)}
      onMouseLeave={() => draft && finish(draft.points, draft.tool)}
    >
      {shown.map((m) => {
        const a = m.anchor;
        const point = a.w < 1 && a.h < 1 && !a.points;
        return (
          <g key={m.id} className="mark" onClick={(e) => { e.stopPropagation(); onPick?.(m); }}>
            {a.points && a.points.length > 1 ? (
              <polyline points={a.points.map((p) => p.join(",")).join(" ")} fill="none" />
            ) : point ? (
              <circle cx={a.x} cy={a.y} r={6} />
            ) : (
              <rect x={a.x} y={a.y} width={a.w} height={a.h} rx={3} />
            )}
            <g className="mark-badge" transform={`translate(${point ? a.x + 8 : a.x + a.w}, ${a.y})`}>
              <circle r={9} />
              <text textAnchor="middle" dy="3.5">{m.n}</text>
            </g>
          </g>
        );
      })}
      {pending && !draft && (slide == null || pending.slide == null || pending.slide === slide) && (
        pending.points && pending.points.length > 1 ? (
          <polyline className="mark-draft" points={pending.points.map((p) => p.join(",")).join(" ")} fill="none" />
        ) : pending.w < 1 && pending.h < 1 ? (
          <circle className="mark-draft" cx={pending.x} cy={pending.y} r={6} />
        ) : (
          <rect className="mark-draft" x={pending.x} y={pending.y} width={pending.w} height={pending.h} rx={3} />
        )
      )}
      {draft && (draft.tool === "rect" && draft.points.length > 1 ? (
        (() => {
          const b = boxOf(draft.points);
          return <rect className="mark-draft" x={b.x} y={b.y} width={b.w} height={b.h} rx={3} />;
        })()
      ) : (
        <polyline className="mark-draft" points={draft.points.map((p) => p.join(",")).join(" ")} fill="none" />
      ))}
    </svg>
  );
}
