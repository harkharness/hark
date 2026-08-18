import { useEffect, useRef, useState } from "react";
import { Mic, SendHorizontal } from "lucide-react";
import * as ipc from "../lib/ipc";
import type { Project } from "../types";

type Mention = {
  /** Char index of the "@" in the textarea value. */
  at: number;
  query: string;
  hits: { project: Project; rel: string }[];
  sel: number;
};

/**
 * The input bar: a real text editor for a voice-first app. Enter sends,
 * Shift+Enter breaks the line, Cmd+V pastes screenshots, and typing "@"
 * opens a fuzzy file autocomplete over the project files (local, free).
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
}: {
  disabled: boolean;
  recording: boolean;
  placeholder: string;
  projects: Project[];
  /** Project the thread lives in; scopes @mentions (falls back to all). */
  activeProject?: Project;
  pendingPermissionId?: string;
  onSubmit: (text: string, image: string | null) => void;
  onMic: () => void;
  onAnswerPermission: (requestId: string, allow: boolean) => void;
  children?: React.ReactNode;
}) {
  const [text, setText] = useState("");
  const [image, setImage] = useState<string | null>(null);
  const [mention, setMention] = useState<Mention | null>(null);
  const areaRef = useRef<HTMLTextAreaElement>(null);
  const debounceRef = useRef<number>(0);

  function autoGrow() {
    const el = areaRef.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, Math.round(window.innerHeight * 0.35))}px`;
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
    const img = image;
    setText("");
    setImage(null);
    setMention(null);
    requestAnimationFrame(autoGrow);
    onSubmit(t, img);
  }

  function onPaste(e: React.ClipboardEvent) {
    const item = Array.from(e.clipboardData.items).find((i) => i.type.startsWith("image/"));
    if (!item) return;
    const file = item.getAsFile();
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => setImage(reader.result as string);
    reader.readAsDataURL(file);
  }

  function onKeyDown(e: React.KeyboardEvent<HTMLTextAreaElement>) {
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
    // Empty input + pending permission: y/n decide it, like a terminal.
    if (!text && pendingPermissionId && (e.key === "y" || e.key === "n")) {
      e.preventDefault();
      onAnswerPermission(pendingPermissionId, e.key === "y");
      return;
    }
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      send();
    }
  }

  useEffect(autoGrow, [text]);

  // Claude Code layout: the textarea owns the full width; chips and the
  // mic/send controls live on a slim row underneath.
  return (
    <div className="inputbar">
      {image && (
        <span className="thumb">
          <img src={image} alt="attachment" />
          <button onClick={() => setImage(null)}>×</button>
        </span>
      )}
      <div className="composer">
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
        <textarea
          ref={areaRef}
          rows={2}
          placeholder={placeholder}
          value={text}
          onChange={(e) => {
            setText(e.target.value);
            detectMention(e.target.value, e.target.selectionStart ?? e.target.value.length);
          }}
          onPaste={onPaste}
          onKeyDown={onKeyDown}
          disabled={disabled}
        />
      </div>
      <div className="inputbar-row">
        <div className="inputbar-chips">{children}</div>
        <button className={`mic ${recording ? "recording" : ""}`} onClick={onMic} title="falar">
          <Mic size={15} />
        </button>
        <button onClick={send} disabled={disabled} title="enviar (Enter)">
          <SendHorizontal size={15} />
        </button>
      </div>
    </div>
  );
}
