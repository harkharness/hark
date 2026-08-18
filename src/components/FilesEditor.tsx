import { X } from "lucide-react";
import FileViewer from "./FileViewer";
import type { OpenFile } from "../types";

/**
 * The "Arquivo" window body: up to 5 file tabs. Every tab stays MOUNTED
 * (hidden with CSS) so unsaved drafts survive tab switches; the amber dot
 * marks dirty tabs, and the LRU eviction upstream never touches them.
 */
/** The Arquivo window's tab strip — lives in the PanelFrame header. */
export function FileTabs({
  files,
  active,
  dirty,
  onActivate,
  onCloseTab,
}: {
  files: OpenFile[];
  active: number;
  dirty: Set<string>;
  onActivate: (index: number) => void;
  onCloseTab: (index: number) => void;
}) {
  return (
    <>
      {files.map((f, i) => (
        <span
          key={f.abs}
          className={`filetab ${i === active ? "on" : ""}`}
          title={f.abs}
          onClick={() => onActivate(i)}
        >
          {dirty.has(f.abs) && <span className="viewer-dirty" />}
          {f.rel.split("/").pop()}
          <button
            className="filetab-close"
            title={dirty.has(f.abs) ? "tem edição não salva" : "fechar aba"}
            onClick={(e) => {
              e.stopPropagation();
              onCloseTab(i);
            }}
          >
            <X size={11} />
          </button>
        </span>
      ))}
    </>
  );
}

export default function FilesEditor({
  files,
  active,
  onDirty,
}: {
  files: OpenFile[];
  active: number;
  onDirty: (abs: string, isDirty: boolean) => void;
}) {
  return (
    <div className="fileseditor">
      {files.map((f, i) => (
        <div key={f.abs} className="filetab-body" style={{ display: i === active ? "flex" : "none" }}>
          <FileViewer file={f} onDirty={onDirty} />
        </div>
      ))}
    </div>
  );
}
