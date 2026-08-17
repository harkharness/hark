import { useEffect, useMemo, useState } from "react";
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

/**
 * Clickable file tree of one project (sidebar). Fully local: one listing
 * call, folders expand client-side, clicking a file opens the viewer.
 */
export default function FileTree({
  project,
  onOpen,
}: {
  project: Project;
  onOpen: (file: OpenFile) => void;
}) {
  const [paths, setPaths] = useState<string[] | null>(null);
  const [open, setOpen] = useState<Set<string>>(new Set());

  useEffect(() => {
    ipc
      .projectFiles(project.path, "", 3000)
      .then(setPaths)
      .catch(() => setPaths([]));
  }, [project.path]);

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
          className="tree-row tree-file"
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
