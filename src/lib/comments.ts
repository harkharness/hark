import { t } from "./i18n";
import type { CommentAnchor } from "../types";

/** `@hark` as a word, any case: the comment is a request (mirror of domain::comments). */
export function mentionsHark(text: string): boolean {
  return /(^|[^\w@])@hark(?![\w])/i.test(text);
}

/** A column index as its letter name (mirror of sheet::column_name). */
function letter(i: number): string {
  let n = i;
  let out = "";
  for (;;) {
    out = String.fromCharCode(65 + (n % 26)) + out;
    if (n < 26) break;
    n = Math.floor(n / 26) - 1;
  }
  return out;
}

/** The anchor as the person reads it. */
export function anchorLabel(a: CommentAnchor): string {
  switch (a.kind) {
    case "page":
      return t("cm_anchor_page");
    case "cell":
      return t("cm_anchor_cell", { c: `${letter(a.col)}${a.row + 2}`, t: a.tab_name ?? String(a.tab + 1) });
    case "note":
      return t("cm_anchor_note", { id: a.id });
    case "panel":
      return t("cm_anchor_panel", { t: a.title ?? String(a.index + 1) });
    case "slide":
      return t("cm_anchor_slide", { n: a.n + 1 });
    case "quote":
      return t("cm_anchor_quote", { q: a.text.split(/\s+/).join(" ").slice(0, 80) });
    case "region": {
      const r = (n: number) => String(Math.round(n));
      const shape =
        a.points && a.points.length > 1
          ? t("cm_anchor_stroke", { x: r(a.x), y: r(a.y), w: r(a.w), h: r(a.h) })
          : a.w < 1 && a.h < 1
            ? t("cm_anchor_point", { x: r(a.x), y: r(a.y) })
            : t("cm_anchor_region", { x: r(a.x), y: r(a.y), w: r(a.w), h: r(a.h) });
      const slide = a.slide != null ? t("cm_anchor_region_slide", { n: a.slide + 1 }) : "";
      const over = a.over?.trim() ? t("cm_anchor_over", { q: a.over.trim().split(/\s+/).join(" ").slice(0, 80) }) : "";
      return `${shape}${slide}${over}`;
    }
  }
}

/** Two anchors name the same place. */
export function sameAnchor(a: CommentAnchor, b: CommentAnchor): boolean {
  if (a.kind !== b.kind) return false;
  switch (a.kind) {
    case "page":
      return true;
    case "cell":
      return b.kind === "cell" && a.tab === b.tab && a.row === b.row && a.col === b.col;
    case "note":
      return b.kind === "note" && a.id === b.id;
    case "panel":
      return b.kind === "panel" && a.index === b.index;
    case "slide":
      return b.kind === "slide" && a.n === b.n;
    case "quote":
      return b.kind === "quote" && a.text === b.text;
    case "region":
      return b.kind === "region" && a.x === b.x && a.y === b.y && a.w === b.w && a.h === b.h && (a.slide ?? null) === (b.slide ?? null);
  }
}
