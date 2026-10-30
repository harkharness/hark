// The documentation tree as the screen reads it. Pure: no ipc, no state.
// The Rust side owns the index (crates/hark-core/src/domain/artifacts.rs);
// this side only flattens, filters and orders what it was given.
import type { ArtifactNode, ArtifactTree } from "../types";

/** One painted line of the tree column. */
export type Row = { node: ArtifactNode; depth: number; hasChildren: boolean };

/** Lowercase, accent-free: "Decisões" matches "decisoes". */
export const fold = (s: string) =>
  s
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "")
    .toLowerCase();

/** The children of a node (null = the root), in index order; a child the
 *  order list forgot still shows, after the ordered ones. */
export function childrenOf(tree: ArtifactTree, parent: string | null): ArtifactNode[] {
  const key = parent ?? "";
  const byId = new Map(tree.nodes.map((n) => [n.id, n]));
  const ordered = (tree.order[key] ?? []).map((id) => byId.get(id)).filter((n): n is ArtifactNode => !!n && (n.parent ?? null) === parent);
  const seen = new Set(ordered.map((n) => n.id));
  const loose = tree.nodes.filter((n) => (n.parent ?? null) === parent && !seen.has(n.id));
  return [...ordered, ...loose];
}

/** The tree flattened for painting: children only under open parents.
 *  A query turns the column into a flat list of the matching titles. */
export function rows(tree: ArtifactTree, open: Set<string>, query: string): Row[] {
  const q = fold(query.trim());
  if (q) {
    return tree.nodes
      .filter((n) => fold(n.title).includes(q))
      .map((n) => ({ node: n, depth: 0, hasChildren: childrenOf(tree, n.id).length > 0 }));
  }
  const out: Row[] = [];
  const walk = (parent: string | null, depth: number) => {
    for (const node of childrenOf(tree, parent)) {
      const kids = childrenOf(tree, node.id);
      out.push({ node, depth, hasChildren: kids.length > 0 });
      if (kids.length > 0 && open.has(node.id)) walk(node.id, depth + 1);
    }
  };
  walk(null, 0);
  return out;
}

/** Titles from the root down to the node, the node included. */
export function crumbOf(tree: ArtifactTree, id: string): string[] {
  const byId = new Map(tree.nodes.map((n) => [n.id, n]));
  const out: string[] = [];
  let cur = byId.get(id);
  const guard = new Set<string>();
  while (cur && !guard.has(cur.id)) {
    guard.add(cur.id);
    out.unshift(cur.title);
    cur = cur.parent ? byId.get(cur.parent) : undefined;
  }
  return out;
}

/** Newest touch first, at most n. */
export function recents(tree: ArtifactTree, n: number): ArtifactNode[] {
  return tree.nodes.filter((n) => !n.orphan).sort((a, b) => (a.updated_at < b.updated_at ? 1 : a.updated_at > b.updated_at ? -1 : 0)).slice(0, n);
}
