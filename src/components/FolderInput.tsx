import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
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
 *
 * The list floats over the page, anchored to the field like the composer's
 * `@` menu: in the flow it pushed the field around at every key, as the
 * number of folders changed.
 */
/** Where the floating list goes: under the field, or over it when the
 *  field sits at the bottom of the window (the rail's "+ project"). */
type Place = { left: number; width: number; top?: number; bottom?: number; maxHeight: number };

const GAP = 6;
const EDGE = 12;

function placeFor(field: DOMRect): Place {
  const below = window.innerHeight - field.bottom - GAP - EDGE;
  const above = field.top - GAP - EDGE;
  const up = below < 260 && above > below;
  // The rail is narrow and a path is long: the list may be wider than
  // the field, never wider than the window.
  const width = Math.min(Math.max(field.width, 360), window.innerWidth - 2 * EDGE);
  const left = Math.max(EDGE, Math.min(field.left, window.innerWidth - EDGE - width));
  return up
    ? { left, width, bottom: window.innerHeight - field.top + GAP, maxHeight: Math.max(120, above) }
    : { left, width, top: field.bottom + GAP, maxHeight: Math.max(120, below) };
}

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
  /** The text `done` answers. Until it catches up the list on screen is
   *  the one before: shown, so nothing blinks, but not acted on. */
  const [answered, setAnswered] = useState<string | null>(null);
  const [index, setIndex] = useState(-1);
  const [place, setPlace] = useState<Place | null>(null);
  const input = useRef<HTMLInputElement>(null);
  const asked = useRef(0);
  /** Enter came before the folders did: taken when they arrive. */
  const enterWaits = useRef(false);

  const take = (path: string | null) => {
    if (path) onPick(path);
    onClose();
  };
  const taker = useRef(take);
  taker.current = take;

  useEffect(() => {
    const mine = ++asked.current;
    const settle = (got: Completion) => {
      // An older answer arriving late would list folders for text that
      // is no longer in the field.
      if (mine !== asked.current) return;
      const at = startIndex(got, text);
      if (enterWaits.current) {
        enterWaits.current = false;
        taker.current(enterTarget(text, got, at));
        return;
      }
      setDone(got);
      setAnswered(text);
      setIndex(at);
    };
    const timer = setTimeout(() => {
      // Through a promise even when the call itself throws (no shell).
      Promise.resolve()
        .then(() => complete(text))
        .then(settle)
        .catch(() => settle(EMPTY));
    }, ASK_AFTER_MS);
    return () => clearTimeout(timer);
  }, [text, complete]);

  const fresh = answered === text;
  const none = fresh && done.hits.length === 0 && text.trim() !== "";
  const open = done.hits.length > 0 || none;

  // The list follows the field, which the rail's scroller or a resize
  // can move; the field itself is never moved by the list.
  useLayoutEffect(() => {
    if (!open) return;
    const follow = () => {
      const field = input.current?.getBoundingClientRect();
      if (field) setPlace(placeFor(field));
    };
    follow();
    window.addEventListener("resize", follow);
    window.addEventListener("scroll", follow, true);
    return () => {
      window.removeEventListener("resize", follow);
      window.removeEventListener("scroll", follow, true);
    };
  }, [open]);

  const into = (at: number) => {
    const next = descendText(done, at);
    if (next === null) return;
    setText(next);
    input.current?.focus();
  };

  return (
    <div className={`folder-pick${className ? ` ${className}` : ""}`}>
      <input
        ref={input}
        className="side-rename folder-field"
        role="combobox"
        aria-expanded={open}
        aria-controls="folder-list"
        aria-autocomplete="list"
        autoFocus
        spellCheck={false}
        autoCapitalize="off"
        autoCorrect="off"
        placeholder={t("folder_placeholder")}
        value={text}
        onChange={(e) => {
          enterWaits.current = false;
          setText(e.target.value);
        }}
        onBlur={onClose}
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            e.stopPropagation();
            onClose();
          } else if (e.key === "Enter") {
            e.preventDefault();
            // The folders on screen answer the text before: Enter waits
            // for the ones that answer this text.
            if (fresh) take(enterTarget(text, done, index));
            else enterWaits.current = true;
          } else if (e.key === "Tab") {
            // Tab stays in the field while there is a folder to go into.
            if (!fresh || done.hits.length === 0) return;
            e.preventDefault();
            into(index);
          } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
            if (done.hits.length === 0) return;
            e.preventDefault();
            setIndex(moveIndex(index, done.hits.length, e.key === "ArrowDown" ? 1 : -1));
          }
        }}
      />
      {open &&
        createPortal(
          // A press on the list must not blur the field: blur closes it.
          <div
            id="folder-list"
            role="listbox"
            className="folder-pop"
            style={place ?? { visibility: "hidden" }}
            onMouseDown={(e) => e.preventDefault()}
          >
            {done.hits.map((hit, i) => (
              <div
                key={hit.path}
                role="option"
                aria-selected={i === index}
                className={`folder-row${i === index ? " sel" : ""}`}
                // Movement, not presence: a list that opens under a resting
                // pointer must not move the highlight Enter is about to take.
                onMouseMove={() => i !== index && setIndex(i)}
                onClick={() => take(hit.path)}
              >
                <span className={`folder-icon${hit.git ? " git" : ""}`}>
                  {hit.git ? <FolderGit2 size={14} /> : <Folder size={14} />}
                </span>
                <span className="folder-name">{hit.name}</span>
                {hit.known && <span className="mm-tag">{t("folder_known")}</span>}
                <span className="folder-path">{hit.shown}</span>
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
            {none && <div className="folder-note">{t("folder_none")}</div>}
            {done.hits.length > 0 && <div className="folder-note keys">{t("folder_keys")}</div>}
          </div>,
          document.body,
        )}
    </div>
  );
}
