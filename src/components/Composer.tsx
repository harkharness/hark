import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { Mic, SendHorizontal, X } from "lucide-react";
import * as ipc from "../lib/ipc";
import { t } from "../lib/i18n";
import type { Project } from "../types";
import {
  type Attachment,
  fenceSegments,
  groupBlocks,
  hasRich,
  exitFence,
  insideOpenFence,
  openFencePair,
  paint,
  wantsExit,
} from "../lib/composerText";


type Mention = {
  /** Char index of the "@" in the textarea value. */
  at: number;
  query: string;
  hits: { project: Project; rel: string }[];
  sel: number;
};

type SlashHit = { name: string; desc?: string };
type Slash = { hits: SlashHit[]; sel: number };

/** What you sent, oldest first — the up arrow walks back through it like
 *  a shell. Per surface (the thread's project, or the mother), kept in
 *  localStorage so closing the window does not forget the last hour. */
const HISTORY_MAX = 50;

function readHistory(scope: string): string[] {
  try {
    const raw = localStorage.getItem(`hark.composer.history.${scope}`);
    const all = raw ? JSON.parse(raw) : [];
    return Array.isArray(all) ? all.filter((l): l is string => typeof l === "string") : [];
  } catch {
    // Private windows and blocked site data throw on read: no history
    // is a fine composer, a crashed one is not.
    return [];
  }
}

function pushHistory(scope: string, line: string): string[] {
  const all = readHistory(scope);
  // Sending the same thing twice adds one entry, like every shell.
  const next = all[all.length - 1] === line ? all : [...all, line].slice(-HISTORY_MAX);
  try {
    localStorage.setItem(`hark.composer.history.${scope}`, JSON.stringify(next));
  } catch {
    /* nothing to do: the in-memory list still walks this session */
  }
  return next;
}

/** Commands Hark resolves itself; everything else in the palette is the
 *  CLI's own list (from the session's init event) sent verbatim. */
const NATIVE_SLASH: SlashHit[] = [
  { name: "modo", desc: "modo de permissão: manual · edições · plano · auto · ignorar" },
  { name: "rename", desc: "renomeia o chat focado" },
  { name: "board", desc: "abre/fecha o quadro" },
  { name: "usage", desc: "uso da sessão, últimas 24h e limites da assinatura" },
];

/**
 * The input bar: a real text editor for a voice-first app. Enter sends,
 * Shift+Enter breaks the line, Cmd+V pastes screenshots (thumbnails above
 * the text, "[image N]" written into it — no more "no primeiro print"),
 * and "@" opens a fuzzy file autocomplete (local, free).
 */
