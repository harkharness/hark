// What a thread produced: the files its tool calls wrote, edited or
// delivered. Pure: reads the messages already on screen, no ipc. The
// "Saídas" section of the session panel is this list.
import type { Msg } from "../types";

export type OutputKind = "written" | "edited" | "delivered";
export type Output = { path: string; kind: OutputKind; touches: number };

const RANK: Record<OutputKind, number> = { edited: 0, written: 1, delivered: 2 };

/** Which paths a tool call touched, and how. Reads and shells touch
 *  nothing; a payload that is not JSON is not a file operation. */
function touches(name: string, raw: string): { path: string; kind: OutputKind }[] {
  let input: Record<string, unknown>;
  try {
    input = JSON.parse(raw) as Record<string, unknown>;
  } catch {
    return [];
  }
  const path = typeof input.file_path === "string" ? input.file_path : undefined;
  switch (name) {
    case "Write":
      return path ? [{ path, kind: "written" }] : [];
    case "Edit":
    case "MultiEdit":
    case "NotebookEdit":
      return path ? [{ path, kind: "edited" }] : [];
    case "SendUserFile":
      return Array.isArray(input.files)
        ? input.files.filter((f): f is string => typeof f === "string").map((p) => ({ path: p, kind: "delivered" as const }))
        : [];
    default:
      return [];
  }
}

/** The thread's outputs, one per path, the most recently touched first.
 *  `thread` is the message label (null = the general thread). */
export function collectOutputs(messages: Msg[], thread: string | null): Output[] {
  const seen = new Map<string, Output & { last: number }>();
  messages.forEach((m, i) => {
    if (m.who !== "tool") return;
    const label = "task" in m && m.task ? m.task : null;
    if (label !== thread) return;
    for (const { path, kind } of touches(m.name, m.input)) {
      const prev = seen.get(path);
      if (prev) {
        prev.touches += 1;
        prev.last = i;
        if (RANK[kind] > RANK[prev.kind]) prev.kind = kind;
      } else {
        seen.set(path, { path, kind, touches: 1, last: i });
      }
    }
  });
  return [...seen.values()]
    .sort((a, b) => b.last - a.last)
    .map(({ path, kind, touches }) => ({ path, kind, touches }));
}
