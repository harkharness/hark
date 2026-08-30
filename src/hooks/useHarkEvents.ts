// The single subscription point for backend events. Workers stream rich
// events ("hark") and permission requests ("hark-permission"); this hook
// translates them into transcript messages and worker-chip state.

import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import * as ipc from "../lib/ipc";
import { st, t } from "../lib/i18n";
import { isAuthError } from "../lib/format";
import type { LiveWorker, Msg, PermissionAsk, HarkEvent } from "../types";

type Handlers = {
  labelFor: (taskId: string) => string;
  push: (m: Msg) => void;
  setMessages: React.Dispatch<React.SetStateAction<Msg[]>>;
  setLiveWorkers: React.Dispatch<React.SetStateAction<Record<string, LiveWorker>>>;
  onWorkerExit: (taskId: string, reason?: string | null) => void;
  onSessionStarted: (taskId: string, sessionId: string) => void;
  /** Raw per-thread feed (the task's own "terminal"). */
  pushRaw: (label: string, line: string) => void;
  /** Window-accumulated spend, per thread label. */
  addCost: (label: string, usd: number) => void;
  /** TTS started/stopped (drives the voice orb). */
  onSpeaking: (on: boolean) => void;
  /** What the ONE microphone is doing, whoever asked for it. The mic
   *  button and the orb live in these windows but the capture runs in the
   *  HUD, so without this they never lit up at all. */
  onMicPhase?: (phase: "capturing" | "transcribing" | "idle") => void;
  /** What a running turn is doing right now, from real stream events:
   *  prose arriving, a tool starting, a tool finishing. The thread comes
   *  first — the status belongs to one chat, and several can run. */
  onTurnActivity?: (
    thread: string,
    phase: "thinking" | "writing" | "tool",
    detail?: string,
    chars?: number,
  ) => void;
  /** Subscription window signal from the newest turn. */
  onRateLimit: (state: import("../types").RateLimitState) => void;
  /** Global hotkey pressed (mother window only receives this). */
  onHotkeyMic?: () => void;
  /** Another window asked the mother to show a tab (board/custos). */
  onMainTab?: (tab: string) => void;
  /** The global board asked this project window to open a task's chat. */
  onFocusTask?: (title: string, sessionId?: string | null) => void;
  /** The voice HUD executed something (the mother records the feed). */
  onVoiceAction?: (utterance: string, target?: string | null, status?: string) => void;
  /** A spoken turn echoed into the mother's unified thread. */
  onChatEcho?: (
    question: string,
    reply:
      | { fala: string; detalhes?: string; itens?: string[]; cost_usd?: number; model?: string }
      | undefined,
    work: boolean,
  ) => void;
  /** Spoken "sempre pode" — record the standing allow rule. */
  onAllowRule?: (label: string, tool: string) => void;
  /** A worker finished a turn (the mother updates its feed row and, for
   *  the hark chat, its session cost/context header). */
  onWorkerTurn?: (
    label: string,
    isError: boolean,
    taskId?: string,
    contextPct?: number | null,
    costUsd?: number,
    sessionId?: string | null,
    errorCode?: string | null,
  ) => void;
  /**
   * Whether THIS window announces events out loud (turn done, permission
   * asked). Exactly one window may announce — the mother — otherwise every
   * open window speaks the same news and the voice stutters in chorus.
   */
  announce?: boolean;
  /** Override WHAT a finished turn says (the hark chat speaks its actual
   *  reply, not "task done"). Return null/undefined for the default. */
  turnSpeech?: (
    taskId: string,
    text: string,
    isError: boolean,
    label: string,
  ) => string | null | undefined;
  /**
   * Standing "sempre permitir" rules: return true to auto-approve this
   * ask without interrupting anyone (the card lands already decided).
   */
  autoAllow?: (ask: PermissionAsk) => boolean;
  speakRef: React.RefObject<boolean>;
  refresh: () => void;
};

const ts = () => new Date().toTimeString().slice(0, 8);
const clip = (s: string, n: number) => (s.length > n ? `${s.slice(0, n)}…` : s);

