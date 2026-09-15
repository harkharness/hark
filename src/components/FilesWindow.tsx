import { useEffect, useRef } from "react";
import { FolderOpen, PanelLeftClose, PanelLeftOpen } from "lucide-react";
import { Panel, PanelGroup, PanelResizeHandle } from "react-resizable-panels";
import FilesPanel from "./FilesPanel";
import FilesEditor from "./FilesEditor";
import type { OpenFile, Project } from "../types";
import { t } from "../lib/i18n";

/**
 * The "Arquivos" window body: the tree column beside the tabs' viewer, ONE
 * frame where there used to be two. The tree collapses (its toggle lives in
 * the frame header, its state in `useFilesTree`); the viewer keeps every tab
 * mounted as before. No tab open = an empty state that says what to do, so
 * closing the last tab never leaves a dead window behind.
 */
export default function FilesWindow({
  projects,
  files,
  active,
  initialProject,
  treeOpen,
  onOpen,
  onDirty,
  onWidth,
}: {
  projects: Project[];
  files: OpenFile[];
  active: number;
  /** Project expanded when the tree opens (sidebar folder click). */
  initialProject?: string;
  treeOpen: boolean;
  onOpen: (file: OpenFile) => void;
  onDirty: (abs: string, dirty: boolean) => void;
  /** The frame's own width, for the tree's default (rail vs full screen). */
  onWidth?: (px: number) => void;
}) {
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = rootRef.current;
    if (!el || !onWidth || typeof ResizeObserver === "undefined") return;
    onWidth(el.getBoundingClientRect().width);
    const observer = new ResizeObserver((entries) => {
      const width = entries[0]?.contentRect.width;
      if (width !== undefined) onWidth(width);
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, [onWidth]);

  const activeAbs = files[active]?.abs;
  const main =
    files.length > 0 ? (
      <FilesEditor files={files} active={active} onDirty={onDirty} />
    ) : (
      <FilesEmpty treeOpen={treeOpen} />
    );

  return (
    <div className="fileswin" ref={rootRef}>
      {treeOpen ? (
        <PanelGroup direction="horizontal" autoSaveId="hark-files">
          <Panel defaultSize={30} minSize={16} maxSize={60} className="fileswin-tree">
            <FilesPanel
              projects={projects}
              initialProject={initialProject}
              activeAbs={activeAbs}
              onOpen={onOpen}
            />
          </Panel>
          <PanelResizeHandle className="rhandle" />
          <Panel minSize={30} className="fileswin-main">
            {main}
          </Panel>
        </PanelGroup>
      ) : (
        <div className="fileswin-main">{main}</div>
      )}
    </div>
  );
}

/** The header control that folds the tree: the one thing the frame gained. */
export function TreeToggle({ open, onToggle }: { open: boolean; onToggle: () => void }) {
  return (
    <button
      className={`frame-ctl ${open ? "on" : ""}`}
      title={t(open ? "files_tree_hide" : "files_tree_show")}
      onClick={onToggle}
    >
      {open ? <PanelLeftClose size={13} /> : <PanelLeftOpen size={13} />}
    </button>
  );
}

function FilesEmpty({ treeOpen }: { treeOpen: boolean }) {
  return (
    <div className="files-empty">
      <FolderOpen size={22} strokeWidth={1.6} />
      <div className="files-empty-title">{t("files_empty_title")}</div>
      <div className="files-empty-hint">{t(treeOpen ? "files_empty_tree" : "files_empty_closed")}</div>
    </div>
  );
}
