import { useEffect, useRef, useState } from "react";
import { Eye, Pencil, Save } from "lucide-react";
import * as ipc from "../lib/ipc";
import { highlightFile } from "../lib/highlight";
import Markdown from "./Markdown";
import type { OpenFile } from "../types";
import { t } from "../lib/i18n";

const isMarkdown = (rel: string) => /\.(md|markdown)$/i.test(rel);
/** Above this size, live re-highlighting on every keystroke gets slow:
 * fall back to a plain (uncolored) textarea. */
const HIGHLIGHT_EDIT_MAX = 120_000;

/**
 * Local file panel, zero tokens both ways. Markdown opens RENDERED (edit
 * unlocks the source); code opens straight in edit mode. Cmd+S saves; the
 * amber dot marks unsaved manual edits, VSCode-style.
 */
export default function FileViewer({
  file,
  onDirty,
}: {
  file: OpenFile;
  /** Reports unsaved-edit state upward (tab dots + LRU protection). */
  onDirty?: (abs: string, dirty: boolean) => void;
}) {
  const [content, setContent] = useState("");
  const [truncated, setTruncated] = useState(false);
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");
  const [status, setStatus] = useState<string | null>(null);
  const underRef = useRef<HTMLPreElement>(null);

  const dirty = editing && draft !== content;
  const liveHighlight = draft.length < HIGHLIGHT_EDIT_MAX;

  useEffect(() => {
    onDirty?.(file.abs, dirty);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [dirty, file.abs]);

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
      setStatus(t("viewer_saved"));
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
        {dirty && <span className="viewer-dirty" title={t("viewer_dirty")} />}
        {truncated && <span className="viewer-badge">truncado</span>}
        {status && <span className="viewer-status">{status}</span>}
        <span className="viewer-actions">
          {editing ? (
            <>
              <button onClick={save} disabled={!dirty} title={t("viewer_save")}>
                <Save size={13} />
              </button>
              <button
                onClick={() => {
                  setDraft(content);
                  setEditing(false);
                }}
                title={t("viewer_back")}
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
              title={truncated ? t("viewer_too_big") : t("viewer_edit")}
            >
              <Pencil size={13} />
            </button>
          )}
        </span>
      </div>
      {editing ? (
        <div className="viewer-editwrap">
          {liveHighlight && (
            <pre className="viewer-code under" aria-hidden ref={underRef}>
              <code
                dangerouslySetInnerHTML={{ __html: `${highlightFile(file.rel, draft)}\n` }}
              />
            </pre>
          )}
          <textarea
            className={`viewer-edit ${liveHighlight ? "ghost" : ""}`}
            value={draft}
            wrap="off"
            onChange={(e) => setDraft(e.target.value)}
            onScroll={(e) => {
              const under = underRef.current;
              if (under) {
                under.scrollTop = e.currentTarget.scrollTop;
                under.scrollLeft = e.currentTarget.scrollLeft;
              }
            }}
            onKeyDown={(e) => {
              if ((e.metaKey || e.ctrlKey) && e.key === "s") {
                e.preventDefault();
                save();
              }
              if (e.key === "Escape") e.stopPropagation();
            }}
            spellCheck={false}
          />
        </div>
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