export function useHarkEvents(h: Handlers) {
  useEffect(() => {
    const un1 = listen<HarkEvent>("hark", (e) => {
      const ev = e.payload;
      if (ev.kind === "assistant_text") {
        h.onTurnActivity?.(h.labelFor(ev.task_id), "writing", undefined, ev.text.length);
        h.push({ who: "hark", text: ev.text, task: h.labelFor(ev.task_id) });
        h.pushRaw(h.labelFor(ev.task_id), `${ts()} ${clip(ev.text, 400)}`);
      } else if (ev.kind === "worker") {
        h.onTurnActivity?.(h.labelFor(ev.task_id), "tool", ev.name);
        h.push({ who: "tool", name: ev.name, input: ev.input, task: h.labelFor(ev.task_id) });
        h.pushRaw(h.labelFor(ev.task_id), `${ts()} ⚙ ${ev.name} ${clip(ev.input, 400)}`);
      } else if (ev.kind === "tool_result") {
        // The tool answered; the model is thinking about it again.
        h.onTurnActivity?.(h.labelFor(ev.task_id), "thinking");
        h.push({
          who: "output",
          content: ev.content,
          error: ev.is_error,
          task: h.labelFor(ev.task_id),
        });
        h.pushRaw(
          h.labelFor(ev.task_id),
          `${ts()} ${ev.is_error ? "✗" : "✓"} ${clip(ev.content, 400)}`,
        );
      } else if (ev.kind === "worker_turn") {
        const label = h.labelFor(ev.task_id);
        // An auth failure ships its own remedy as a runnable block: the
        // ▶ on the fence opens the terminal pane and runs it (26/08 —
        // prose telling the user what to type helped nobody).
        const fence = "```";
        const authFail =
          ev.error_code === "agent_auth" || (ev.is_error && isAuthError(ev.text));
        const turnText = authFail
          ? `${ev.text}\n\n${t("auth_fix")}\n\n${fence}bash\nclaude /login\n${fence}`
          : ev.text;
        h.pushRaw(
          label,
          `${ts()} ── turno ${ev.is_error ? "FALHOU " : ""}${ev.model ?? ""} $${(ev.cost_usd ?? 0).toFixed(4)}`,
        );
        if (ev.cost_usd) h.addCost(label, ev.cost_usd);
        // The final assistant_text often equals the result: don't show twice.
        h.setMessages((old) => {
          // A compaction turn was already folded away on arrival; matching
          // it here too keeps the summary from being pasted a second time.
          const lastHark = [...old]
            .reverse()
            .find((m) => (m.who === "hark" || m.who === "compact") && m.task === label);
          if (lastHark && "text" in lastHark && lastHark.text === turnText) {
            if (lastHark.who === "compact") return old;
            return old.map((m) =>
              m === lastHark
                ? { ...m, cost: ev.cost_usd, model: ev.model, usage: ev.usage }
                : m,
            );
          }
          return [
            ...old,
            {
              who: "hark",
              text: turnText,
              cost: ev.cost_usd,
              model: ev.model,
              usage: ev.usage,
              task: label,
            },
          ];
        });
        h.setLiveWorkers((old) =>
          old[ev.task_id]
            ? {
                ...old,
                [ev.task_id]: {
                  ...old[ev.task_id],
                  status: "turn_done",
                  context_pct: ev.context_pct ?? old[ev.task_id].context_pct,
                },
              }
            : old,
        );
        h.onWorkerTurn?.(
          ev.label ?? label,
          ev.is_error,
          ev.task_id,
          ev.context_pct,
          ev.cost_usd,
          ev.session_id,
          authFail ? "agent_auth" : ev.error_code,
        );
        if (h.announce && h.speakRef.current) {
          const spoken = ev.label ?? label;
          const custom = h.turnSpeech?.(ev.task_id, ev.text, ev.is_error, spoken);
          ipc.speak(
            custom ??
              (ev.is_error
                ? st("sp_turn_failed", { t: spoken })
                : st("sp_turn_done", { t: spoken })),
          ).catch(() => {});
        }
        h.refresh();
      } else if (ev.kind === "phase") {
        // "requesting" and "thinking" are both waiting as far as the user
        // is concerned; the status line escalates its wording by elapsed.
        h.onTurnActivity?.(
          h.labelFor(ev.task_id),
          ev.phase === "writing" ? "writing" : "thinking",
        );
      } else if (ev.kind === "worker_exit") {
        // The reason travels with the event now; "encerrado" alone left
        // the user staring at a dead chat with nothing to act on.
        const why =
          ev.reason ??
          (ev.exit_code != null && ev.exit_code !== 0 ? `exit ${ev.exit_code}` : null);
        h.push({
          who: "sys",
          text: `worker ${h.labelFor(ev.task_id)} encerrado${why ? ` — ${why}` : ""}`,
          task: h.labelFor(ev.task_id),
        });
        h.onWorkerExit(ev.task_id, ev.reason);
        h.refresh();
      } else if (ev.kind === "session_started") {
        h.onSessionStarted(ev.task_id, ev.session_id);
        h.refresh();
      } else if (ev.kind === "speaking") {
        h.onSpeaking(ev.on);
      } else if (ev.kind === "mic") {
        h.onMicPhase?.(ev.phase);
      } else if (ev.kind === "hotkey_mic") {
        h.onHotkeyMic?.();
      } else if (ev.kind === "main_tab") {
        h.onMainTab?.(ev.tab);
      } else if (ev.kind === "focus_task") {
        h.onFocusTask?.(ev.title, ev.session_id);
      } else if (ev.kind === "voice_action") {
        h.onVoiceAction?.(ev.utterance, ev.target, ev.status);
        h.refresh();
      } else if (ev.kind === "chat_echo") {
        h.onChatEcho?.(ev.question, ev.reply, ev.work);
      } else if (ev.kind === "allow_rule") {
        h.onAllowRule?.(ev.label, ev.tool);
      } else if (ev.kind === "permission_decided") {
        // Someone answered (click, keys, voice, the HUD): every window's
        // copy of the card resolves — no stale "aguardando" anywhere.
        h.setMessages((old) =>
          old.map((m) =>
            m.who === "permission" && m.requestId === ev.request_id && !m.decision
              ? { ...m, decision: ev.allow ? "allow" : "deny" }
              : m,
          ),
        );
      } else if (ev.kind === "config_changed") {
        // Settings saved somewhere: theme/mode/ceilings re-read via refresh.
        h.refresh();
      } else if (ev.kind === "rate_limit") {
        h.onRateLimit({
          status: ev.status,
          resets_at: ev.resets_at,
          limit_kind: ev.limit_kind,
        });
      } else if (ev.kind === "status") {
        h.push({ who: "sys", text: ev.text });
      }
    });

    // Permission requests land INLINE in the thread, never as a blocking
    // modal: other workers must keep streaming while one waits.
    const un2 = listen<PermissionAsk>("hark-permission", (e) => {
      const ask = e.payload;
      const thread = h.labelFor(ask.task_id);
      // A standing "sempre permitir" rule answers on the spot: the card
      // shows up already decided and nobody is interrupted. NEVER for a
      // production-gated ask — that one always reaches a human.
      if (!ask.prod_risk && h.autoAllow?.(ask)) {
        h.push({
          who: "permission",
          requestId: ask.request_id,
          tool: ask.tool_name,
          input: ask.input,
          decision: "allow",
          auto: true,
          task: thread,
        });
        h.pushRaw(thread, `${ts()} 🔐 ${ask.tool_name} auto-permitido (regra da task)`);
        ipc.approve(ask.request_id, true).catch(() => {});
        return;
      }
      h.push({
        who: "permission",
        requestId: ask.request_id,
        tool: ask.tool_name,
        input: ask.input,
        prodRisk: ask.prod_risk,
        label: ask.label,
        task: thread,
      });
      h.pushRaw(thread, `${ts()} 🔐 ${ask.tool_name} aguardando decisão`);
      h.setLiveWorkers((old) =>
        old[ask.task_id]
          ? { ...old, [ask.task_id]: { ...old[ask.task_id], status: "awaiting" } }
          : old,
      );
      if (h.announce && h.speakRef.current)
        ipc.speak(
          ask.label
            ? st("sp_perm_ask", { tool: ask.tool_name, t: ask.label })
            : st("sp_perm_ask_bare", { tool: ask.tool_name }),
        ).catch(() => {});
    });

    return () => {
      un1.then((f) => f());
      un2.then((f) => f());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
}
