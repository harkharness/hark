import { useEffect, useMemo, useRef, useState } from "react";
import * as ipc from "../lib/ipc";
import type { OpenFile, Project } from "../types";

type Node = {
  name: string;
  path: string; // relative path from the project root
  children?: Node[]; // undefined = file
};

/** Build a nested tree from the flat relative paths the backend returns. */
function buildTree(paths: string[]): Node[] {
  const root: Node[] = [];
  for (const path of paths) {
    const parts = path.split("/");
    let level = root;
    let acc = "";
    for (let i = 0; i < parts.length; i++) {
      acc = acc ? `${acc}/${parts[i]}` : parts[i];
      const isFile = i === parts.length - 1;
      let node = level.find((n) => n.name === parts[i] && (n.children ? !isFile : isFile));
      if (!node) {
        node = { name: parts[i], path: acc, children: isFile ? undefined : [] };
        level.push(node);
      }
      if (node.children) level = node.children;
    }
  }
  const sort = (nodes: Node[]) => {
    nodes.sort((a, b) => {
      const dirA = a.children ? 0 : 1;
      const dirB = b.children ? 0 : 1;
      return dirA - dirB || a.name.localeCompare(b.name);
    });
    nodes.forEach((n) => n.children && sort(n.children));
  };
  sort(root);
  return root;
}

/** Every folder above a relative path: "a/b/c.rs" → ["a", "a/b"]. */
function ancestors(rel: string): string[] {
  const parts = rel.split("/");
  const out: string[] = [];
  for (let i = 1; i < parts.length; i++) out.push(parts.slice(0, i).join("/"));
  return out;
}

/**
 * Clickable file tree of one project. Fully local: one listing call,
 * folders expand client-side, clicking a file opens a tab. The tab in
 * front is revealed (its folders come open) and marked, like an editor.
 */
export default function FileTree({
  project,
  activeRel,
  onOpen,
}: {
  project: Project;
  /** The open file, relative to this project, when it lives here. */
  activeRel?: string;
  onOpen: (file: OpenFile) => void;
}) {
  const [paths, setPaths] = useState<string[] | null>(null);
  const [open, setOpen] = useState<Set<string>>(new Set());
  const activeRowRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    ipc
      .projectFiles(project.path, "", 3000)
      .then(setPaths)
      .catch(() => setPaths([]));
  }, [project.path]);

  // Reveal: the folders above the open file come open...
  useEffect(() => {
    if (!activeRel) return;
    const above = ancestors(activeRel);
    if (above.length === 0) return;
    setOpen((old) => {
      if (above.every((dir) => old.has(dir))) return old;
      const next = new Set(old);
      above.forEach((dir) => next.add(dir));
      return next;
    });
  }, [activeRel]);

  // ...and its row is brought into view once it exists.
  useEffect(() => {
    activeRowRef.current?.scrollIntoView?.({ block: "nearest" });
  }, [activeRel, open, paths]);

  const tree = useMemo(() => buildTree(paths ?? []), [paths]);

  function toggle(path: string) {
    setOpen((old) => {
      const next = new Set(old);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  }

  const render = (nodes: Node[], depth: number) =>
    nodes.map((node) =>
      node.children ? (
        <div key={node.path}>
          <button
            className="tree-row"
            style={{ paddingLeft: 8 + depth * 12 }}
            onClick={() => toggle(node.path)}
          >
            <span className="tree-caret">{open.has(node.path) ? "▾" : "▸"}</span>
            {node.name}
          </button>
          {open.has(node.path) && render(node.children, depth + 1)}
        </div>
      ) : (
        <button
          key={node.path}
          ref={node.path === activeRel ? activeRowRef : undefined}
          className={`tree-row tree-file ${node.path === activeRel ? "on" : ""}`}
          style={{ paddingLeft: 8 + depth * 12 }}
          onClick={() =>
            onOpen({ abs: `${project.path}/${node.path}`, rel: node.path, project })
          }
          title={node.path}
        >
          {node.name}
        </button>
      ),
    );

  if (paths === null) return <div className="side-empty">listando…</div>;
  if (paths.length === 0) return <div className="side-empty">vazio</div>;
  return <div className="filetree">{render(tree, 0)}</div>;
}
