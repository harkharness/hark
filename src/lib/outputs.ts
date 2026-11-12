// What a thread produced: the files its tool calls wrote, edited or
// delivered. Pure: reads the messages already on screen, no ipc. The
// "Saídas" section of the session panel is this list.
import type { ArtifactKind, ArtifactTree, Msg } from "../types";
import { artifactFile, pageId } from "./harkLink";

export type OutputKind = "written" | "edited" | "delivered";
/** The page an output is, when it lives under the artifacts folder. */
export type OutputPage = { space: string; id: string; title: string; kind: ArtifactKind };
export type Output = { path: string; kind: OutputKind; touches: number; page?: OutputPage };

/** The kind a page file's extension says, for a page the index has not
 *  seen yet (the agent just wrote it). */
export function kindOfFile(file: string): ArtifactKind {
  if (/\.sheet\.json$/i.test(file)) return "sheet";
  if (/\.board\.json$/i.test(file)) return "board";
  if (/\.dash\.json$/i.test(file)) return "data";
  return "doc";
}

/** The outputs with their page attached, from the spaces' indexes: the
 *  title and the kind the bar shows instead of a file name. Pure. */
export function withPages(outputs: Output[], trees: Record<string, ArtifactTree | undefined>): Output[] {
  return outputs.map((o) => {
    const art = artifactFile(o.path);
    if (!art) return o;
    const nodes = trees[art.space]?.nodes ?? [];
    const id = pageId(art.file, nodes);
    const node = nodes.find((n) => n.id === id);
    return { ...o, page: { space: art.space, id, title: node?.title ?? id, kind: node?.kind ?? kindOfFile(art.file) } };
  });
}

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
