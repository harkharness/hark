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
