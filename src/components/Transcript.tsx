import { useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import { Check, Copy, Lock, Square, Volume2 } from "lucide-react";
import Markdown from "./Markdown";
import ToolCall, { ToolOutput } from "./ToolCall";
import { directiveLabels, shortModel } from "../lib/format";
import type { Directives, Msg } from "../types";
import { t } from "../lib/i18n";
import * as ipc from "../lib/ipc";

/** Deliverable-style tools stay visible on their own — never grouped. */
const STANDALONE_TOOLS = new Set(["SendUserFile", "ExitPlanMode"]);

/** Markdown → something `say` can read aloud without spelling syntax. */
function speakable(md: string): string {
  return md
    .replace(/```[\s\S]*?```/g, " ")
    .replace(/`([^`]+)`/g, "$1")
    .replace(/!\[[^\]]*\]\([^)]*\)/g, " ")
    .replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
    .replace(/^#{1,6}\s+/gm, "")
    .replace(/[*_~]{1,3}([^*_~\n]+)[*_~]{1,3}/g, "$1")
    .replace(/^\s*[-*+]\s+/gm, "")
    .replace(/^[-:| ]+$/gm, " ")
    .replace(/\|/g, ", ")
    .replace(/\s+/g, " ")
    .trim();
}

/** "há 20 min" — self-ticking so an idle chat stays honest. */
function TimeAgo({ ts }: { ts: number }) {
  const [, setTick] = useState(0);
  useEffect(() => {
    const id = window.setInterval(() => setTick((n) => n + 1), 30_000);
    return () => window.clearInterval(id);
  }, []);
  const mins = Math.floor((Date.now() - ts) / 60_000);
  const label =
    mins < 1
      ? t("time_now")
      : mins < 60
        ? t("time_min", { n: mins })
        : mins < 1440
          ? t("time_hour", { n: Math.floor(mins / 60) })
          : t("time_day", { n: Math.floor(mins / 1440) });
  return (
    <span className="msg-when" title={new Date(ts).toLocaleString()}>
      {label}
    </span>
  );
}

/**
 * Ghost bar under a bubble (hover reveals): copy the raw markdown, and on
 * vox replies replay the whole text through TTS — zero tokens, unlike
 * asking again. Clicking listen while speaking stops it.
 */
function MsgActions({
  copyText,
  speakText,
  ts,
}: {
  copyText: string;
  speakText?: string;
  ts?: number;
}) {
  const [copied, setCopied] = useState(false);
  const [speaking, setSpeaking] = useState(false);

  async function listen() {
    if (speaking) {
      ipc.speakStop().catch(() => {});
      setSpeaking(false);
      return;
    }
    // Kill any other message still talking before starting this one.
    await ipc.speakStop().catch(() => {});
    setSpeaking(true);
    try {
      await ipc.speak(speakText ?? "");
    } finally {
      setSpeaking(false);
    }
  }

  return (
    <div className="msg-actions">
      <button
        title={t("msg_copy")}
        onClick={() => {
          navigator.clipboard.writeText(copyText).catch(() => {});
          setCopied(true);
          window.setTimeout(() => setCopied(false), 1500);
        }}
      >
        {copied ? <Check size={13} /> : <Copy size={13} />}
      </button>
      {speakText && (
        <button
          className={speaking ? "speaking" : ""}
          title={speaking ? t("msg_stop") : t("msg_listen")}
          onClick={listen}
        >
          {speaking ? <Square size={13} /> : <Volume2 size={13} />}
        </button>
      )}
      {ts != null && <TimeAgo ts={ts} />}
    </div>
  );
}

/**
 * The visible thread: one task at a time (or the general vox conversation).
 * Pure rendering; all state lives in App.
 */
export default function Transcript({
  messages,
  directivesFor,
  onAnswerPermission,
  onOpenPath,
  onRunCommand,
}: {
  messages: Msg[];
  directivesFor: (taskLabel?: string) => Directives | undefined;
  onAnswerPermission: (requestId: string, allow: boolean, always?: boolean) => void;
  onOpenPath: (path: string) => void;
  /** ▶ on shell blocks: send the command to the in-app terminal. */
  onRunCommand?: (cmd: string, execute: boolean) => void;
}) {
  const endRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    endRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  const renderOne = (m: Msg, i: number): ReactNode => (
    <div key={i} className={`msg ${m.who}`}>
      {m.who === "sys" ? (
        <span>{m.text}</span>
      ) : m.who === "tool" ? (
        <ToolCall name={m.name} input={m.input} onOpenPath={onOpenPath} />
      ) : m.who === "output" ? (
        <ToolOutput content={m.content} isError={m.error} />
      ) : m.who === "permission" ? (
        <div className={`permission ${m.decision ?? "waiting"} ${m.prodRisk ? "prod" : ""}`}>
          <div className="perm-title">
            <Lock size={13} /> {t("perm_allow_q_pre")}{" "}
            <b>{m.task ? m.task.slice(0, 32) : t("perm_worker")}</b> {t("perm_allow_q_mid")}{" "}
            <b>{m.tool}</b>?
          </div>
          {m.prodRisk && (
            <div className="perm-prod">⚠ {t("perm_prod", { reason: m.prodRisk })}</div>
          )}
          <ToolCall name={m.tool} input={m.input} onOpenPath={onOpenPath} defaultOpen />
          {m.decision ? (
            <div className={`perm-done ${m.decision}`}>
              {m.decision === "allow"
                ? m.auto
                  ? t("perm_auto")
                  : t("perm_allowed")
                : t("perm_denied")}
            </div>
          ) : (
            <div className="perm-actions">
              <button
                className="deny"
                onClick={() => onAnswerPermission(m.requestId, false)}
              >
                {t("perm_deny")} <kbd>n</kbd>
              </button>
              {!m.prodRisk && (
                <button
                  className="always"
                  title={t("perm_always_hint", { tool: m.tool })}
                  onClick={() => onAnswerPermission(m.requestId, true, true)}
                >
                  {t("perm_always")} <kbd>a</kbd>
                </button>
              )}
              <button
                className="allow"
                onClick={() => onAnswerPermission(m.requestId, true)}
              >
                {t("perm_once")} <kbd>y</kbd>
              </button>
            </div>
          )}
        </div>
      ) : (
        <div className="bubble">
          <span className="tag">
            {m.who === "user" ? t("tag_you") : "vox"}
            {m.task ? ` → ${m.task.slice(0, 12)}` : ""}
          </span>
          {m.who === "vox" ? (
            <>
              <div className="fala">
                <Markdown onRun={onRunCommand} onOpenPath={onOpenPath}>{m.text}</Markdown>
              </div>
              {m.detalhes && (
                <div className="detalhes">
                  <Markdown onRun={onRunCommand} onOpenPath={onOpenPath}>{m.detalhes}</Markdown>
                </div>
              )}
              {m.itens && m.itens.length > 0 && (
                <ul className="itens">
                  {m.itens.map((it, j) => (
                    <li key={j}>{it}</li>
                  ))}
                </ul>
              )}
              {(m.cost != null || m.model) && (
                <span
                  className="cost"
                  title={
                    m.usage
                      ? `in ${m.usage.input} · out ${m.usage.output} · cache lido ${m.usage.cache_read} · cache novo ${m.usage.cache_created}` +
                        (m.usage.input + m.usage.cache_read + m.usage.cache_created > 0
                          ? ` · cache ${Math.round((m.usage.cache_read / (m.usage.input + m.usage.cache_read + m.usage.cache_created)) * 100)}%`
                          : "")
                      : undefined
                  }
                >
                  {[
                    shortModel(m.model),
                    ...directiveLabels(directivesFor(m.task)),
                    `$${(m.cost ?? 0).toFixed(4)}`,
                  ].join(" · ")}
                </span>
              )}
              <MsgActions
                copyText={[m.text, m.detalhes, ...(m.itens ?? [])]
                  .filter(Boolean)
                  .join("\n\n")}
                speakText={speakable(
                  [m.text, m.detalhes, ...(m.itens ?? [])].filter(Boolean).join(". "),
                )}
                ts={m.ts}
              />
            </>
          ) : (
            <>
              <div>{m.text}</div>
              {m.images?.map((src, j) => (
                <img key={j} className="paste" src={src} alt={`image `} />
              ))}
              <MsgActions copyText={m.text} ts={m.ts} />
            </>
          )}
        </div>
      )}
    </div>
  );

  // Consecutive tool calls (and their outputs) collapse into ONE bordered
  // group — "executado N comandos" — the way Claude Code keeps a burst of
  // work from scattering down the chat. Deliverables and permission cards
  // always break the run and stand on their own.
  const nodes: ReactNode[] = [];
  let run: { m: Msg; i: number }[] = [];
  const flush = () => {
    if (run.length === 0) return;
    const tools = run.filter(({ m }) => m.who === "tool").length;
    if (tools >= 2) {
      nodes.push(
        <details key={`group-${run[0].i}`} className="tool-group">
          <summary>{t("tools_ran", { n: tools })}</summary>
          <div className="tg-body">{run.map(({ m, i }) => renderOne(m, i))}</div>
        </details>,
      );
    } else {
      run.forEach(({ m, i }) => nodes.push(renderOne(m, i)));
    }
    run = [];
  };
  messages.forEach((m, i) => {
    const groupable =
      (m.who === "tool" && !STANDALONE_TOOLS.has(m.name)) ||
      (m.who === "output" && run.length > 0);
    if (groupable) run.push({ m, i });
    else {
      flush();
      nodes.push(renderOne(m, i));
    }
  });
  flush();

  return (
    <div className="transcript">
      {nodes}
      <div ref={endRef} />
    </div>
  );
}
