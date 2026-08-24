import { useEffect, useRef, useState } from "react";
import { Mic, SendHorizontal, X } from "lucide-react";
import * as ipc from "../lib/ipc";
import { t } from "../lib/i18n";
import type { Project } from "../types";

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
];

/** A pasted screenshot: thumbnail on top, "[image N]" reference in prose. */
export type Attachment = { dataUrl: string };

/** data URL → (media_type, base64) pair the backend expects. */
export function toImagePair(a: Attachment): [string, string] {
  return [a.dataUrl.slice(5, a.dataUrl.indexOf(";")), a.dataUrl.split(",")[1]];
}

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
}) {
  const [text, setText] = useState("");
  const [images, setImages] = useState<Attachment[]>([]);
  const [mention, setMention] = useState<Mention | null>(null);
  const [slash, setSlash] = useState<Slash | null>(null);
  const areaRef = useRef<HTMLTextAreaElement>(null);
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
      }
    });
  }

  function send() {
    if (!text.trim() || disabled) return;
    const t = text;
    const imgs = images;
    setText("");
    setImages([]);
    setMention(null);
    setSlash(null);
    requestAnimationFrame(autoGrow);
    onSubmit(t, imgs);
  }

  /** Paste a screenshot: thumbnail up top, "[image N]" written at caret. */
  function onPaste(e: React.ClipboardEvent) {
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
      const caret = el?.selectionStart ?? 0;
      const before = text.slice(0, caret);
      const fence = before.match(/(^|\n)```([a-zA-Z0-9+-]*)$/);
      if (el && fence && caret === (el.selectionEnd ?? caret)) {
        e.preventDefault();
        const after = text.slice(caret);
        const opened = `${before}\n`;
        setText(`${opened}\n\`\`\`${after}`);
        // Land between the fences on the next paint.
        const at = opened.length;
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

  // First paint happens before the panels settle their widths, so the
  // placeholder wraps and the measured height sticks too tall. Re-measure
  // whenever the box actually changes size (mount, sidebar/rail toggles).
  useEffect(() => {
    const el = areaRef.current;
    if (!el) return;
    const ro = new ResizeObserver(autoGrow);
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
        <textarea
          ref={areaRef}
          rows={1}
          placeholder={placeholder}
          value={text}
          onChange={(e) => {
            setText(e.target.value);
            detectMention(e.target.value, e.target.selectionStart ?? e.target.value.length);
            detectSlash(e.target.value);
          }}
          onPaste={onPaste}
          onKeyDown={onKeyDown}
          disabled={disabled}
        />
      </div>
      <div className="inputbar-row">
        <div className="inputbar-chips">{children}</div>
        {trailing}
        <button className={`mic ${recording ? "recording" : ""}`} onClick={onMic} title={t("speak_btn")}>
          <Mic size={15} />
        </button>
        <button onClick={send} disabled={disabled} title={t("send_btn")}>
          <SendHorizontal size={15} />
        </button>
      </div>
    </div>
  );
}