export default function Composer({
  disabled,
  recording,
  placeholder,
  projects,
  activeProject,
  pendingPermissionId,
  onSubmit,
  onMic,
  onAnswerPermission,
  children,
  trailing,
  banner,
  autoFocus,
}: {
  disabled: boolean;
  recording: boolean;
  placeholder: string;
  projects: Project[];
  /** Project the thread lives in; scopes @mentions (falls back to all). */
  activeProject?: Project;
  pendingPermissionId?: string;
  onSubmit: (text: string, images: Attachment[]) => void;
  onMic: () => void;
  onAnswerPermission: (requestId: string, allow: boolean, always?: boolean) => void;
  children?: React.ReactNode;
  /** Window controls docked at the right of the control row (costs,
   *  board, terminal, volume…) — the topbar is gone. */
  trailing?: React.ReactNode;
  /** A readout pinned above the text, inside the card (the repo ruler). */
  banner?: React.ReactNode;
  /** Focus the field on mount (the mother's expanded chat opens to type). */
  autoFocus?: boolean;
}) {
  const [text, setText] = useState("");
  /** Caret offset — the mirror reveals markers on the caret's line. */
  const [caret, setCaret] = useState(0);
  const [images, setImages] = useState<Attachment[]>([]);
  const [mention, setMention] = useState<Mention | null>(null);
  const [slash, setSlash] = useState<Slash | null>(null);
  const areaRef = useRef<HTMLTextAreaElement>(null);
  const mirrorRef = useRef<HTMLDivElement>(null);
  const debounceRef = useRef<number>(0);
  /** Where the up arrow is standing in the sent history (null = typing). */
  const [histAt, setHistAt] = useState<number | null>(null);
  const histRef = useRef<string[]>([]);
  /** What was in the box when history navigation started, handed back on
   *  the way down past the newest entry. */
  const draftRef = useRef("");
  const histScope = activeProject?.path ?? "hark";
  /** CLI slash list, fetched once per composer (cheap local lookup). */
  const cliSlash = useRef<string[] | null>(null);

  function autoGrow() {
    const el = areaRef.current;
    if (!el) return;
    el.style.height = "auto";
    // Input-sized at rest; grows to FIVE lines max, then scrolls inside.
    el.style.height = `${Math.min(el.scrollHeight, 120)}px`;
  }

  /** The block box is a measured rectangle behind the text: full width,
   *  covering the (hidden) fence lines too — the text layer itself never
   *  changes, so the caret stays honest by construction. */
  function measureBlocks() {
    const m = mirrorRef.current;
    if (!m) return;
    const mr = m.getBoundingClientRect();
    const segs = m.querySelectorAll<HTMLElement>(".cm-fseg");
    m.querySelectorAll<HTMLElement>(".cm-fblock").forEach((box, i) => {
      const seg = segs[i];
      if (!seg) {
        box.style.display = "none";
        return;
      }
      const r = seg.getBoundingClientRect();
      // The fence lines keep their full line height — they are real text
      // the caret walks through — but the BOX need not claim them: it
      // pulls in half a line at each end, so what is left of the fence
      // reads as the block's own breathing room instead of padding the
      // box out to three lines for one line of code.
      const inset = Math.min(10, r.height / 4);
      box.style.display = "block";
      box.style.top = `${r.top - mr.top + m.scrollTop + inset}px`;
      box.style.height = `${r.height - inset * 2}px`;
    });
  }

  /** "@quer" right before the caret means the autocomplete is active. */
  function detectMention(value: string, caret: number) {
    const before = value.slice(0, caret);
    const match = before.match(/@([\w./~-]*)$/);
    if (!match) {
      setMention(null);
      return;
    }
    const at = caret - match[0].length;
    const query = match[1];
    window.clearTimeout(debounceRef.current);
    debounceRef.current = window.setTimeout(async () => {
      const scope = activeProject ? [activeProject] : projects;
      const results = await Promise.all(
        scope.map((project) =>
          ipc
            .projectFiles(project.path, query, activeProject ? 8 : 4)
            .then((rels) => rels.map((rel) => ({ project, rel })))
            .catch(() => []),
        ),
      );
      const hits = results.flat().slice(0, 8);
      setMention(hits.length > 0 ? { at, query, hits, sel: 0 } : null);
    }, 120);
  }

  /** The palette is live while the text is just "/name" (no args yet). */
  async function detectSlash(value: string) {
    const match = value.match(/^\/([\w:-]*)$/);
    if (!match) {
      setSlash(null);
      return;
    }
    if (!cliSlash.current) {
      cliSlash.current = await ipc
        .slashCommands(activeProject?.path)
        .catch(() => [] as string[]);
    }
    const q = match[1].toLowerCase();
    const taken = new Set(NATIVE_SLASH.map((c) => c.name));
    const cli = (cliSlash.current ?? [])
      .filter((n) => !taken.has(n) && n.toLowerCase().includes(q))
      .sort(
        (a, b) =>
          Number(b.toLowerCase().startsWith(q)) - Number(a.toLowerCase().startsWith(q)) ||
          a.localeCompare(b),
      );
    const hits = [
      ...NATIVE_SLASH.filter((c) => c.name.startsWith(q)),
      ...cli.map((name) => ({ name })),
    ].slice(0, 10);
    setSlash(hits.length > 0 ? { hits, sel: 0 } : null);
  }

  function pickSlash(hit: SlashHit) {
    setText(`/${hit.name} `);
    setSlash(null);
    requestAnimationFrame(() => areaRef.current?.focus());
  }

  function pickMention(hit: { project: Project; rel: string }) {
    if (!mention) return;
    // Inside the thread's own project a relative path is enough (the
    // worker runs there); otherwise spell out the full path.
    const inserted =
      activeProject && hit.project.path === activeProject.path
        ? hit.rel
        : `${hit.project.path}/${hit.rel}`;
    const caret = areaRef.current?.selectionStart ?? text.length;
    const next = `${text.slice(0, mention.at)}@${inserted} ${text.slice(caret)}`;
    setText(next);
    setMention(null);
    requestAnimationFrame(() => {
      const el = areaRef.current;
      if (el) {
        const pos = mention.at + inserted.length + 2;
        el.focus();
        el.setSelectionRange(pos, pos);
        setCaret(pos);
      }
    });
  }

  /** Replace the whole box and park the caret at the end (history recall,
   *  which should read as "this is what you typed", ready to edit). */
  function replaceText(value: string) {
    setText(value);
    setCaret(value.length);
    requestAnimationFrame(() => {
      const el = areaRef.current;
      if (!el) return;
      el.focus();
      el.setSelectionRange(value.length, value.length);
      autoGrow();
    });
  }

  /** Apply a computed edit and park the caret exactly where it says. */
  function applyEdit(next: { text: string; caret: number }) {
    setText(next.text);
    setCaret(next.caret);
    requestAnimationFrame(() => {
      const el = areaRef.current;
      if (!el) return;
      el.focus();
      el.setSelectionRange(next.caret, next.caret);
      autoGrow();
    });
  }

  /** Walk the sent history: -1 is older, +1 newer. Returns false when
   *  there is nowhere to go, so the arrow keeps its normal job. */
  function recall(dir: -1 | 1): boolean {
    if (histAt === null) {
      if (dir === 1) return false;
      // Re-read on entry: another window in this project may have sent
      // something since this composer mounted.
      histRef.current = readHistory(histScope);
      if (histRef.current.length === 0) return false;
      draftRef.current = text;
      setHistAt(histRef.current.length - 1);
      replaceText(histRef.current[histRef.current.length - 1]);
      return true;
    }
    const next = histAt + dir;
    if (next < 0) return true; // already at the oldest: stay, don't fall through
    if (next >= histRef.current.length) {
      // Past the newest entry the draft comes back, exactly as left.
      setHistAt(null);
      replaceText(draftRef.current);
      return true;
    }
    setHistAt(next);
    replaceText(histRef.current[next]);
    return true;
  }

  function send() {
    if (!text.trim() || disabled) return;
    const t = text;
    const imgs = images;
    histRef.current = pushHistory(histScope, t);
    setHistAt(null);
    draftRef.current = "";
    setText("");
    setCaret(0);
    setImages([]);
    setMention(null);
    setSlash(null);
    requestAnimationFrame(autoGrow);
    onSubmit(t, imgs);
  }

  /** Paste a screenshot: thumbnail up top, "[image N]" written at caret.
   *  Paste a URL OVER selected text: the selection becomes a markdown
   *  link instead of being erased — the label survives, the url hides. */
  function onPaste(e: React.ClipboardEvent) {
    const el = areaRef.current;
    const pasted = e.clipboardData.getData("text/plain").trim();
    const from = el?.selectionStart ?? 0;
    const to = el?.selectionEnd ?? 0;
    if (el && to > from && /^https?:\/\/\S+$/.test(pasted)) {
      e.preventDefault();
      const label = text.slice(from, to);
      const linked = `[${label}](${pasted})`;
      setText(`${text.slice(0, from)}${linked}${text.slice(to)}`);
      const pos = from + linked.length;
      setCaret(pos);
      requestAnimationFrame(() => {
        el.focus();
        el.setSelectionRange(pos, pos);
      });
      return;
    }
    const item = Array.from(e.clipboardData.items).find((i) => i.type.startsWith("image/"));
    if (!item) return;
    const file = item.getAsFile();
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => {
      const dataUrl = reader.result as string;
      setImages((old) => {
        const ref = `[image ${old.length + 1}]`;
        const el = areaRef.current;
        const caret = el?.selectionStart ?? text.length;
        setText((t) => {
          const glueL = t.slice(0, caret).match(/\s$|^$/) ? "" : " ";
          return `${t.slice(0, caret)}${glueL}${ref} ${t.slice(caret)}`;
        });
        return [...old, { dataUrl }];
      });
    };
    reader.readAsDataURL(file);
  }

  /** Removing a thumb renumbers the remaining references in the text. */
  function removeImage(index: number) {
    setImages((old) => old.filter((_, i) => i !== index));
    setText((t) => {
      let out = t.replace(new RegExp(`\\[image ${index + 1}\\] ?`, "g"), "");
      for (let n = index + 2; n <= images.length; n++) {
        out = out.replace(new RegExp(`\\[image ${n}\\]`, "g"), `[image ${n - 1}]`);
      }
      return out;
    });
  }

  function onKeyDown(e: React.KeyboardEvent<HTMLTextAreaElement>) {
    if (slash) {
      if (e.key === "ArrowDown" || e.key === "ArrowUp") {
        e.preventDefault();
        const delta = e.key === "ArrowDown" ? 1 : -1;
        setSlash({
          ...slash,
          sel: (slash.sel + delta + slash.hits.length) % slash.hits.length,
        });
        return;
      }
      if (e.key === "Enter" || e.key === "Tab") {
        e.preventDefault();
        pickSlash(slash.hits[slash.sel]);
        return;
      }
      if (e.key === "Escape") {
        setSlash(null);
        return;
      }
    }
    if (mention) {
      if (e.key === "ArrowDown" || e.key === "ArrowUp") {
        e.preventDefault();
        const delta = e.key === "ArrowDown" ? 1 : -1;
        setMention({
          ...mention,
          sel: (mention.sel + delta + mention.hits.length) % mention.hits.length,
        });
        return;
      }
      if (e.key === "Enter" || e.key === "Tab") {
        e.preventDefault();
        pickMention(mention.hits[mention.sel]);
        return;
      }
      if (e.key === "Escape") {
        setMention(null);
        return;
      }
    }
    // Down arrow inside a block is the way OUT when there is nothing
    // useful below: the last line of the draft, or a closing fence that
    // is itself the last line (invisible, so "down" looked like nothing
    // happened and the next keystroke landed inside the marker).
    if (e.key === "ArrowDown" && !e.altKey && !e.metaKey) {
      const pos = areaRef.current?.selectionStart ?? 0;
      if (wantsExit(text, pos)) {
        e.preventDefault();
        applyEdit(exitFence(text, pos));
        return;
      }
    }
    // Up arrow on the first line recalls what you sent, like a shell;
    // down walks back and hands the draft over past the newest entry.
    // Multi-line editing keeps the arrows: only a caret with no newline
    // behind it (above) or ahead of it (below) is a history request.
    if ((e.key === "ArrowUp" || e.key === "ArrowDown") && !e.altKey && !e.metaKey) {
      const pos = areaRef.current?.selectionStart ?? 0;
      const asks =
        e.key === "ArrowUp"
          ? !text.slice(0, pos).includes("\n")
          : histAt !== null && !text.slice(pos).includes("\n");
      if (asks && recall(e.key === "ArrowUp" ? -1 : 1)) {
        e.preventDefault();
        return;
      }
    }
    // Empty input + pending permission: y/n/a decide it, like a terminal.
    if (!text && pendingPermissionId && (e.key === "y" || e.key === "n" || e.key === "a")) {
      e.preventDefault();
      onAnswerPermission(pendingPermissionId, e.key !== "n", e.key === "a");
      return;
    }
    // "```" then SPACE opens a fenced block with the cursor inside. Enter
    // is deliberately not a trigger any more: enter means send, in the
    // block and out of it, and a key that sometimes sends and sometimes
    // opens a box is the kind of thing nobody can predict.
    if (e.key === " ") {
      const el = areaRef.current;
      const pos = el?.selectionStart ?? 0;
      const before = text.slice(0, pos);
      const fence = before.match(/(^|\n)```([\w+-]{0,12})$/);
      // Inside an open block a typed ``` is the CLOSER: let the plain
      // Enter land and the parser see the close — auto-pairing here was
      // the "block keeps reopening" loop the user could never leave.
      if (el && fence && pos === (el.selectionEnd ?? pos) && !insideOpenFence(text, pos)) {
        e.preventDefault();
        // Land between the fences on the next paint.
        applyEdit(openFencePair(text, pos));
        return;
      }
    }
    // One rule everywhere, block or prose: Enter sends, Shift+Enter
    // breaks the line. Making Enter mean something else inside a block
    // was a second rule to learn for the sake of one context.
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      send();
    }
  }

  useEffect(autoGrow, [text]);

  const blockCount = useMemo(
    () => fenceSegments(text).filter((s) => s.fenced).length,
    [text],
  );
  useLayoutEffect(measureBlocks, [text, blockCount]);

  // First paint happens before the panels settle their widths, so the
  // placeholder wraps and the measured height sticks too tall. Re-measure
  // whenever the box actually changes size (mount, sidebar/rail toggles).
  useEffect(() => {
    const el = areaRef.current;
    if (!el) return;
    const ro = new ResizeObserver(() => {
      autoGrow();
      measureBlocks();
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  // Claude Code layout: one rounded box (thumbnails + text), controls on
  // a slim row underneath. Starts input-sized; grows as you type.
  return (
    <div className="inputbar">
      <div className={`composer ${images.length > 0 ? "with-thumbs" : ""}`}>
        {banner}
        {slash && (
          <div className="mention-pop slash-pop">
            {slash.hits.map((hit, i) => (
              <button
                key={hit.name}
                className={i === slash.sel ? "sel" : ""}
                onMouseEnter={() => setSlash({ ...slash, sel: i })}
                onClick={() => pickSlash(hit)}
              >
                <span className="slash-name">/{hit.name}</span>
                <span className="slash-desc">{hit.desc ?? "comando do Claude Code"}</span>
              </button>
            ))}
          </div>
        )}
        {mention && (
          <div className="mention-pop">
            {mention.hits.map((hit, i) => (
              <button
                key={`${hit.project.name}/${hit.rel}`}
                className={i === mention.sel ? "sel" : ""}
                onMouseEnter={() => setMention({ ...mention, sel: i })}
                onClick={() => pickMention(hit)}
              >
                <span className="mention-proj">{hit.project.name}/</span>
                {hit.rel}
              </button>
            ))}
          </div>
        )}
        {images.length > 0 && (
          <div className="composer-thumbs">
            {images.map((img, i) => (
              <span key={i} className="thumb" title={`[image ${i + 1}]`}>
                <img src={img.dataUrl} alt={`image ${i + 1}`} />
                <i>{i + 1}</i>
                <button onClick={() => removeImage(i)} title="remover">
                  <X size={10} />
                </button>
              </span>
            ))}
          </div>
        )}
        {/* The mirror is positioned against THIS box, not the composer:
            with thumbnails attached the textarea no longer starts at the
            composer's top edge, and inset:0 would offset every line. */}
        <div className="composer-field">
        {/* A textarea cannot style part of its own text, so the code
            block is painted by a mirror behind it: identical font,
            padding and wrapping, fenced regions boxed. The textarea
            itself renders transparent glyphs and keeps the caret, the
            selection and every native editing behaviour. */}
        <div
          className="composer-mirror"
          aria-hidden="true"
          ref={mirrorRef}
        >
          {Array.from({ length: blockCount }, (_, i) => (
            <div
              key={`fb${i}`}
              className={`cm-fblock ${insideOpenFence(text, caret) ? "in" : ""}`}
            />
          ))}
          {groupBlocks(paint(text, caret)).map((g, i) =>
            g.fid !== undefined ? (
              <span key={i} className="cm-fseg" data-fid={g.fid}>
                {g.parts.map((p, j) =>
                  p.cls ? (
                    <span key={j} className={p.cls}>
                      {p.text}
                    </span>
                  ) : (
                    p.text
                  ),
                )}
              </span>
            ) : (
              g.parts.map((p, j) =>
                p.cls ? (
                  <span key={`${i}-${j}`} className={p.cls}>
                    {p.text}
                  </span>
                ) : (
                  p.text
                ),
              )
            ),
          )}
          {/* Trailing newline needs a glyph or the mirror ends short. */}
          {text.endsWith("\n") ? " " : ""}
        </div>
        <textarea
          ref={areaRef}
          className={hasRich(text) ? "has-rich" : ""}
          rows={1}
          autoFocus={autoFocus}
          placeholder={placeholder}
          value={text}
          onChange={(e) => {
            // Any real keystroke ends history navigation: otherwise a
            // later down-arrow would swap the edited text for the draft.
            if (histAt !== null) setHistAt(null);
            setText(e.target.value);
            setCaret(e.target.selectionStart ?? e.target.value.length);
            detectMention(e.target.value, e.target.selectionStart ?? e.target.value.length);
            detectSlash(e.target.value);
          }}
          onSelect={(e) => setCaret(e.currentTarget.selectionStart ?? 0)}
          onScroll={(e) => {
            const m = mirrorRef.current;
            if (m) m.scrollTop = e.currentTarget.scrollTop;
          }}
          onPaste={onPaste}
          onKeyDown={onKeyDown}
          disabled={disabled}
        />
        </div>
        {/* Controls live INSIDE the card (Claude Code grammar): one
            surface holds the text and its row of pills and circles. */}
        <div className="inputbar-row">
        <div className="inputbar-chips">{children}</div>
        {trailing}
        <button className={`mic ${recording ? "recording" : ""}`} onClick={onMic} title={t("speak_btn")}>
          <Mic size={15} />
        </button>
        <button className="send" onClick={send} disabled={disabled} title={t("send_btn")}>
          <SendHorizontal size={15} />
        </button>
        </div>
      </div>
    </div>
  );
}
