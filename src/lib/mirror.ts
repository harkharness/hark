import type { Msg } from "../types";

/** One line of the condensed mirror of a chat in focus. */
export type MirrorLine =
  | { kind: "user"; text: string }
  | { kind: "text"; text: string }
  | { kind: "tool"; text: string; ok?: boolean }
  | { kind: "perm"; text: string; state: "allow" | "deny" | "waiting" | "expired" };

/** The one thing a tool call is about: its command, its path, its query. */
function gist(input: string): string {
  try {
    const o = JSON.parse(input) as Record<string, unknown>;
    for (const k of ["command", "file_path", "path", "pattern", "query", "url", "description"]) {
      if (typeof o[k] === "string" && o[k]) return clip(o[k] as string);
    }
    const first = Object.values(o).find((v) => typeof v === "string" && v);
    return first ? clip(first as string) : "";
  } catch {
    return clip(input);
  }
}

const clip = (s: string, n = 80) => (s.length > n ? `${s.slice(0, n - 1)}…` : s);

/**
 * A chat in focus, condensed for Início: every tool call is one line,
 * marked by its result; a permission is one line with its state; the
 * words (yours and the agent's) stay whole. Newest last, tail only.
 */
export function condense(msgs: Msg[], max = 8): MirrorLine[] {
  const out: MirrorLine[] = [];
  for (const m of msgs) {
    if (m.who === "user") out.push({ kind: "user", text: m.text });
    else if (m.who === "hark") out.push({ kind: "text", text: m.text });
    else if (m.who === "tool") out.push({ kind: "tool", text: `${m.name} · ${gist(m.input)}` });
    else if (m.who === "output") {
      const last = out[out.length - 1];
      if (last?.kind === "tool" && last.ok === undefined) last.ok = !m.error;
    } else if (m.who === "permission") {
      out.push({ kind: "perm", text: m.tool, state: m.decision ?? (m.expired ? "expired" : "waiting") });
    }
  }
  return out.slice(-max);
}
