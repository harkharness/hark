/** What a `hark://` link names, as a click in a chat resolves it. Mirrors
 *  `domain::deep_link` on the Rust side: a page is known by its id, the
 *  file name without its page extension, however the agent wrote it. */

export type HarkTarget = { kind: "page"; space: string; id: string } | { kind: "chat"; session: string };

const PAGE_EXT = /\.(sheet\.json|board\.json|dash\.json|md|html)$/i;

function clean(seg: string): string | null {
  let s: string;
  try {
    s = decodeURIComponent(seg);
  } catch {
    return null;
  }
  return s && s !== "." && s !== ".." && !s.includes("/") ? s : null;
}

export function harkTarget(href: string): HarkTarget | null {
  if (!href.startsWith("hark://")) return null;
  const rest = href.slice("hark://".length).split("#")[0].replace(/\/+$/, "");
  const parts = rest.split("/");
  if (parts[0] === "chat" && parts.length === 2) {
    const session = clean(parts[1]);
    return session ? { kind: "chat", session } : null;
  }
  if (parts[0] === "artifacts" && parts.length === 3) {
    const space = clean(parts[1]);
    const file = clean(parts[2]);
    if (!space || !file) return null;
    const id = file.replace(PAGE_EXT, "");
    return id ? { kind: "page", space, id } : null;
  }
  return null;
}

/** A file the agent wrote under `~/.hark/artifacts/<space>/…` is a page
 *  of that space, not a file for the editor: the session bar's outputs
 *  open it in the artifacts view. `index.json` is the space's own index. */
export function artifactFile(path: string): { space: string; file: string } | null {
  const m = /^(?:~|\/Users\/[^/]+|\/home\/[^/]+)\/\.hark\/artifacts\/([^/]+)\/(.+)$/.exec(path.trim());
  if (!m) return null;
  const [, space, file] = m;
  if (file === "index.json" || file.endsWith("/")) return null;
  return { space, file };
}

/** The id of the page stored at `file` (relative to the space root). The
 *  index is the truth — a page renamed on disk keeps its id — and the
 *  file's stem is what a page not yet indexed will be called. */
export function pageId(file: string, nodes: { id: string; path: string }[]): string {
  const hit = nodes.find((n) => n.path === file);
  if (hit) return hit.id;
  const base = file.split("/").pop() ?? file;
  return base.replace(PAGE_EXT, "");
}
