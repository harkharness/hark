/**
 * What a gallery card draws for a page, read from the file it already
 * has (fase "miniaturas"): the page at scale, never a generic picture.
 * Nothing is computed here: formulas stay text, a dashboard shows no
 * numbers, a board shows shapes and colours without words.
 */

const json = (text: string): Record<string, unknown> | null => {
  try {
    const v = JSON.parse(text) as unknown;
    return v && typeof v === "object" && !Array.isArray(v) ? (v as Record<string, unknown>) : null;
  } catch {
    return null;
  }
};

export type SheetThumbCell = { text: string; num: boolean; formula: boolean };
export type SheetThumb = { columns: string[]; rows: SheetThumbCell[][]; tabs: string[] };

/** The first tab's first 4 columns × 4 rows; null when the file is not a sheet. */
export function sheetPreview(text: string, cols = 4, rows = 4): SheetThumb | null {
  const v = json(text);
  const tabs = Array.isArray(v?.tabs) ? (v!.tabs as Record<string, unknown>[]) : null;
  if (!tabs || tabs.length === 0) return null;
  const first = tabs[0];
  const columns = (Array.isArray(first.columns) ? (first.columns as unknown[]) : []).slice(0, cols).map(String);
  const data = (Array.isArray(first.rows) ? (first.rows as unknown[][]) : []).slice(0, rows).map((r) =>
    Array.from({ length: columns.length }, (_, i) => {
      const c = r?.[i];
      if (c == null) return { text: "", num: false, formula: false };
      if (typeof c === "number") return { text: String(c), num: true, formula: false };
      const s = String(c);
      return { text: s, num: false, formula: s.startsWith("=") };
    }),
  );
  return { columns, rows: data, tabs: tabs.map((t) => String(t.name ?? "")) };
}

export type BoardThumb = {
  notes: { id: string; x: number; y: number; w: number; h: number; color: string }[];
  groups: { title: string; x: number; y: number; w: number; h: number }[];
  links: { x1: number; y1: number; x2: number; y2: number }[];
  milestones: number;
};

const NOTE_W = 150;
const NOTE_H = 96;

/** The board's shapes scaled into a box of `w`×`h`; notes without a
 *  place fall into a simple grid after the placed ones. */
export function boardPreview(text: string, w: number, h: number): BoardThumb | null {
  const v = json(text);
  if (!v) return null;
  const notes = (Array.isArray(v.notes) ? (v.notes as Record<string, unknown>[]) : []).filter((n) => typeof n.id === "string");
  const groups = Array.isArray(v.groups) ? (v.groups as Record<string, unknown>[]) : [];
  const num = (x: unknown) => (typeof x === "number" && Number.isFinite(x) ? x : null);
  // Place the unplaced: a grid to the right of what has coordinates.
  let maxX = 0;
  for (const n of notes) if (num(n.x) != null) maxX = Math.max(maxX, num(n.x)! + NOTE_W);
  for (const g of groups) if (num(g.x) != null) maxX = Math.max(maxX, num(g.x)! + (num(g.w) ?? 0));
  let k = 0;
  const placed = notes.map((n) => {
    let x = num(n.x);
    let y = num(n.y);
    if (x == null || y == null) {
      x = maxX + 20 + (k % 2) * (NOTE_W + 14);
      y = 20 + Math.floor(k / 2) * (NOTE_H + 14);
      k += 1;
    }
    return { id: n.id as string, x, y, color: typeof n.color === "string" ? n.color : "yellow" };
  });
  const gs = groups.map((g) => ({ title: String(g.title ?? ""), x: num(g.x) ?? 0, y: num(g.y) ?? 0, w: num(g.w) ?? 0, h: num(g.h) ?? 0 }));
  const extentX = Math.max(1, ...placed.map((n) => n.x + NOTE_W), ...gs.map((g) => g.x + g.w));
  const extentY = Math.max(1, ...placed.map((n) => n.y + NOTE_H), ...gs.map((g) => g.y + g.h));
  const pad = 6;
  const s = Math.min((w - pad * 2) / extentX, (h - pad * 2) / extentY);
  const sx = (x: number) => pad + x * s;
  const sy = (y: number) => pad + y * s;
  const out = placed.map((n) => ({ id: n.id, x: sx(n.x), y: sy(n.y), w: NOTE_W * s, h: NOTE_H * s, color: n.color }));
  const at = (id: string) => out.find((n) => n.id === id);
  const links = (Array.isArray(v.links) ? (v.links as Record<string, unknown>[]) : [])
    .map((l) => [at(String(l.from)), at(String(l.to))] as const)
    .filter((p): p is readonly [BoardThumb["notes"][number], BoardThumb["notes"][number]] => !!p[0] && !!p[1])
    .map(([a, b]) => ({ x1: a.x + a.w / 2, y1: a.y + a.h / 2, x2: b.x + b.w / 2, y2: b.y + b.h / 2 }));
  return {
    notes: out,
    groups: gs.map((g) => ({ title: g.title, x: sx(g.x), y: sy(g.y), w: g.w * s, h: g.h * s })),
    links,
    milestones: Array.isArray(v.timeline) ? (v.timeline as unknown[]).length : 0,
  };
}

export type DashThumbTile = { title: string; chart: "number" | "bar" | "line" | "table" };

/** One tile per panel, in the file's order; [] for a dashboard without
 *  panels, null when the file is not one. */
export function dashPreview(text: string): DashThumbTile[] | null {
  const v = json(text);
  if (!v) return null;
  const panels = Array.isArray(v.panels) ? (v.panels as Record<string, unknown>[]) : [];
  return panels.slice(0, 6).map((p) => {
    const c = String(p.chart ?? "table");
    const chart = c === "number" || c === "bar" || c === "line" ? c : "table";
    return { title: String(p.title ?? ""), chart };
  });
}

export type DocThumb = { title: string; lines: string[] };

/** The document's title (its first heading, else the page's) and its
 *  opening lines as plain text: marks, links, code fences and the
 *  frontmatter gone. */
export function docPreview(text: string, fallbackTitle: string, max = 6): DocThumb {
  const body = text.replace(/^---[\s\S]*?---\s*/, "").replace(/```[\s\S]*?```/g, "");
  let title = fallbackTitle;
  const lines: string[] = [];
  for (const raw of body.split("\n")) {
    const line = raw.trim();
    if (!line) continue;
    const clean = line
      .replace(/^#{1,6}\s+/, "")
      .replace(/^[-*+]\s+(\[[ x]\]\s+)?/, "")
      .replace(/^\d+\.\s+/, "")
      .replace(/^>\s?/, "")
      .replace(/!\[[^\]]*\]\([^)]*\)/g, "")
      .replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
      .replace(/[*_`~]+/g, "")
      .trim();
    if (!clean) continue;
    if (/^#\s+/.test(line) && title === fallbackTitle) {
      title = clean;
      continue;
    }
    lines.push(clean);
    if (lines.length >= max) break;
  }
  return { title, lines };
}
