import { useEffect, useState } from "react";
import { Eye, Pencil, Save, X } from "lucide-react";
import * as ipc from "../lib/ipc";
import { highlightFile } from "../lib/highlight";
import Markdown from "./Markdown";
import type { OpenFile } from "../types";

const isMarkdown = (rel: string) => /\.(md|markdown)$/i.test(rel);

/**
 * Local file panel, zero tokens both ways. Markdown opens RENDERED (edit
 * unlocks the source); code opens straight in edit mode. Cmd+S saves; the
 * amber dot marks unsaved manual edits, VSCode-style.
 */
export default function FileViewer({ file, onClose }: { file: OpenFile; onClose: () => void }) {
  const [content, setContent] = useState("");
  const [truncated, setTruncated] = useState(false);
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");
  const [status, setStatus] = useState<string | null>(null);

  const dirty = editing && draft !== content;

  useEffect(() => {
    setStatus(null);
    ipc
      .fileRead(file.abs)
      .then((out) => {
        setContent(out.content);
        setDraft(out.content);
        setTruncated(out.truncated);
        // Markdown is for reading first; everything else is for editing.
        setEditing(!isMarkdown(file.rel) && !out.truncated);
      })
      .catch((err) => {
        setContent("");
        setEditing(false);
        setStatus(`erro: ${err}`);
      });
  }, [file.abs, file.rel]);

  async function save() {
    try {
      await ipc.fileSave(file.abs, draft);
      setContent(draft);
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
        {dirty && <span className="viewer-dirty" title="edições não salvas (Cmd+S)" />}
        {truncated && <span className="viewer-badge">truncado</span>}
        {status && <span className="viewer-status">{status}</span>}
        <span className="viewer-actions">
          {editing ? (
            <>
              <button onClick={save} disabled={!dirty} title="salvar (Cmd+S)">
                <Save size={13} />
              </button>
              <button
                onClick={() => {
                  setDraft(content);
                  setEditing(false);
                }}
                title="voltar à visualização"
              >
                <Eye size={13} />
              </button>
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
              <Pencil size={13} />
            </button>
          )}
          <button onClick={onClose} title="fechar">
            <X size={13} />
          </button>
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
            if (e.key === "Escape") e.stopPropagation();
          }}
          spellCheck={false}
        />
      ) : isMarkdown(file.rel) ? (
        <div className="viewer-md">
          <Markdown>{content}</Markdown>
        </div>
      ) : (
        <pre className="viewer-code">
          <code dangerouslySetInnerHTML={{ __html: highlightFile(file.rel, content) }} />
        </pre>
      )}
    </div>
  );
}
