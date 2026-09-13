import {
  memo,
  useCallback,
  useEffect,
  useId,
  useLayoutEffect,
  useReducer,
  useRef,
  useState,
} from "react";
import type { ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import { Check, ChevronDown, Copy, Lock, RefreshCw, Square, Volume2, X } from "lucide-react";
import Markdown from "./Markdown";
import ToolCall, { ToolOutput, toolHint, toolLabel } from "./ToolCall";
import UsageCard from "./UsageCard";
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

/**
 * One function identity for the life of the component, always calling the
 * LATEST version handed in. The identity changes only when the callback
 * appears or disappears — "has a run button" is a real difference, and
 * Markdown decides it by `onRun` being there at all.
 */
function useLatest<F extends ((...args: never[]) => unknown) | undefined>(fn: F): F {
  const ref = useRef(fn);
  useLayoutEffect(() => {
    ref.current = fn;
  });
  const stable = useCallback(
    (...args: unknown[]) =>
      (ref.current as ((...a: unknown[]) => unknown) | undefined)?.(...args),
    [],
  );
  return (fn ? stable : undefined) as F;
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
  speakSource,
  ts,
}: {
  copyText: string;
  /** Raw markdown; turned into speech text on the click, not on every
   *  render — eight regex passes over a long reply, per row, per render,
   *  was part of what made a thread crawl. */
  speakSource?: string;
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
    ipc.speak(speakable(speakSource ?? "")).catch(() => release());
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
      {speakSource && (
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

/** What one row of the thread needs — and nothing that changes when the
 *  thread does not. `m` is compared by identity: `push` appends, and
 *  every `setMessages(old.map(...))` keeps untouched messages `===`. */
type RowProps = {
  m: Msg;
  /** The task's CURRENT directives, for the cost line under a reply. */
  directives?: Directives;
  onAnswerPermission: (requestId: string, allow: boolean, always?: boolean) => void;
  onOpenPath: (path: string) => void;
  onRunCommand?: (cmd: string, execute: boolean) => void;
  onQueuedNow?: (msgId: string, taskLabel?: string) => void;
  onQueuedDrop?: (msgId: string, taskLabel?: string) => void;
};

/**
 * One message. Memoised, because the thread re-renders far more often
 * than any message changes — a poll settling, a reply landing, a turn
 * reporting — and each render of a reply is a markdown parse plus a
 * syntax highlight. With 200 rows on screen that was seconds per render,
 * every few seconds, on an idle window (measured 13/09; docs/FRONTEND.md).
 */
const Row = memo(function Row({
  m,
  directives,
  onAnswerPermission,
  onOpenPath,
  onRunCommand,
  onQueuedNow,
  onQueuedDrop,
}: RowProps) {
  return (
    <div className={`msg ${m.who}`}>
      {m.who === "sys" ? (
        <span>{m.text}</span>
      ) : m.who === "usage" ? (
        <UsageCard report={m.report} />
      ) : m.who === "compact" ? (
        // The CLI's compaction: pages of summary it wrote to itself. It
        // belongs to the record — you can read what it kept — but not to
        // the conversation, so it arrives folded.
        <details className="compaction">
          <summary>
            <RefreshCw size={11} /> {t("compaction_folded")}
          </summary>
          <div className="cmp-body">
            <Markdown onOpenPath={onOpenPath}>{m.text}</Markdown>
          </div>
        </details>
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
          <div
            className={`permission ${m.expired ? "expired" : "waiting"} ${
              m.prodRisk ? "prod" : ""
            }`}
          >
            <div className="perm-title">
              <Lock size={13} /> <b className="perm-who">{m.label ?? m.task ?? t("perm_worker")}</b>{" "}
              {t("perm_asks")} <b>{toolLabel(m.tool).label}</b>
              <span className="tool-hint">{toolHint(m.tool, m.input)}</span>
            </div>
            {m.prodRisk && (
              <div className="perm-prod">⚠ {t("perm_prod", { reason: m.prodRisk })}</div>
            )}
            <ToolCall name={m.tool} input={m.input} onOpenPath={onOpenPath} defaultOpen />
            {m.expired ? (
              <div className="perm-expired">{t("perm_expired")}</div>
            ) : (
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
            )}
          </div>
        )
      ) : (
        <div
          className={`bubble${
            m.who === "user" && m.queued ? ` queued-${m.queued.state}` : ""
          }`}
        >
          {/* No "você ·" / "hark ·" label: who spoke is the SHAPE now.
              Reopening a long history, two identical columns of prose with
              a small caption over each was unreadable — the eye has to
              read the label to know whose words it is looking at, on every
              single message. The time already lives in the hover bar. */}
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
                    ...directiveLabels(directives),
                    `$${(m.cost ?? 0).toFixed(4)}`,
                  ].join(" · ")}
                </span>
              )}
              <MsgActions
                copyText={[m.text, m.detalhes, ...(m.itens ?? [])]
                  .filter(Boolean)
                  .join("\n\n")}
                speakSource={[m.text, m.detalhes, ...(m.itens ?? [])]
                  .filter(Boolean)
                  .join(". ")}
                ts={m.ts}
              />
            </>
          ) : (
            <>
              <div>{m.text}</div>
              {m.images?.map((src, j) => (
                <img key={j} className="paste" src={src} alt={`image `} />
              ))}
              {/* Typed behind a running turn. The CLI would have queued it
                  on stdin, out of sight and out of reach; held here, the
                  bubble can say it is waiting and offer the two things you
                  actually want — run it NOW, or take it back. */}
              {m.queued?.state === "waiting" && m.msgId && (
                <div className="queued-row">
                  <span className="queued-tag">{t("queued_waiting")}</span>
                  <button
                    className="queued-x"
                    title={t("queued_drop")}
                    onClick={() => onQueuedDrop?.(m.msgId!, m.task)}
                  >
                    <X size={13} />
                  </button>
                  <button className="queued-now" onClick={() => onQueuedNow?.(m.msgId!, m.task)}>
                    {t("queued_now")}
                  </button>
                </div>
              )}
              {m.queued?.state === "failed" && (
                <div className="queued-row failed">{m.queued.why ?? t("queued_lost")}</div>
              )}
              <MsgActions copyText={m.text} ts={m.ts} />
            </>
          )}
        </div>
      )}
    </div>
  );
});

