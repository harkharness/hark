// The search palette's rules: what matches, how the hits are grouped.
// Pure: the component brings the items, this says what to show.
import { fold } from "./artifacts";

export type SearchKind = "page" | "task" | "project" | "session" | "action";
export type SearchItem = {
  kind: SearchKind;
  id: string;
  title: string;
  sub?: string;
  /** ISO time of the last touch, for the recents. */
  when?: string;
  /** Waiting on the person (a permission, a question). */
  attention?: boolean;
};
export type Section = { id: string; items: SearchItem[] };

/** The tab order, and the order of the groups under a query. */
export const KINDS: SearchKind[] = ["task", "page", "project", "session", "action"];
const RECENT = 8;

/** Every typed word must appear in the title or subtitle, accents and
 *  case aside; a kind other than "all" keeps that kind only. */
export function searchItems(items: SearchItem[], query: string, kind: SearchKind | "all"): SearchItem[] {
  const words = fold(query).split(/\s+/).filter(Boolean);
  return items.filter((it) => {
    if (kind !== "all" && it.kind !== kind) return false;
    const hay = fold(`${it.title} ${it.sub ?? ""}`);
    return words.every((w) => hay.includes(w));
  });
}

/** Before a word is typed, "all" shows what needs you, what moved last
 *  and what you can do; typed, the hits group by kind in a fixed order;
 *  one kind is one flat group. Empty groups never show. */
export function sections(items: SearchItem[], query: string, kind: SearchKind | "all"): Section[] {
  const hits = searchItems(items, query, kind);
  if (kind !== "all") return hits.length ? [{ id: kind, items: hits }] : [];
  if (query.trim() === "") {
    const attention = hits.filter((i) => i.attention);
    const recent = hits
      .filter((i) => i.when)
      .sort((a, b) => (a.when! < b.when! ? 1 : a.when! > b.when! ? -1 : 0))
      .slice(0, RECENT);
    const actions = hits.filter((i) => i.kind === "action");
    return [
      { id: "attention", items: attention },
      { id: "recent", items: recent },
      { id: "actions", items: actions },
    ].filter((s) => s.items.length > 0);
  }
  return KINDS.map((k) => ({ id: k, items: hits.filter((i) => i.kind === k) })).filter((s) => s.items.length > 0);
}
