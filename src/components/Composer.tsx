import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { Mic, SendHorizontal, X } from "lucide-react";
import * as ipc from "../lib/ipc";
import { t } from "../lib/i18n";
import type { Project } from "../types";
import {
  type Attachment,
  fenceSegments,
  groupBlocks,
  hasFence,
  hasRich,
  insideOpenFence,
  paint,
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
      box.style.display = "block";
      box.style.top = `${r.top - mr.top + m.scrollTop - 3}px`;
      box.style.height = `${r.height + 6}px`;
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

  function send() {
    if (!text.trim() || disabled) return;
    const t = text;
    const imgs = images;
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
    // Empty input + pending permission: y/n/a decide it, like a terminal.
    if (!text && pendingPermissionId && (e.key === "y" || e.key === "n" || e.key === "a")) {
      e.preventDefault();
      onAnswerPermission(pendingPermissionId, e.key !== "n", e.key === "a");
      return;
    }
    // "```" then space or Enter opens a fenced block, cursor inside, the
    // way every chat input the user already types in behaves. Without it
    // Enter just sent three backticks as a message.
    if (e.key === "Enter" || e.key === " ") {
      const el = areaRef.current;
      const pos = el?.selectionStart ?? 0;
      const before = text.slice(0, pos);
      const fence = before.match(/(^|\n)```([\w+-]{0,12})$/);
      // Inside an open block a typed ``` is the CLOSER: let the plain
      // Enter land and the parser see the close — auto-pairing here was
      // the "block keeps reopening" loop the user could never leave.
      if (el && fence && pos === (el.selectionEnd ?? pos) && !insideOpenFence(text, pos)) {
        e.preventDefault();
        const after = text.slice(pos);
        const opened = `${before}\n`;
        setText(`${opened}\n\`\`\`${after}`);
        // Land between the fences on the next paint.
        const at = opened.length;
        setCaret(at);
        requestAnimationFrame(() => {
          el.selectionStart = at;
          el.selectionEnd = at;
          el.focus();
        });
        return;
      }
    }
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
          className={`composer-mirror ${hasFence(text) ? "has-fence" : ""}`}
          aria-hidden="true"
          ref={mirrorRef}
        >
          {Array.from({ length: blockCount }, (_, i) => (
            <div key={`fb${i}`} className="cm-fblock" />
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
          className={`${hasFence(text) ? "has-fence" : ""} ${hasRich(text) ? "has-rich" : ""}`}
          rows={1}
          autoFocus={autoFocus}
          placeholder={placeholder}
          value={text}
          onChange={(e) => {
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
      </div>
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
  );
}