/** A tool call with its fused output. The output arrives as primitives
 *  (not the `{content, error}` object the caller rebuilds every render)
 *  so the memo can compare by value. */
const ToolRow = memo(function ToolRow({
  m,
  decision,
  resultContent,
  resultError,
  onOpenPath,
}: {
  m: Extract<Msg, { who: "tool" }>;
  decision?: "allow" | "deny";
  resultContent?: string;
  resultError?: boolean;
  onOpenPath: (path: string) => void;
}) {
  return (
    <div className={`msg ${m.who}`}>
      <ToolCall
        name={m.name}
        input={m.input}
        onOpenPath={onOpenPath}
        result={
          resultContent != null ? { content: resultContent, error: !!resultError } : undefined
        }
        decision={decision}
      />
    </div>
  );
});

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
  onLoadOlder,
  onQueuedNow,
  onQueuedDrop,
}: {
  messages: Msg[];
  directivesFor: (taskLabel?: string) => Directives | undefined;
  onAnswerPermission: (requestId: string, allow: boolean, always?: boolean) => void;
  onOpenPath: (path: string) => void;
  /** ▶ on shell blocks: send the command to the in-app terminal. */
  onRunCommand?: (cmd: string, execute: boolean) => void;
  /** Reach further back in this thread's log. Absent at the beginning of
   *  the history, or with no thread focused. */
  onLoadOlder?: () => void;
  /** A waiting message jumps the queue: the running turn is cut and this
   *  one runs next. */
  onQueuedNow?: (msgId: string, taskLabel?: string) => void;
  /** ...or never runs at all. */
  onQueuedDrop?: (msgId: string, taskLabel?: string) => void;
}) {
  const endRef = useRef<HTMLDivElement>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const lastRef = useRef<Msg | null>(null);
  // Starts true: a thread that has just opened is AT its end, and the
  // first append is what puts it there on screen.
  const atBottomRef = useRef(true);
  const [atBottom, setAtBottom] = useState(true);

  const toEnd = (behavior: ScrollBehavior = "smooth") =>
    endRef.current?.scrollIntoView({ behavior });

  // Follow the conversation only when something was ADDED to the end.
  // Loading older history prepends, and jumping to the bottom right after
  // would throw away exactly what the click asked to see.
  useEffect(() => {
    const last = messages.at(-1) ?? null;
    if (last === lastRef.current) return;
    lastRef.current = last;
    // Reading back through a long thread is work: an arriving reply must
    // not yank the page out from under it. Your OWN message is different
    // — you just sent it, you want to watch it land.
    if (!atBottomRef.current && last?.who !== "user") return;
    toEnd();
  }, [messages]);

  // App hands these down as inline lambdas and plain function declarations
  // — a new identity every render, which would re-render every memoised
  // row for nothing. Pinned here, once, for the whole thread.
  const answer = useLatest(onAnswerPermission);
  const open = useLatest(onOpenPath);
  const runCmd = useLatest(onRunCommand);
  const queueNow = useLatest(onQueuedNow);
  const queueDrop = useLatest(onQueuedDrop);

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
      <ToolRow
        key={i}
        m={m}
        decision={decision}
        resultContent={result?.content}
        resultError={result?.error}
        onOpenPath={open}
      />
    ) : (
      <Row
        key={i}
        m={m}
        // Only a reply shows directives; every other row keeps its props still.
        directives={m.who === "hark" ? directivesFor(m.task) : undefined}
        onAnswerPermission={answer}
        onOpenPath={open}
        onRunCommand={runCmd}
        onQueuedNow={queueNow}
        onQueuedDrop={queueDrop}
      />
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
    <div className="transcript-wrap">
      <div
        className="transcript"
        ref={scrollRef}
        onScroll={(e) => {
          const el = e.currentTarget;
          // 80px of slack: "close enough to the end" has to survive the
          // last line still being laid out.
          const near = el.scrollHeight - el.scrollTop - el.clientHeight < 80;
          atBottomRef.current = near;
          setAtBottom(near);
        }}
      >
        {onLoadOlder && (
          <button className="load-older" onClick={onLoadOlder}>
            {t("history_older")}
          </button>
        )}
        {nodes}
        <div ref={endRef} />
      </div>
      {/* Only while there is somewhere to go: scrolled to the end it would
          be a button that does nothing. */}
      {!atBottom && (
        <button className="jump-latest" title={t("jump_latest")} onClick={() => toEnd()}>
          <ChevronDown size={16} />
        </button>
      )}
    </div>
  );
}
