import { useEffect, useRef, useState } from "react";
import { Eye, Pencil, Save } from "lucide-react";
import * as ipc from "../lib/ipc";
import { highlightFile } from "../lib/highlight";
import Markdown from "./Markdown";
import type { OpenFile } from "../types";
import { t } from "../lib/i18n";

const isMarkdown = (rel: string) => /\.(md|markdown)$/i.test(rel);
const isCsv = (rel: string) => /\.(csv|tsv)$/i.test(rel);
/** Above this size, live re-highlighting on every keystroke gets slow:
 * fall back to a plain (uncolored) textarea. */
const HIGHLIGHT_EDIT_MAX = 120_000;
/** Rows rendered in the CSV table view; the rest stays behind the note. */
const CSV_MAX_ROWS = 500;

/** Minimal CSV parse: quoted fields, "" escapes, \r\n — no dependencies. */
function parseCsv(text: string, sep: string): string[][] {
  const rows: string[][] = [];
  let row: string[] = [];
  let field = "";
  let quoted = false;
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (quoted) {
      if (c === '"') {
        if (text[i + 1] === '"') {
          field += '"';
          i++;
        } else quoted = false;
      } else field += c;
    } else if (c === '"') quoted = true;
    else if (c === sep) {
      row.push(field);
      field = "";
    } else if (c === "\n") {
      row.push(field.replace(/\r$/, ""));
      rows.push(row);
      row = [];
      field = "";
    } else field += c;
  }
  if (field !== "" || row.length > 0) {
    row.push(field.replace(/\r$/, ""));
    rows.push(row);
  }
  return rows.filter((r) => r.length > 1 || (r[0] ?? "") !== "");
}

/** Pick the delimiter that splits the first line the most (pt-BR uses ;). */
function sniffSep(text: string, rel: string): string {
  if (/\.tsv$/i.test(rel)) return "\t";
  const first = text.slice(0, text.indexOf("\n") < 0 ? text.length : text.indexOf("\n"));
  const counts: [string, number][] = [",", ";", "\t"].map((s) => [
    s,
    first.split(s).length - 1,
  ]);
  counts.sort((a, b) => b[1] - a[1]);
  return counts[0][1] > 0 ? counts[0][0] : ",";
}

/** Data first: the table is the reading view, the pencil shows the source. */
function CsvTable({ text, rel }: { text: string; rel: string }) {
  const rows = parseCsv(text, sniffSep(text, rel));
  if (rows.length === 0) return <div className="viewer-note">∅</div>;
  const [header, ...body] = rows;
  const shown = body.slice(0, CSV_MAX_ROWS);
  return (
    <div className="viewer-csv">
      <table>
        <thead>
          <tr>
            {header.map((h, i) => (
              <th key={i}>{h}</th>
            ))}
          </tr>
        </thead>
        <tbody>
          {shown.map((r, i) => (
            <tr key={i}>
              {header.map((_, j) => (
                <td key={j} title={r[j]}>
                  {r[j] ?? ""}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
      {body.length > shown.length && (
        <div className="viewer-note">{t("more_lines", { n: body.length - shown.length })}</div>
      )}
    </div>
  );
}

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
        // Markdown and CSV are for reading first; the rest is for editing.
        setEditing(!isMarkdown(file.rel) && !isCsv(file.rel) && !out.truncated);
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
      ) : isCsv(file.rel) ? (
        <CsvTable text={content} rel={file.rel} />
      ) : (
        <pre className="viewer-code">
          <code dangerouslySetInnerHTML={{ __html: highlightFile(file.rel, content) }} />
        </pre>
      )}
    </div>
  );
}
