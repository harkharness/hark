import type { Msg } from "../types";

/**
 * Whether a log entry is one the thread already shows. The live events
 * push a turn as it happens; the mirror and the catch-up read the same
 * turn back from the session file a moment later, with the file's own
 * timestamps. Same thread, same role, same text is the same message,
 * whatever the clocks say. Only the recent tail is searched: an old
 * identical line must not swallow a new one.
 */
export function alreadyShown(shown: Msg[], m: Msg, tail = 40): boolean {
  const key = identity(m);
  if (!key) return false;
  const from = Math.max(0, shown.length - tail);
  for (let i = shown.length - 1; i >= from; i--) {
    if (identity(shown[i]) === key) return true;
  }
  return false;
}

function identity(m: Msg): string | null {
  const thread = m.task ?? "";
  switch (m.who) {
    case "user":
    case "hark":
      return `${thread}|${m.who}|${m.text}`;
    case "tool":
      return `${thread}|tool|${m.name}|${m.input}`;
    case "output":
      return `${thread}|output|${m.content}`;
    default:
      return null;
  }
}
