import { useEffect, useRef, useState } from "react";
import { ChevronRight, Folder, FolderGit2 } from "lucide-react";
import * as ipc from "../lib/ipc";
import { t } from "../lib/i18n";
import { descendText, EMPTY, enterTarget, moveIndex, startIndex, type Completion } from "../lib/folderPick";

/** How long the field waits after a key before asking for folders. */
const ASK_AFTER_MS = 60;

/**
 * The field a folder becomes a project through. Nobody remembers a
 * project's whole directory, so it lists folders as the text grows: the
 * ones under a typed path, or the ones a bare name matches near the
 * projects already registered. Enter takes one, Tab goes into it.
 */
export default function FolderInput({
  className,
  onPick,
  onClose,
  complete = ipc.folderComplete,
}: {
  className?: string;
  /** The folder to register: absolute from a row, as typed otherwise. */
  onPick: (path: string) => void;
  onClose: () => void;
  /** The folders for a typed text; the backend's, unless a test brings its own. */
  complete?: (typed: string) => Promise<Completion>;
}) {
  const [text, setText] = useState("");
  const [done, setDone] = useState<Completion>(EMPTY);
  /** The text `done` answers: until it catches up, nothing is highlighted. */
  const [answered, setAnswered] = useState<string | null>(null);
  const [index, setIndex] = useState(-1);
  const input = useRef<HTMLInputElement>(null);
  const asked = useRef(0);

  useEffect(() => {
    const mine = ++asked.current;
    const timer = setTimeout(() => {
      // Through a promise even when the call itself throws (no shell).
      Promise.resolve()
        .then(() => complete(text))
        .then((got) => {
          // An older answer arriving late would list folders for text
          // that is no longer in the field.
          if (mine !== asked.current) return;
          setDone(got);
          setAnswered(text);
          setIndex(startIndex(got, text));
        })
        .catch(() => {
          if (mine !== asked.current) return;
          setDone(EMPTY);
          setAnswered(text);
          setIndex(-1);
        });
    }, ASK_AFTER_MS);
    return () => clearTimeout(timer);
  }, [text, complete]);

  // In the rail the field is the last thing in a scroller: a list that
  // grew below the fold would be folders nobody sees.
  const box = useRef<HTMLDivElement>(null);
  const rows = answered === text ? done.hits.length : 0;
  useEffect(() => {
    box.current?.scrollIntoView?.({ block: "nearest" });
  }, [rows, answered]);

  // The list answers the text in the field, or it is not acted on: Enter
  // right after a key must not take a folder listed for the text before.
  const fresh = answered === text;
  const shown = fresh ? done : EMPTY;
  const cursor = fresh ? index : -1;

  const take = (path: string | null) => {
    if (path) onPick(path);
    onClose();
  };
  const into = (at: number) => {
    const next = descendText(shown, at);
    if (next === null) return;
    setText(next);
    input.current?.focus();
  };

  return (
    <div ref={box} className={`folder-pick${className ? ` ${className}` : ""}`}>
      <input
        ref={input}
        className="side-rename folder-field"
        role="combobox"
        aria-expanded={shown.hits.length > 0}
        aria-controls="folder-list"
        aria-autocomplete="list"
        autoFocus
        spellCheck={false}
        autoCapitalize="off"
        autoCorrect="off"
        placeholder={t("folder_placeholder")}
        value={text}
        onChange={(e) => setText(e.target.value)}
        onBlur={onClose}
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            e.stopPropagation();
            onClose();
          } else if (e.key === "Enter") {
            e.preventDefault();
            take(enterTarget(text, shown, cursor));
          } else if (e.key === "Tab") {
            // Tab stays in the field while there is a folder to go into.
            if (shown.hits.length === 0) return;
            e.preventDefault();
            into(cursor);
          } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
            if (shown.hits.length === 0) return;
            e.preventDefault();
            setIndex(moveIndex(cursor, shown.hits.length, e.key === "ArrowDown" ? 1 : -1));
          }
        }}
      />
      {/* A press on the list must not blur the field: blur closes it. */}
      <div id="folder-list" role="listbox" className="folder-list" onMouseDown={(e) => e.preventDefault()}>
        {shown.hits.map((hit, i) => (
          <div
            key={hit.path}
            role="option"
            aria-selected={i === cursor}
            className={`mm-row folder-row${i === cursor ? " cursor" : ""}`}
            // Movement, not presence: a list that opens under a resting
            // pointer must not move the highlight Enter is about to take.
            onMouseMove={() => i !== cursor && setIndex(i)}
            onClick={() => take(hit.path)}
          >
            <span className={`folder-icon${hit.git ? " git" : ""}`}>
              {hit.git ? <FolderGit2 size={14} /> : <Folder size={14} />}
            </span>
            <span className="mm-text">
              <span className="mm-name">
                {hit.name}
                {hit.known && <span className="mm-tag">{t("folder_known")}</span>}
              </span>
              <span className="mm-hint">{hit.shown}</span>
            </span>
            <button
              type="button"
              className="folder-into"
              title={t("folder_into")}
              aria-label={t("folder_into")}
              tabIndex={-1}
              onClick={(e) => {
                e.stopPropagation();
                into(i);
              }}
            >
              <ChevronRight size={14} />
            </button>
          </div>
        ))}
        {fresh && shown.hits.length === 0 && text.trim() !== "" && (
          <div className="folder-note">{t("folder_none")}</div>
        )}
        {shown.hits.length > 0 && <div className="folder-note keys">{t("folder_keys")}</div>}
      </div>
    </div>
  );
}
