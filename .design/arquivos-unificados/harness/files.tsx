// Dev harness for the files window, served by `vite` (no Tauri): the real
// components with a FAKE backend installed at the Tauri boundary, so the
// tree, the filter, the tabs, the viewer and the gutter all run for real.
// Open: http://localhost:1420/.design/arquivos-unificados/harness/files.html
import { useCallback, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import "/src/styles.css";
import FilesWindow, { TreeToggle } from "/src/components/FilesWindow";
import { FileTabs } from "/src/components/FilesEditor";
import PanelFrame from "/src/components/PanelFrame";
import { useFilesTree } from "/src/hooks/useFilesTree";
import type { OpenFile, Project } from "/src/types";

const ROOT = "/Users/dev/Projects/vox";
const VOX: Project = { name: "vox", path: ROOT };
const OTHER: Project = { name: "workspace-fabrica", path: "/Users/dev/Projects/workspace-fabrica" };
// Stable, like the App's state: a fresh array per render would refetch the filter.
const PROJECTS: Project[] = [VOX, OTHER];

const VOX_FILES = [
  ".design/arquivos-unificados/build.mjs",
  ".design/arquivos-unificados/canvas.json",
  ".design/arquivos-unificados/esforco.md",
  ".github/workflows/test.yml",
  "crates/hark-core/src/domain/agents.rs",
  "crates/hark-core/src/domain/registry.rs",
  "crates/hark-plugin-acp/fixtures/initialize.codex-acp-1.11.0.json",
  "crates/hark-plugin-acp/src/session.rs",
  "docs/PLUGINS.md",
  "scripts/release.sh",
  "src/App.tsx",
  "src/Mother.tsx",
  "src/components/FilesWindow.tsx",
  "src/components/FileViewer.tsx",
  "src/lib/filesTree.ts",
  "src/styles.css",
  "src-tauri/src/lib.rs",
  ".gitignore",
  "Cargo.toml",
  "index.html",
  "install.sh",
  "package.json",
  "README.md",
];
const OTHER_FILES = ["docs/status/current-work.md", "CLAUDE.md", "README.md"];

function matches(query: string, path: string): boolean {
  const q = query.toLowerCase();
  const p = path.toLowerCase();
  let at = 0;
  for (const ch of q) {
    at = p.indexOf(ch, at);
    if (at < 0) return false;
    at += 1;
  }
  return true;
}

// The fake backend: what `invoke` reaches in the real app.
(window as unknown as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__ = {
  invoke: async (cmd: string, args: Record<string, unknown>) => {
    if (cmd === "project_files") {
      const files = args.path === ROOT ? VOX_FILES : OTHER_FILES;
      const query = String(args.query ?? "");
      return (query ? files.filter((f) => matches(query, f)) : files).slice(0, Number(args.limit ?? 30));
    }
    if (cmd === "file_read") {
      const path = String(args.path);
      if (path.startsWith(`${ROOT}/`)) {
        const res = await fetch(`/${path.slice(ROOT.length + 1)}`);
        if (res.ok) return { content: await res.text(), truncated: false };
      }
      return { content: `# ${path.split("/").pop()}\n\n(conteúdo falso do harness)\n`, truncated: false };
    }
    if (cmd === "file_save") return null;
    return null;
  },
  transformCallback: () => 0,
  metadata: {},
};

function Harness({ width, label }: { width: number; label: string }) {
  const [files, setFiles] = useState<OpenFile[]>([]);
  const [active, setActive] = useState(0);
  const [dirty, setDirty] = useState<Set<string>>(new Set());
  const [expanded, setExpanded] = useState(false);
  const tree = useFilesTree();
  const lastFocus = useRef(new Map<string, number>());

  const open = useCallback((file: OpenFile) => {
    setFiles((old) => {
      const at = old.findIndex((f) => f.abs === file.abs);
      if (at >= 0) {
        setActive(at);
        return file.line && old[at].line !== file.line
          ? old.map((f, i) => (i === at ? { ...f, line: file.line } : f))
          : old;
      }
      setActive(old.length);
      return [...old, file];
    });
  }, []);

  const onDirty = useCallback((abs: string, isDirty: boolean) => {
    setDirty((old) => {
      if (old.has(abs) === isDirty) return old;
      const next = new Set(old);
      if (isDirty) next.add(abs);
      else next.delete(abs);
      return next;
    });
  }, []);

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
      <div style={{ display: "flex", gap: 8, alignItems: "center", color: "var(--dim)", fontSize: 12 }}>
        <span>{label}</span>
        <button onClick={() => open({ abs: `${ROOT}/README.md`, rel: "README.md", line: 21, project: VOX })}>
          chat: README.md:21
        </button>
        <button onClick={() => open({ abs: `${ROOT}/install.sh`, rel: "install.sh", project: VOX })}>
          chat: install.sh
        </button>
        <button onClick={() => open({ abs: `${ROOT}/src/lib/filesTree.ts`, rel: "src/lib/filesTree.ts", line: 9, project: VOX })}>
          chat: src/lib/filesTree.ts:9
        </button>
      </div>
      <div style={{ width: expanded ? 1240 : width, height: 640, display: "flex", background: "var(--bg)" }}>
        <PanelFrame
          title="Arquivos"
          lead={<TreeToggle open={tree.open} onToggle={tree.toggle} />}
          tabs={
            <FileTabs
              files={files}
              active={active}
              dirty={dirty}
              onActivate={(i) => {
                setActive(i);
                const f = files[i];
                if (f) lastFocus.current.set(f.abs, Date.now());
              }}
              onCloseTab={(i) =>
                setFiles((old) => {
                  const next = old.filter((_, j) => j !== i);
                  setActive((a) => Math.max(0, a > i ? a - 1 : Math.min(a, next.length - 1)));
                  return next;
                })
              }
            />
          }
          expanded={expanded}
          onToggleExpand={() => setExpanded((e) => !e)}
          onToggleCollapse={() => {}}
          onClose={() => setFiles([])}
        >
          <FilesWindow
            projects={PROJECTS}
            files={files}
            active={active}
            treeOpen={tree.open}
            onOpen={open}
            onDirty={onDirty}
            onWidth={tree.onWidth}
          />
        </PanelFrame>
      </div>
    </div>
  );
}

createRoot(document.getElementById("root")!).render(
  <div style={{ padding: 16, display: "flex", flexDirection: "column", gap: 24, background: "var(--bg-deep)", minHeight: "100vh" }}>
    <Harness width={900} label="larga (tela cheia)" />
    <Harness width={440} label="rail (42%)" />
  </div>,
);
