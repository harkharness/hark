import { useEffect, useState } from "react";
import * as ipc from "../lib/ipc";
import { highlightFile } from "../lib/highlight";
import type { OpenFile } from "../types";

/**
 * Local file viewer/editor in a side panel. Reading and small line edits
 * happen HERE, on disk, with zero tokens: the whole point is not paying a
 * model to cat a file.
 */
export default function FileViewer({ file, onClose }: { file: OpenFile; onClose: () => void }) {
  const [content, setContent] = useState("");
  const [truncated, setTruncated] = useState(false);
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");
  const [status, setStatus] = useState<string | null>(null);

  useEffect(() => {
    setEditing(false);
    setStatus(null);
    ipc
      .fileRead(file.abs)
      .then((out) => {
        setContent(out.content);
        setTruncated(out.truncated);
      })
      .catch((err) => {
        setContent("");
        setStatus(`erro: ${err}`);
      });
  }, [file.abs]);

  async function save() {
    try {
      await ipc.fileSave(file.abs, draft);
      setContent(draft);
      setEditing(false);
      setStatus("salvo ✓");
      window.setTimeout(() => setStatus(null), 2500);
    } catch (err) {
      setStatus(`erro ao salvar: ${err}`);
    }
  }

  return (
    <div className="viewer">
      <div className="viewer-head">
        <span className="viewer-proj">{file.project.name}/</span>
        <span className="viewer-path" title={file.abs}>
          {file.rel}
        </span>
        {truncated && <span className="viewer-badge">truncado</span>}
        {status && <span className="viewer-status">{status}</span>}
        <span className="viewer-actions">
          {editing ? (
            <>
              <button onClick={save}>salvar</button>
              <button onClick={() => setEditing(false)}>cancelar</button>
            </>
          ) : (
            <button
              onClick={() => {
                setDraft(content);
                setEditing(true);
              }}
              disabled={truncated}
              title={truncated ? "arquivo grande demais para editar aqui" : "editar localmente"}
            >
              editar
            </button>
          )}
          <button onClick={onClose}>×</button>
        </span>
      </div>
      {editing ? (
        <textarea
          className="viewer-edit"
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            if ((e.metaKey || e.ctrlKey) && e.key === "s") {
              e.preventDefault();
              save();
            }
            if (e.key === "Escape") setEditing(false);
          }}
          spellCheck={false}
        />
      ) : (
        <pre className="viewer-code">
          <code dangerouslySetInnerHTML={{ __html: highlightFile(file.rel, content) }} />
        </pre>
      )}
    </div>
  );
}
