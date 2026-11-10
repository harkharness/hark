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

/** The board's shapes scaled into a box of `w`×`h`. Notes and groups
 *  without a place are laid out as the viewer lays them (mirror of
 *  whiteboard::lay_out): groups side by side, two notes per row, loose
 *  notes in a lane under them. */
export function boardPreview(text: string, w: number, h: number): BoardThumb | null {
  const v = json(text);
  if (!v) return null;
  const GAP = 14;
  const PAD = 24;
  const PER_ROW = 2;
  const notes = (Array.isArray(v.notes) ? (v.notes as Record<string, unknown>[]) : []).filter((n) => typeof n.id === "string");
  const groups = (Array.isArray(v.groups) ? (v.groups as Record<string, unknown>[]) : []).filter((g) => typeof g.id === "string");
  const num = (x: unknown) => (typeof x === "number" && Number.isFinite(x) ? x : null);
  const pos = new Map<string, { x: number; y: number; color: string }>();
  let cursorX = PAD;
  let laneY = PAD;
  const groupW = PAD * 2 + NOTE_W * PER_ROW + GAP * (PER_ROW - 1);
  const gs = groups.map((g) => {
    const members = notes.filter((n) => n.group === g.id);
    const rows = Math.max(1, Math.ceil(members.length / PER_ROW));
    const gx = num(g.x) ?? cursorX;
    const gy = num(g.y) ?? PAD;
    const gw = num(g.w) ?? groupW;
    const gh = num(g.h) ?? PAD * 2 + NOTE_H * rows + GAP * (rows - 1) + 12;
    members.forEach((n, k) => {
      const x = num(n.x);
      const y = num(n.y);
      pos.set(n.id as string, {
        x: x ?? gx + PAD + (k % PER_ROW) * (NOTE_W + GAP),
        y: y ?? gy + PAD + 12 + Math.floor(k / PER_ROW) * (NOTE_H + GAP),
        color: typeof n.color === "string" ? n.color : "yellow",
      });
    });
    cursorX = Math.max(gx, cursorX) + gw + GAP * 2;
    laneY = Math.max(laneY, gy + gh + GAP * 2);
    return { title: String(g.title ?? ""), x: gx, y: gy, w: gw, h: gh };
  });
  let k = 0;
  for (const n of notes) {
    if (pos.has(n.id as string)) continue;
    const x = num(n.x);
    const y = num(n.y);
    const placed = x != null && y != null;
    pos.set(n.id as string, {
      x: placed ? x : PAD + (k % 5) * (NOTE_W + GAP),
      y: placed ? y : laneY + Math.floor(k / 5) * (NOTE_H + GAP),
      color: typeof n.color === "string" ? n.color : "yellow",
    });
    if (!placed) k += 1;
  }
  // The viewer lays groups side by side, which can be thousands of pixels
  // wide; a thumbnail re-packs them into rows in the box's own shape,
  // each group carrying its notes along. Loose notes keep their lane.
  const packed = gs.map((g) => ({ ...g }));
  if (packed.length > 1) {
    const order = packed.map((g, i) => i).sort((a, b) => packed[a].y - packed[b].y || packed[a].x - packed[b].x);
    const total = packed.reduce((m, g) => m + g.w + GAP * 2, 0);
    const rowW = Math.max(groupW, Math.sqrt(total * Math.max(1, ...packed.map((g) => g.h + GAP * 2)) * (w / h)));
    let cx = PAD;
    let cy = PAD;
    let rowH = 0;
    for (const i of order) {
      const g = packed[i];
      if (cx > PAD && cx + g.w > rowW) {
        cx = PAD;
        cy += rowH + GAP * 2;
        rowH = 0;
      }
      const dx = cx - g.x;
      const dy = cy - g.y;
      for (const n of notes.filter((n) => n.group === groups[i].id)) {
        const q = pos.get(n.id as string);
        if (q) pos.set(n.id as string, { ...q, x: q.x + dx, y: q.y + dy });
      }
      g.x = cx;
      g.y = cy;
      cx += g.w + GAP * 2;
      rowH = Math.max(rowH, g.h);
    }
    const bottom = cy + rowH + GAP * 2;
    // Loose notes under the packed rows.
    const loose = notes.filter((n) => !groups.some((g) => g.id === n.group));
    const lowest = Math.min(...loose.map((n) => pos.get(n.id as string)?.y ?? Infinity));
    if (loose.length && Number.isFinite(lowest)) {
      for (const n of loose) {
        const q = pos.get(n.id as string)!;
        pos.set(n.id as string, { ...q, y: q.y - lowest + bottom });
      }
    }
  }
  const all = [...pos.values()];
  const extentX = Math.max(1, ...all.map((n) => n.x + NOTE_W), ...packed.map((g) => g.x + g.w));
  const extentY = Math.max(1, ...all.map((n) => n.y + NOTE_H), ...packed.map((g) => g.y + g.h));
  const pad = 6;
  // Never below a scale where a note is still a shape (12px wide): the
  // box clips what does not fit, which beats a field of dots.
  const s = Math.max(12 / NOTE_W, Math.min((w - pad * 2) / extentX, (h - pad * 2) / extentY));
  const sx = (x: number) => pad + x * s;
  const sy = (y: number) => pad + y * s;
  const out = [...pos.entries()].map(([id, n]) => ({ id, x: sx(n.x), y: sy(n.y), w: NOTE_W * s, h: NOTE_H * s, color: n.color }));
  const at = (id: string) => out.find((n) => n.id === id);
  const links = (Array.isArray(v.links) ? (v.links as Record<string, unknown>[]) : [])
    .map((l) => [at(String(l.from)), at(String(l.to))] as const)
    .filter((p): p is readonly [BoardThumb["notes"][number], BoardThumb["notes"][number]] => !!p[0] && !!p[1])
    .map(([a, b]) => ({ x1: a.x + a.w / 2, y1: a.y + a.h / 2, x2: b.x + b.w / 2, y2: b.y + b.h / 2 }));
  return {
    notes: out,
    groups: packed.map((g) => ({ title: g.title, x: sx(g.x), y: sy(g.y), w: g.w * s, h: g.h * s })),
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
