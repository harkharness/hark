import { useEffect, useId, useReducer, useRef, useState } from "react";
import type { ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import { Check, Copy, Lock, Square, Volume2 } from "lucide-react";
import Markdown from "./Markdown";
import ToolCall, { ToolOutput, toolHint, toolLabel } from "./ToolCall";
import { directiveLabels, shortModel } from "../lib/format";
import type { Directives, Msg } from "../types";
import { t } from "../lib/i18n";
import * as ipc from "../lib/ipc";

/**
 * Who is talking, tracked from the AUDIO, not from the call.
 *
 * `speak` returns the moment it spawns its thread, so awaiting it said
 * nothing about whether the voice had finished — the button flipped back
 * to "play" while the sentence was still being read, and a second click
 * started the message over instead of stopping it. The backend already
 * announces speaking on/off; that is the only honest source.
 *
 * One listener for the whole transcript: a subscription per message row
 * would mean dozens of them.
 */
let speakingOwner: string | null = null;
/** Has the audio actually STARTED since the current owner claimed it?
 *  Without this, the "off" event fired by stopping the previous message
 *  would immediately clear the owner that just claimed. */
let audioStarted = false;
const speakSubs = new Set<() => void>();
let speakWired = false;

function notifySpeakSubs() {
  speakSubs.forEach((fn) => fn());
}

function wireSpeakEvents() {
  if (speakWired) return;
  speakWired = true;
  listen<{ kind?: string; on?: boolean }>("hark", (e) => {
    if (e.payload?.kind !== "speaking") return;
    if (e.payload.on) {
      audioStarted = true;
    } else if (audioStarted) {
      speakingOwner = null;
      audioStarted = false;
    }
    notifySpeakSubs();
  });
}

/** Is THIS row the one being read, and how to claim or release it. */
function useSpeaking(id: string) {
  const [, bump] = useReducer((n: number) => n + 1, 0);
  useEffect(() => {
    wireSpeakEvents();
    speakSubs.add(bump);
    return () => {
      speakSubs.delete(bump);
    };
  }, []);
  return {
    speaking: speakingOwner === id,
    claim: () => {
      speakingOwner = id;
      audioStarted = false;
      notifySpeakSubs();
    },
    release: () => {
      if (speakingOwner === id) {
        speakingOwner = null;
        audioStarted = false;
        notifySpeakSubs();
      }
    },
  };
}

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
 * hark replies replay the whole text through TTS — zero tokens, unlike
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
  const rowId = useId();
  const { speaking, claim, release } = useSpeaking(rowId);

  async function toggleSpeech() {
    if (speaking) {
      // Cutting also drops whatever was queued behind it — the mutex in
      // the backend serialises reads, which is why a second click used to
      // QUEUE another full reading instead of interrupting.
      await ipc.speakStop().catch(() => {});
      release();
      return;
    }
    // Kill any other message still talking before starting this one.
    await ipc.speakStop().catch(() => {});
    claim();
    ipc.speak(speakText ?? "").catch(() => release());
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
          onClick={toggleSpeech}
        >
          {speaking ? <Square size={13} /> : <Volume2 size={13} />}
        </button>
      )}
      {ts != null && <TimeAgo ts={ts} />}
    </div>
  );
}

