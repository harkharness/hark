import type { JournalTurn } from "../types";

/** One past day of Hark's chat, as the Início sidebar lists it. */
export type Day = {
  /** Local calendar day, YYYY-MM-DD. */
  date: string;
  /** "ontem · seg 03", "qui 30": yesterday says so, the rest say the weekday. */
  label: string;
  count: number;
  /** The first question of the day: what it was about. */
  first: string;
};

const WEEKDAYS: Record<string, string[]> = {
  pt: ["dom", "seg", "ter", "qua", "qui", "sex", "sáb"],
  en: ["sun", "mon", "tue", "wed", "thu", "fri", "sat"],
};
const YESTERDAY: Record<string, string> = { pt: "ontem", en: "yesterday" };

const key = (d: Date) =>
  `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;

/** The local calendar day of a journal timestamp, or null when unreadable. */
export function dayOf(ts: string): string | null {
  const at = new Date(ts);
  return Number.isNaN(at.getTime()) ? null : key(at);
}

/**
 * The journal grouped by local day, newest first, today left out: today is
 * the thread itself. Pure; the sidebar renders it.
 */
export function days(turns: JournalTurn[], now: Date, lang = "pt"): Day[] {
  const today = key(now);
  const yesterday = key(new Date(now.getTime() - 86_400_000));
  const weekdays = WEEKDAYS[lang] ?? WEEKDAYS.pt;
  const by = new Map<string, { count: number; first: string; at: Date }>();
  for (const turn of turns) {
    const at = new Date(turn.ts);
    if (Number.isNaN(at.getTime())) continue;
    const k = key(at);
    if (k === today) continue;
    const cur = by.get(k);
    if (cur) {
      cur.count += 1;
      if (at < cur.at) {
        cur.at = at;
        cur.first = turn.question;
      }
    } else by.set(k, { count: 1, first: turn.question, at });
  }
  return [...by.entries()]
    .sort(([a], [b]) => b.localeCompare(a))
    .map(([date, v]) => {
      const short = `${weekdays[v.at.getDay()]} ${String(v.at.getDate()).padStart(2, "0")}`;
      const label = date === yesterday ? `${YESTERDAY[lang] ?? YESTERDAY.pt} · ${short}` : short;
      return { date, label, count: v.count, first: v.first };
    });
}