/**
 * The visible thread: one task at a time (or the general hark conversation).
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
        m.decision ? (
          // Decided: the card COLLAPSES to one quiet line — the decision
          // is the record, the payload stays one click away.
          <details className={`perm-line ${m.decision}`}>
            <summary>
              <span className={`tool-tick ${m.decision === "allow" ? "ok" : "err"}`}>
                {m.decision === "allow" ? "✓" : "✗"}
              </span>
              <span className="toolname">{toolLabel(m.tool).label}</span>
              <span className="tool-hint">{toolHint(m.tool, m.input)}</span>
              <span className={`perm-how ${m.decision}`}>
                {m.decision === "allow"
                  ? m.auto
                    ? t("perm_auto")
                    : t("perm_allowed")
                  : t("perm_denied")}
              </span>
            </summary>
            <div className="tool-body">
              <ToolCall name={m.tool} input={m.input} onOpenPath={onOpenPath} defaultOpen />
            </div>
          </details>
        ) : (
          <div className={`permission waiting ${m.prodRisk ? "prod" : ""}`}>
            <div className="perm-title">
              <Lock size={13} /> <b className="perm-who">{m.label ?? m.task ?? t("perm_worker")}</b>{" "}
              {t("perm_asks")} <b>{toolLabel(m.tool).label}</b>
              <span className="tool-hint">{toolHint(m.tool, m.input)}</span>
            </div>
            {m.prodRisk && (
              <div className="perm-prod">⚠ {t("perm_prod", { reason: m.prodRisk })}</div>
            )}
            <ToolCall name={m.tool} input={m.input} onOpenPath={onOpenPath} defaultOpen />
            <div className="perm-actions">
              <span className="perm-voice-hint">{t("perm_voice_hint")}</span>
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
          </div>
        )
      ) : (
        <div className="bubble">
          <span className="tag">
            {m.who === "user" ? t("tag_you") : "hark"}
            {m.task ? ` → ${m.task.slice(0, 12)}` : ""}
          </span>
          {m.who === "hark" ? (
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

  // A tool and its output are ONE unit: the output fuses into the tool's
  // fold (✓/✗ on the line, "resultado · N linhas" inside). Orphan outputs
  // render on their own.
  type Unit = {
    m: Msg;
    i: number;
    result?: { content: string; error: boolean };
    decision?: "allow" | "deny";
  };
  const units: Unit[] = [];
  messages.forEach((m, i) => {
    if (m.who === "output") {
      const last = units.at(-1);
      if (last && last.m.who === "tool") {
        last.result = last.result
          ? {
              content: `${last.result.content}\n${m.content}`,
              error: last.result.error || m.error,
            }
          : { content: m.content, error: m.error };
        return;
      }
    }
    // Same fusion the other way round: an auto-approved card can land
    // after the tool row it belongs to.
    if (m.who === "permission" && m.decision) {
      const twin = units.find(
        (u) =>
          u.m.who === "tool" &&
          !u.decision &&
          u.m.name === m.tool &&
          u.m.input === m.input,
      );
      if (twin) {
        twin.decision = m.decision;
        return;
      }
    }
    // The ask and the call it authorized are the same event twice: the
    // card arrives when the CLI asks, the tool row when the block lands.
    // Seven MCP reads showed fourteen rows. Same tool, same payload, one
    // line — the decision rides along as a badge.
    if (m.who === "tool") {
      const twin = units.findIndex(
        (u) =>
          u.m.who === "permission" &&
          !!u.m.decision &&
          u.m.tool === m.name &&
          u.m.input === m.input,
      );
      if (twin >= 0) {
        const decision = (units[twin].m as { decision?: "allow" | "deny" }).decision;
        units.splice(twin, 1);
        units.push({ m, i, decision });
        return;
      }
    }
    units.push({ m, i });
  });

  const renderUnit = ({ m, i, result, decision }: Unit): ReactNode =>
    m.who === "tool" ? (
      <div key={i} className={`msg ${m.who}`}>
        <ToolCall
          name={m.name}
          input={m.input}
          onOpenPath={onOpenPath}
          result={result}
          decision={decision}
        />
      </div>
    ) : (
      renderOne(m, i)
    );

  // Consecutive tool units collapse into ONE bordered group — "executado
  // N comandos" — the way Claude Code keeps a burst of work from
  // scattering down the chat. Deliverables and permission cards always
  // break the run and stand on their own.
  const nodes: ReactNode[] = [];
  let run: Unit[] = [];
  const flush = () => {
    if (run.length === 0) return;
    const tools = run.filter(
      ({ m }) => m.who === "tool" || (m.who === "permission" && m.decision),
    ).length;
    if (tools >= 2) {
      nodes.push(
        <details key={`group-${run[0].i}`} className="tool-group">
          <summary>{t("tools_ran", { n: tools })}</summary>
          <div className="tg-body">{run.map(renderUnit)}</div>
        </details>,
      );
    } else {
      run.forEach((u) => nodes.push(renderUnit(u)));
    }
    run = [];
  };
  units.forEach((u) => {
    const groupable =
      (u.m.who === "tool" && !STANDALONE_TOOLS.has(u.m.name)) ||
      // A DECIDED permission is a record, not a request: it folds into the
      // run like the tool call it belongs to. An open ask breaks the run —
      // it is the one thing on screen waiting for the user.
      (u.m.who === "permission" && !!u.m.decision) ||
      (u.m.who === "output" && run.length > 0);
    if (groupable) run.push(u);
    else {
      flush();
      nodes.push(renderUnit(u));
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
