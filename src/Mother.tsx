import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ExternalLink, Lock, Maximize2, Mic, Minimize2 } from "lucide-react";
import Board from "./components/Board";
import { setLang, setSpeechLang, st, t } from "./lib/i18n";
import CostsPanel from "./components/CostsPanel";
import Settings from "./components/Settings";
import Onboarding from "./components/Onboarding";
import Transcript from "./components/Transcript";
import VoiceOrb, { type OrbMode } from "./components/VoiceOrb";
import { Settings as SettingsIcon } from "lucide-react";
import { useHarkEvents } from "./hooks/useHarkEvents";
import { agentError } from "./lib/format";
import * as ipc from "./lib/ipc";
import { checkForUpdate, restartIntoUpdate } from "./lib/updater";
import type { BoardTask, Msg, Overview, Project, RateLimitState, SessionHit } from "./types";

type MotherTab = "voz" | "board" | "custos";

/** The mother's persistent work chat (backend task id — off the board). */
const HARK_CHAT = "hark-chat";

/** Markdown → one readable line for the compact 5-message panel. */
function miniText(md: string): string {
  return md
    .replace(/```[\s\S]*?```/g, " ")
    .replace(/[*_`#>]/g, "")
    .replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
    .replace(/\s+/g, " ")
    .trim();
}

/** What the voice reads from a chat reply: the first sentence, clean. */
function firstSentence(md: string): string {
  const clean = miniText(md);
  const first = clean.split(/(?<=[.!?…])\s+/)[0] ?? clean;
  return first.length > 240 ? `${first.slice(0, 240)}…` : first;
}

/** The compact panel is a DIGEST, never a wall: the last two exchanges,
 *  question on one clamped line, reply on two, tool bursts as one
 *  activity line. Full text lives behind "expandir". */
type DigestRow =
  | { kind: "user" | "hark" | "sys"; text: string; key: number }
  | { kind: "tools"; n: number; err: boolean; key: number };

function buildDigest(msgs: Msg[]): DigestRow[] {
  const rows: DigestRow[] = [];
  msgs.forEach((m, i) => {
    if (m.who === "tool") {
      const last = rows.at(-1);
      if (last?.kind === "tools") last.n += 1;
      else rows.push({ kind: "tools", n: 1, err: false, key: i });
    } else if (m.who === "output") {
      const last = rows.at(-1);
      if (last?.kind === "tools" && m.error) last.err = true;
    } else if (m.who === "user" || m.who === "hark" || m.who === "sys") {
      rows.push({ kind: m.who, text: miniText(m.text), key: i });
    }
  });
  // Cut at the second-to-last question: two exchanges, newest at the end.
  const users = rows.map((r, i) => (r.kind === "user" ? i : -1)).filter((i) => i >= 0);
  const start = users.length >= 2 ? users[users.length - 2] : 0;
  return rows.slice(start).slice(-8);
}

/**
 * The mother window: the voice of Hark AND the global views. Three tabs:
 * voz (orb + mic), board (every project's demands — demands belong to the
 * user, not to a directory) and custos (the whole ledger). Project windows
 * are filtered views; closing the mother closes everything.
 *
 * Spoken commands resolve locally first (task_command: open project, new
 * chat, board/costs) — zero tokens; only real questions reach the ask
 * pipeline.
 */
export default function Mother() {
  const [overview, setOverview] = useState<Overview | null>(null);
  const [messages, setMessages] = useState<Msg[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [recording, setRecording] = useState(false);
  const [speaking, setSpeaking] = useState(false);
  const [rateLimit, setRateLimit] = useState<RateLimitState | null>(null);
  const [spentToday, setSpentToday] = useState<number | null>(null);
  const [input, setInput] = useState("");
  const [tab, setTab] = useState<MotherTab>("voz");
  // A downloaded-and-verified update waiting for the user's click.
  const [updateReady, setUpdateReady] = useState<string | null>(null);
  // The butler offer: a chat finished elsewhere, the NEXT sentence may be
  // about it ("vai lá", "faz X lá"). One at a time — newest wins.
  const followUpRef = useRef<{ label: string; sessionId: string } | null>(null);
  // The message the chat is answering right now. If the worker dies with
  // this in flight, it is resent ONCE through the normal respawn path —
  // a request must never evaporate because a process did.
  const chatInFlightRef = useRef<{ text: string; retried: boolean } | null>(null);
  // Sessions matching a recovery request, waiting for the user to pick one.
  const [picks, setPicks] = useState<{ query: string; candidates: SessionHit[] } | null>(null);
  // A typed work instruction planned and waiting for the user's confirm.
  const [pendingPlan, setPendingPlan] = useState<{
    text: string;
    plan: Extract<import("./types").VoicePlan, { kind: "work" }>;
  } | null>(null);
  // App-level settings modal (machine config; project config is elsewhere).
  const [settingsOpen, setSettingsOpen] = useState(false);
  // "+ projeto" card flips into a path input.
  const [addingProject, setAddingProject] = useState(false);
  // What the voice did and where: the command-center feed.
  const [actions, setActions] = useState<
    { utterance: string; target?: string | null; status?: string; ts: number }[]
  >([]);
  // Spend today per workspace (feeds the project cards).
  const [spendByWs, setSpendByWs] = useState<Record<string, number>>({});
  // The persistent work chat: expanded split view + live session header.
  const [chatExpanded, setChatExpanded] = useState(false);
  const [chatCost, setChatCost] = useState(0);
  const [chatCtx, setChatCtx] = useState<number | null>(null);
  const [chatLive, setChatLive] = useState(false);
  const speakRef = useRef(true);
  // The global hotkey/Esc handlers must see fresh state.
  const micRef = useRef<() => void>(() => {});
  const recordingRef = useRef(false);

  const push = useCallback(
    (m: Msg) =>
      setMessages((old) =>
        [
          ...old,
          (m.who === "user" || m.who === "hark") && m.ts == null
            ? { ...m, ts: Date.now() }
            : m,
        ].slice(-80),
      ),
    [],
  );

  // The unified thread: local turns (typed asks) + the persistent work
  // chat's events. Project workers' messages stay OUT — they belong to
  // their own windows; the mother only announces them in the feed.
  const chatMsgs = useMemo(
    () => messages.filter((m) => !m.task || m.task === HARK_CHAT),
    [messages],
  );

  // First-run wizard: a virgin machine (no config, no whisper model or no
  // claude CLI) gets the guided setup instead of a dead microphone.
  const [setup, setSetup] = useState<ipc.SetupStatus | null>(null);
  useEffect(() => {
    ipc
      .setupStatus()
      .then((s) => {
        if (!s.onboarded && (!s.config_exists || !s.whisper_ok || !s.claude_ok)) setSetup(s);
      })
      .catch(() => {});
  }, []);

  // Update poll: on boot and every 6h, gated by config. The download runs
  // in the background; only the pill's click ever restarts anything.
  useEffect(() => {
    let alive = true;
    const poll = async () => {
      const cfg = await ipc.configRead().catch(() => null);
      if (!cfg?.values.auto_update) return;
      const st = await checkForUpdate();
      if (alive && st.kind === "ready") setUpdateReady(st.version);
    };
    poll();
    const id = setInterval(poll, 6 * 60 * 60 * 1000);
    return () => {
      alive = false;
      clearInterval(id);
    };
  }, []);

  // On boot, the stored chat session repaints the thread: the chat is
  // CONTINUOUS across app restarts, visually too. Local, zero tokens.
  useEffect(() => {
    (async () => {
      const st = await ipc.harkChatStatus().catch(() => null);
      if (!st?.session_id) return;
      setChatLive(true);
      const tr = await ipc.readTranscript(st.session_id, 30).catch(() => null);
      if (!tr) return;
      const hist: Msg[] = tr.entries
        .filter((e) => e.role === "user" || e.role === "assistant")
        .map((e) => ({
          who: e.role === "user" ? ("user" as const) : ("hark" as const),
          text: e.text,
          task: HARK_CHAT,
          ts: Date.parse(e.ts) || undefined,
        }));
      if (hist.length) setMessages((old) => [...hist, ...old].slice(-80));
    })();
  }, []);
  const refresh = useCallback(() => {
    ipc
      .overview()
      .then((o) => {
        // Language first: setOverview re-renders with t() already right.
        setLang(o.ui_language);
        setSpeechLang(o.language);
        setOverview(o);
        document.documentElement.dataset.theme = o.theme;
      })
      .catch(() => {});
    const day = new Date(Date.now() - 24 * 3600e3).toISOString();
    ipc
      .spendSummary(day, "kind", "live")
      .then((aggs) => setSpentToday(aggs.reduce((a, b) => a + b.cost_usd, 0)))
      .catch(() => {});
    ipc
      .spendSummary(day, "workspace", "live")
      .then((aggs) =>
        setSpendByWs(Object.fromEntries(aggs.map((a) => [a.key, a.cost_usd]))),
      )
      .catch(() => {});
    // The chat header's cost comes from the LEDGER (today's hark-chat line),
    // not from a window-local accumulator — restarts don't zero it.
    ipc
      .spendSummary(day, "task", "live")
      .then((aggs) => {
        const chat = aggs.find((a) => a.key === "hark-chat");
        if (chat) setChatCost(chat.cost_usd);
      })
      .catch(() => {});
  }, []);
  useEffect(refresh, [refresh]);

  // The mother NEVER touches the active context: it is a neutral observer.
  // The focus ledger (Rust) keeps the last PROJECT window as the spoken
  // default — glancing at the mother must not send work to the global ask.

  // Esc anywhere in this window: settings close first, then recording →
  // cut the capture, otherwise → cut the voice. Cmd+, opens settings.
  const settingsOpenRef = useRef(false);
  settingsOpenRef.current = settingsOpen;
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if ((e.metaKey || e.ctrlKey) && e.key === ",") {
        e.preventDefault();
        setSettingsOpen(true);
        return;
      }
      if (e.key !== "Escape") return;
      if (settingsOpenRef.current) setSettingsOpen(false);
      else if (recordingRef.current) ipc.hearStop().catch(() => {});
      else ipc.speakStop().catch(() => {});
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  useHarkEvents({
    labelFor: (id) => id,
    push,
    setMessages,
    setLiveWorkers: () => {},
    pushRaw: () => {},
    addCost: () => {},
    onWorkerExit: useCallback((taskId: string, reason?: string | null) => {
      if (taskId !== HARK_CHAT) return;
      const inflight = chatInFlightRef.current;
      if (!inflight || inflight.retried) return;
      // One retry, announced. hark_chat_send respawns the worker resuming
      // the same session, so the conversation's memory survives the crash.
      chatInFlightRef.current = { ...inflight, retried: true };
      push({
        who: "sys",
        text: `o chat caiu no meio da resposta${reason ? ` (${reason})` : ""} — reabrindo a sessão e reenviando`,
      });
      ipc
        .harkChatSend(inflight.text)
        .then(() => setChatLive(true))
        .catch((err) => push({ who: "sys", text: `chat hark: ${err}` }));
    }, []),
    onSessionStarted: () => {},
    onSpeaking: setSpeaking,
    onRateLimit: setRateLimit,
    announce: true,
    onVoiceAction: useCallback(
      (utterance: string, target?: string | null, status?: string) =>
        setActions((old) => [...old, { utterance, target, status, ts: Date.now() }].slice(-8)),
      [],
    ),
    // A finished turn flips the feed row of its task: dispatched → done.
    // Hark-chat turns also feed the chat header (session cost + context).
    onWorkerTurn: useCallback(
      (
        label: string,
        isError: boolean,
        taskId?: string,
        ctxPct?: number | null,
        cost?: number,
        sessionId?: string | null,
      ) => {
        setActions((old) =>
          old.map((a) =>
            a.target?.toLowerCase() === label.toLowerCase() && a.status === "despachado"
              ? { ...a, status: isError ? "✗ falhou" : "✓ concluído" }
              : a,
          ),
        );
        if (taskId === HARK_CHAT) {
          setChatLive(true);
          // The reply landed: nothing is in flight anymore.
          chatInFlightRef.current = null;
          // Cost comes from the ledger on the refresh this turn triggers.
          if (ctxPct != null) setChatCtx(ctxPct);
          return;
        }
        // A chat finished elsewhere: the next sentence here may answer
        // the offer. Failures announce but never offer — "faz algo lá"
        // on a broken turn is a decision, not a follow-up.
        if (!isError && sessionId) {
          followUpRef.current = { label, sessionId };
        }
      },
      [],
    ),
    onHotkeyMic: useCallback(() => {
      setTab("voz");
      micRef.current();
    }, []),
    onMainTab: useCallback((t: string) => {
      if (t === "board" || t === "custos") setTab(t);
      // Other windows/HUD redirect here: app settings live on the mother.
      if (t === "settings") setSettingsOpen(true);
    }, []),
    // The chat speaks its ACTUAL reply (first sentence), not "task done".
    // Other chats finishing get the butler's offer — spoken through the
    // same serialized TTS, so it always waits for the current sentence.
    turnSpeech: useCallback((taskId: string, text: string, isError: boolean, label: string) => {
      if (taskId === HARK_CHAT) {
        if (isError) return undefined;
        const sentence = firstSentence(text);
        return sentence || undefined;
      }
      if (isError) return undefined;
      return st("sp_fu_offer", { t: label });
    }, []),
    // Spoken turns handled by the HUD land in the unified thread too:
    // the question always; the lean reply when the ask answered it.
    onChatEcho: useCallback(
      (
        question: string,
        reply: { fala: string; cost_usd?: number; model?: string } | undefined,
        work: boolean,
      ) => {
        push({ who: "user", text: question, task: work ? HARK_CHAT : undefined });
        // Work handed off by the HUD is in flight in the same chat: cover
        // it with the same crash-resend guarantee as typed messages.
        if (work) chatInFlightRef.current = { text: question, retried: false };
        if (reply)
          push({ who: "hark", text: reply.fala, cost: reply.cost_usd, model: reply.model });
      },
      [push],
    ),
    speakRef,
    refresh,
  });

  const say = useCallback((text: string) => {
    ipc.speak(text).catch(() => {});
  }, []);

  function openProject(p: Project, task?: { title: string; session?: string }) {
    ipc
      .openProjectWindow(p.name, p.path, task?.title, task?.session)
      .catch((err) => push({ who: "sys", text: `janela: ${err}` }));
  }

  /**
   * Recover an existing session: it becomes a board task named after the
   * SESSION and opens in its project's window, chat loaded. All local —
   * the index already knows every session on this machine.
   */
  async function recoverSession(hit: SessionHit) {
    setPicks(null);
    // A board candidate may arrive without a session: the index finds it
    // by title before the task adoption.
    let sessionId = hit.session_id;
    if (!sessionId) {
      const found = await ipc.findSession(hit.title).catch(() => null);
      if (!found) {
        push({ who: "sys", text: `sem sessão pra "${hit.title}"` });
        return;
      }
      sessionId = found.session_id;
    }
    let task: { title: string; workspace?: string | null };
    try {
      task = await ipc.taskFromSession(sessionId);
    } catch (err) {
      push({ who: "sys", text: `não consegui registrar a sessão: ${err}` });
      return;
    }
    const ws = task.workspace;
    const project = ws
      ? (overview?.projects ?? []).find((p) => ws === p.path || ws.startsWith(`${p.path}/`))
      : undefined;
    refresh();
    if (!project) {
      push({
        who: "sys",
        text: `"${task.title}" virou task, mas ${ws ?? "o diretório dela"} não é um projeto registrado — adiciona ele pra abrir o chat`,
      });
      say(`Criei a task ${task.title}, mas o projeto dela não está registrado.`);
      return;
    }
    openProject(project, { title: task.title, session: sessionId });
    say(st("sp_resuming_in", { t: task.title, p: project.name }));
  }

  /**
   * A card click on the global board: open the window of the project that
   * task belongs to, with its chat already loaded — back to work in one
   * click, no hunting for the session.
   */
  async function openTask(t: BoardTask) {
    const resolve = (dir?: string | null) =>
      dir
        ? (overview?.projects ?? []).find((p) => dir === p.path || dir.startsWith(`${p.path}/`))
        : undefined;
    let project = resolve(t.workspace);
    let session = t.session_ids.at(-1);
    if (!project) {
      // Older tasks carry no workspace: the session's own cwd says where
      // the work lives.
      const hit = await ipc.findSession(t.title).catch(() => null);
      project = resolve(hit?.cwd);
      session = session ?? hit?.session_id;
    }
    if (!project) {
      push({
        who: "sys",
        text: `"${t.title}" não aponta pra nenhum projeto registrado; adiciona o projeto pra abrir o chat`,
      });
      say("Essa task não tem projeto registrado.");
      return;
    }
    openProject(project, { title: t.title, session });
  }

  /** Local commands (zero tokens) the mother can execute herself. */
  async function runCommand(text: string): Promise<boolean> {
    const cmd = await ipc.taskCommand(text).catch(() => null);
    if (!cmd) return false;
    push({ who: "user", text });
    if (cmd.kind === "open_project") {
      openProject({ name: cmd.title, path: cmd.path });
      if (cmd.instruction) {
        try {
          await ipc.chatStart(cmd.path, cmd.instruction);
          say(st("sp_opening_and_work", { t: cmd.title }));
        } catch (err) {
          push({ who: "sys", text: `chat: ${err}` });
          say(st("sp_opened_chat_failed", { t: cmd.title }));
        }
      } else {
        say(st("sp_opening_project", { t: cmd.title }));
      }
    } else if (cmd.kind === "new_chat") {
      openProject({ name: cmd.title, path: cmd.path });
      if (cmd.instruction) {
        try {
          await ipc.chatStart(cmd.path, cmd.instruction);
          say(st("sp_chat_started", { t: cmd.title }));
        } catch (err) {
          push({ who: "sys", text: `chat: ${err}` });
          say("O chat não subiu, olha a janela.");
        }
      } else {
        say(`Abri ${cmd.title}. Diga a primeira tarefa lá.`);
      }
    } else if (cmd.kind === "open_hq") {
      setTab(cmd.tab);
      say(cmd.tab === "custos" ? "Custos na tela." : st("sp_board_screen"));
    } else if (cmd.kind === "project_added") {
      push({ who: "sys", text: `projeto ${cmd.title} adicionado (${cmd.path})` });
      say(st("sp_project_added", { t: cmd.title }));
      refresh();
      openProject({ name: cmd.title, path: cmd.path });
    } else if (cmd.kind === "project_error") {
      push({ who: "sys", text: cmd.title });
      say("Não consegui adicionar esse projeto.");
    } else if (cmd.kind === "open_file") {
      // The mother has no editor: send the user to the project window.
      const target = cmd.project
        ? (overview?.projects ?? []).find((p) =>
            p.name.toLowerCase().includes(cmd.project!.toLowerCase()),
          )
        : undefined;
      if (target) {
        openProject(target);
        say(st("sp_file_in_window", { p: target.name }));
      } else {
        say(st("sp_file_which_project"));
      }
    } else if (cmd.kind === "session_candidates") {
      if (cmd.candidates.length === 0) {
        push({ who: "sys", text: `nenhuma sessão fala sobre "${cmd.query}"` });
        say("Não achei sessão sobre isso.");
      } else if (cmd.candidates.length === 1) {
        await recoverSession(cmd.candidates[0]);
      } else {
        setTab("voz");
        setPicks({ query: cmd.query, candidates: cmd.candidates });
        say(`Achei ${cmd.candidates.length} sessões. Qual delas?`);
      }
    } else if (cmd.kind === "open" || cmd.kind === "switch") {
      // Going (back) to a chat opens ITS project window — the mother is
      // an observer, work never happens here.
      await recoverSession({ session_id: cmd.session_id ?? "", title: cmd.title });
    } else if (cmd.kind === "task_candidates") {
      setTab("voz");
      setPicks({
        query: cmd.query,
        candidates: cmd.candidates.map((c) => ({
          session_id: c.session_id ?? "",
          title: c.title,
          cwd: c.workspace,
        })),
      });
      say(st("sp_found_tasks", { n: cmd.candidates.length }));
    } else if (cmd.kind === "open_settings") {
      setSettingsOpen(true);
      say("Configurações na tela.");
    } else if (cmd.kind === "compact" || cmd.kind === "set_mode") {
      say("Isso é na janela do chat focado.");
      push({ who: "sys", text: "compactar/modo agem no chat focado — abre a janela dele" });
    } else if (cmd.kind === "not_found") {
      push({ who: "sys", text: `nada bate com "${cmd.query}"` });
      say("Não achei esse projeto.");
    } else {
      // Board bookkeeping (rename/pin/archive) shows its result on the
      // board tab right here.
      setTab("board");
      refresh();
      say(st("sp_done_board"));
    }
    return true;
  }

  /** Newest permission still undecided (any window's worker). */
  const pendingPerm = [...messages]
    .reverse()
    .find((m) => m.who === "permission" && !m.decision);

  async function answerPermission(requestId: string, allow: boolean) {
    setMessages((old) =>
      old.map((m) =>
        m.who === "permission" && m.requestId === requestId
          ? { ...m, decision: allow ? "allow" : "deny" }
          : m,
      ),
    );
    await ipc.approve(requestId, allow).catch(() => {});
  }

  async function submit(text: string) {
    if (!text.trim() || busy) return;
    setInput("");
    // A pending permission + a clean typed verdict = the answer. ONE
    // grammar (domain::verdict) — "assim que der" is NOT a yes.
    if (pendingPerm?.who === "permission") {
      const verdict = await ipc.interpretVerdict(text).catch(() => null);
      if (verdict?.kind === "confirm") {
        await answerPermission(pendingPerm.requestId, true);
        say(st("sp_allowed"));
        return;
      }
      if (verdict?.kind === "deny") {
        await answerPermission(pendingPerm.requestId, false);
        say(st("sp_denied"));
        return;
      }
    }
    // A pending butler offer owns the NEXT sentence — and only it. An
    // unrelated sentence clears the offer and flows on untouched.
    {
      const offer = followUpRef.current;
      if (offer) {
        followUpRef.current = null;
        const fu = await ipc.interpretFollowup(text, offer.label).catch(() => null);
        if (fu?.kind === "go") {
          push({ who: "user", text });
          say(st("sp_fu_going", { t: offer.label }));
          await recoverSession({ session_id: offer.sessionId, title: offer.label });
          return;
        }
        if (fu?.kind === "do" && fu.instruction) {
          push({ who: "user", text });
          await ipc
            .voiceExecute({ instruction: fu.instruction, session_id: offer.sessionId, new_task: false })
            .catch((err) => push({ who: "sys", text: `dispatch: ${err}` }));
          say(st("sp_fu_doing", { t: offer.label }));
          return;
        }
        if (fu?.kind === "stay") {
          push({ who: "user", text });
          say(st("sp_fu_stay"));
          return;
        }
      }
    }
    if (await runCommand(text)) return;
    // ACTION verbs never fall into the ask pipeline (the 19/08 incident:
    // "roda essa verificação de DNS" burned tokens on a refusal). Plan
    // the destination and confirm — same machinery as the HUD.
    const route = await ipc.routeText(text).catch(() => "ask");
    // Surface router: work needing external tools or producing content
    // goes to the PERSISTENT chat (full settings + MCP); questions stay
    // on the cheap bare ask. One visual thread either way.
    const lane = await ipc.askLane(text).catch(() => "lean");
    if (route === "dispatch") {
      push({ who: "user", text });
      const plan = await ipc.planUtterance(text).catch(() => null);
      if (plan?.kind === "work") {
        const target = plan.task_title ?? plan.project_name ?? "novo chat";
        setPendingPlan({ text, plan });
        say(st("sp_confirm_to", { target }));
        return;
      }
      if (plan?.kind === "candidates") {
        setTab("voz");
        setPicks({
          query: text,
          candidates: plan.options.map((o) => ({
            session_id: o.session_id ?? "",
            title: o.title,
            cwd: o.workspace,
          })),
        });
        say(st("sp_found_dests", { n: plan.options.length }));
        return;
      }
      // Work with NO project target: the 21/08 Slack case. That is the
      // mother's own work — it goes to the persistent chat, never dies.
      if (lane === "work") {
        await sendToChat(text);
        return;
      }
      // Before giving up on a target, let the classifier look at the
      // catalog: "atualiza o artefato daquela conversa" has a target, it
      // just is not spelled the way the grammar expects.
      if (await tryIntent(text)) return;
      push({ who: "sys", text: "sem alvo — fale \"na task X\" ou abra a janela do projeto" });
      say("Não achei o alvo pra esse trabalho.");
      return;
    }
    push({ who: "user", text });
    if (lane === "work") {
      await sendToChat(text);
      return;
    }
    // A sentence the grammar did not recognise is not automatically a
    // question. Placing it costs ~$0.01 on the light tier; answering the
    // wrong thing cost ~$0.04 and accomplished nothing, ten times over.
    if (await tryIntent(text)) return;
    setBusy("perguntando…");
    try {
      const reply = await ipc.askText(text);
      // The thread IS the record — no duplicate feed row for turns.
      push({ who: "hark", text: reply.fala, cost: reply.cost_usd, model: reply.model });
      say(reply.fala);
    } catch (err) {
      push({ who: "sys", text: `erro: ${agentError(err)}` });
    } finally {
      setBusy(null);
      refresh();
    }
  }

  /** Hand real work to the persistent chat worker; its turns stream back
   *  into the same thread as events (task_id "hark-chat"). Non-blocking:
   *  the input stays free while the worker runs. */
  /**
   * The fallback that used to be prose.
   *
   * Word lists cannot read natural speech: you cannot predict word order,
   * which conjugation someone reaches for, or how much context a request
   * carries. Anything they missed became a paraphrase at four cents. This
   * asks the LIGHT model to PLACE the sentence against the real catalog
   * of projects and sessions, then acts. Returns true when it acted.
   */
  async function tryIntent(text: string): Promise<boolean> {
    // The last exchanges are what make "eu disse sim" resolvable at all.
    const recent = messages
      .slice(-6)
      // Tool rows are machinery, not conversation.
      .map((m: Msg) => `${m.who}: ${("text" in m ? m.text : "").slice(0, 200)}`);
    const intent = await ipc.classifyUtterance(text, recent).catch(() => null);
    if (!intent || intent.kind === "question") return false;

    if (intent.kind === "clarify" && intent.options.length > 0) {
      setTab("voz");
      setPicks({
        query: text,
        candidates: intent.options.map((o) => ({
          session_id: o.session_id,
          title: o.title,
          cwd: o.project ?? undefined,
        })),
      });
      const question = intent.question ?? st("sp_found_dests", { n: intent.options.length });
      push({ who: "hark", text: question, cost: intent.cost_usd ?? undefined });
      say(question);
      return true;
    }

    if (intent.kind === "status") {
      const workers = overview?.workers ?? [];
      const running = workers.filter((w) => w.status === "running");
      const line = (() => {
        if (intent.session_id) {
          const w = workers.find((x) => x.session_id === intent.session_id);
          return w
            ? st("sp_status_one", {
                t: w.summary,
                s: w.status === "running" ? st("sp_status_running") : st("sp_status_done"),
              })
            : st("sp_status_none");
        }
        if (running.length === 0) return st("sp_status_none");
        if (running.length === 1)
          return st("sp_status_one", { t: running[0].summary, s: st("sp_status_running") });
        return st("sp_status_many", {
          n: running.length,
          list: running.map((w) => w.summary).slice(0, 3).join("; "),
        });
      })();
      push({ who: "hark", text: line, cost: intent.cost_usd ?? undefined });
      say(line);
      return true;
    }

    if (intent.kind === "open_project" && intent.project) {
      const project = (overview?.projects ?? []).find(
        (p) => p.name.toLowerCase() === intent.project!.toLowerCase(),
      );
      if (!project) return false;
      openProject(project);
      if (intent.instruction) {
        try {
          await ipc.chatStart(project.path, intent.instruction);
          say(st("sp_opening_and_work", { t: project.name }));
        } catch (err) {
          push({ who: "sys", text: `chat: ${err}` });
        }
      } else {
        say(st("sp_opening_project", { t: project.name }));
      }
      return true;
    }

    if (intent.kind === "open_session" && intent.session_id) {
      await recoverSession({
        session_id: intent.session_id,
        title: intent.session_title ?? text,
      });
      // Opening AND working is one sentence for the user, so it is one
      // step here: the instruction rides into the session just opened.
      if (intent.instruction) {
        await ipc
          .voiceExecute({
            instruction: intent.instruction,
            session_id: intent.session_id,
            new_task: false,
          })
          .catch((err) => push({ who: "sys", text: `dispatch: ${err}` }));
      }
      return true;
    }

    if (intent.kind === "dispatch" && intent.instruction && intent.session_id) {
      await ipc
        .voiceExecute({
          instruction: intent.instruction,
          session_id: intent.session_id,
          new_task: false,
        })
        .catch((err) => push({ who: "sys", text: `dispatch: ${err}` }));
      say(st("sp_confirm_to", { target: intent.session_title ?? "a sessão em foco" }));
      return true;
    }

    return false;
  }

  async function sendToChat(text: string) {
    try {
      chatInFlightRef.current = { text, retried: false };
      await ipc.harkChatSend(text);
      setChatLive(true);
    } catch (err) {
      chatInFlightRef.current = null;
      push({ who: "sys", text: `chat hark: ${agentError(err)}` });
    }
  }

  /** ONE voice surface: the mother's mic/orb opens the global HUD too. */
  async function onMic() {
    await ipc.hudShow().catch((err) => push({ who: "sys", text: `voz: ${err}` }));
  }
  micRef.current = onMic;

  const mode: OrbMode = recording ? "listening" : speaking ? "speaking" : busy ? "busy" : "idle";
  const liveWorkers = (overview?.workers ?? []).filter((w) => w.status === "running").length;

  // Shared blocks: the compact column and the expanded split reuse them.
  const orbBlock = (
    <div className="mother-orb" onClick={onMic} title="clique ou fale (Esc corta)">
      <VoiceOrb mode={mode} />
    </div>
  );
  // The assistant GREETS — identity first, telemetry demoted below it.
  const hour = new Date().getHours();
  const greetKey = hour < 12 ? "greet_morning" : hour < 18 ? "greet_afternoon" : "greet_evening";
  const greetBlock = (
    <div className="mother-greet">
      {t(greetKey)}
      {overview?.user_name ? `, ${overview.user_name}.` : "."}
    </div>
  );
  const statusBlock = (
    <div className="mother-status">
      {busy ?? (recording ? "ouvindo… (Esc corta)" : speaking ? "falando…" : "pronto")}
      {spentToday != null && (
        <span className="mother-spend"> · hoje ${spentToday.toFixed(2)}</span>
      )}
      {liveWorkers > 0 && (
        <span className="mother-workers">
          {" "}
          · {liveWorkers} worker{liveWorkers === 1 ? "" : "s"} ativo
          {liveWorkers === 1 ? "" : "s"}
        </span>
      )}
      {rateLimit && rateLimit.status !== "allowed" && (
        <span className="warn">
          {" "}
          · {rateLimit.status === "rejected" ? "limite atingido" : "quase no limite"}
        </span>
      )}
    </div>
  );
  const assistantName = overview?.assistant_name || "Hark";
  const chatHead = (expanded: boolean) => (
    <div className={expanded ? "chat-head" : "hark-chat-head"}>
      <span className="hark-chat-title">{assistantName}</span>
      <span className="hark-chat-meta">
        {chatLive ? t("chat_session_live") : t("chat_session_new")}
        {chatCost > 0 && ` · $${chatCost.toFixed(2)} ${t("proj_today")}`}
        {chatCtx != null && ` · ${t("chat_ctx", { n: Math.round(chatCtx * 100) })}`}
      </span>
      <button onClick={() => setChatExpanded(!expanded)}>
        {expanded ? <Minimize2 size={11} /> : <Maximize2 size={11} />}
        {expanded ? t("chat_collapse") : t("chat_expand")}
      </button>
    </div>
  );
  const digestRows = buildDigest(chatMsgs);
  const feedBlock = (actions.length > 0 || pendingPerm) && (
    <div className="mother-feed">
      {actions.slice(-4).map((a) => (
        <div key={a.ts} className="mother-action">
          <Mic size={13} className="mother-action-icon" />
          <span className="mother-action-text">“{a.utterance}”</span>
          <span className="mother-action-target">
            → {a.target ?? "hark"} · {a.status ?? "ok"}
          </span>
        </div>
      ))}
      {pendingPerm?.who === "permission" && (
        <div className="mother-action perm">
          <Lock size={13} className="mother-action-icon" />
          <span className="mother-action-text">
            {pendingPerm.tool} pede permissão
            {pendingPerm.task ? ` em ${pendingPerm.task.slice(0, 26)}` : ""}
            {pendingPerm.prodRisk ? ` — ⚠ ${pendingPerm.prodRisk}` : ""}
          </span>
          <span className="mother-action-target">fale “pode” ou “nega”</span>
        </div>
      )}
    </div>
  );
  const planBlock = pendingPlan && (
    <div className="mother-plan">
      <div className="mother-picks-head">
        {t("plan_to")}{" "}
        <b>
          {pendingPlan.plan.task_title ??
            `novo chat em ${pendingPlan.plan.project_name ?? "?"}`}
        </b>
        ?
      </div>
      <pre>{pendingPlan.plan.instruction}</pre>
      <div className="row">
        <button className="plain" onClick={() => setPendingPlan(null)}>
          {t("m_cancel")}
        </button>
        <button
          className="allow"
          onClick={async () => {
            const { plan } = pendingPlan;
            setPendingPlan(null);
            try {
              await ipc.voiceExecute(plan);
              say(st("sp_dispatched"));
            } catch (err) {
              push({ who: "sys", text: `despacho: ${err}` });
            }
          }}
        >
          {t("m_confirm")}
        </button>
      </div>
    </div>
  );
  const picksBlock = picks && (
    <div className="mother-picks">
      <div className="mother-picks-head">
        {t("picks_about", { q: picks.query })}
        <button onClick={() => setPicks(null)}>{t("m_close")}</button>
      </div>
      {picks.candidates.map((c) => (
        <button key={c.session_id} onClick={() => recoverSession(c)}>
          <b>{c.title}</b>
          <span>
            {c.last_ts ? c.last_ts.slice(0, 10) : ""}
            {c.cwd ? ` · ${c.cwd.split("/").filter(Boolean).pop()}` : ""}
          </span>
        </button>
      ))}
    </div>
  );
  const projectsBlock = (
    <div className="mother-projects">
      {(overview?.projects ?? []).map((p) => {
        const live = (overview?.workers ?? []).filter(
          (w) =>
            w.status === "running" &&
            (w.workspace === p.path || w.workspace.startsWith(`${p.path}/`)),
        ).length;
        const spent = Object.entries(spendByWs)
          .filter(([ws]) => ws === p.path || ws.startsWith(`${p.path}/`))
          .reduce((a, [, v]) => a + v, 0);
        return (
          <button
            key={p.path}
            className="mother-proj-card"
            onClick={() => openProject(p)}
            title={p.path}
          >
            <span className="mother-proj-name">
              <ExternalLink size={12} /> {p.name}
            </span>
            <span className="mother-proj-meta">
              {live > 0 && <span className="live-dot">◍ {live}</span>}
              {spent > 0 && ` $${spent.toFixed(2)} ${t("proj_today")}`}
              {live === 0 && spent === 0 && t("proj_quiet")}
            </span>
          </button>
        );
      })}
      {addingProject ? (
        <input
          className="mother-proj-add-input"
          autoFocus
          placeholder={t("proj_add_placeholder")}
          onBlur={() => setAddingProject(false)}
          onKeyDown={async (e) => {
            if (e.key === "Escape") setAddingProject(false);
            if (e.key !== "Enter") return;
            const value = (e.target as HTMLInputElement).value.trim();
            if (!value) return;
            setAddingProject(false);
            try {
              const entry = await ipc.projectAdd(value);
              push({ who: "sys", text: `projeto ${entry.name} adicionado (${entry.path})` });
              refresh();
              openProject(entry);
            } catch (err) {
              push({ who: "sys", text: `projeto: ${err}` });
            }
          }}
        />
      ) : (
        <button
          className="mother-proj-card mother-proj-add"
          onClick={() => setAddingProject(true)}
          title={t("proj_add_title")}
        >
          <span className="mother-proj-name">{t("proj_add")}</span>
          <span className="mother-proj-meta">{t("proj_add_hint")}</span>
        </button>
      )}
    </div>
  );

  return (
    <div
      className={
        tab !== "voz"
          ? "mother mother-wide"
          : chatExpanded
            ? "mother mother-splitwrap"
            : "mother"
      }
    >
      {setup && <Onboarding status={setup} onClose={() => setSetup(null)} />}
      {updateReady && (
        <button
          className="update-pill"
          title={t("up_restart")}
          onClick={() => restartIntoUpdate().catch(() => setUpdateReady(null))}
        >
          {t("up_ready", { v: updateReady })} · {t("up_restart")}
        </button>
      )}
      <nav className="tabs mother-tabs">
        <button className={tab === "voz" ? "active" : ""} onClick={() => setTab("voz")}>
          {t("tab_voice")}
        </button>
        <button className={tab === "board" ? "active" : ""} onClick={() => setTab("board")}>
          {t("tab_board")}
        </button>
        <button className={tab === "custos" ? "active" : ""} onClick={() => setTab("custos")}>
          {t("tab_costs")}
        </button>
        <button
          className="mother-gear"
          title={t("settings_btn")}
          onClick={() => setSettingsOpen(true)}
        >
          <SettingsIcon size={14} />
        </button>
      </nav>
      <Settings open={settingsOpen} onClose={() => setSettingsOpen(false)} />

      {tab === "board" ? (
        <Board
          tasks={overview?.board ?? []}
          projects={overview?.projects ?? []}
          onMove={(title, status) => ipc.boardMove(title, status).then(refresh).catch(() => {})}
          onOpen={openTask}
          onSubtask={(title, index, done) => ipc.boardSubtaskToggle(title, index, done).then(refresh).catch(() => {})}
        />
      ) : tab === "custos" ? (
        <div className="costs-page">
          <CostsPanel />
        </div>
      ) : (
        chatExpanded ? (
        // The user's saved mockup: split view. Left keeps the mother as it
        // is (orb + shortcuts, NO input); the chat's composer is THE input.
        <div className="mother-split">
          <div className="split-left">
            {orbBlock}
            {greetBlock}
            {statusBlock}
            {feedBlock}
            {planBlock}
            {picksBlock}
            {projectsBlock}
          </div>
          <div className="split-right">
            {chatHead(true)}
            <Transcript
              messages={chatMsgs}
              directivesFor={() => undefined}
              onAnswerPermission={(id, allow) => void answerPermission(id, allow)}
              onOpenPath={(p) => void ipc.openExternal(p).catch(() => {})}
            />
            <div className="chat-composer">
              <input
                autoFocus
                placeholder={t("chat_placeholder")}
                value={input}
                onChange={(e) => setInput(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") submit(input);
                }}
                disabled={!!busy}
              />
              <button
                className={`mic ${recording ? "recording" : ""}`}
                onClick={onMic}
                title="falar"
              >
                <Mic size={16} />
              </button>
            </div>
          </div>
        </div>
      ) : (
        <>
          {orbBlock}
          {greetBlock}
          {statusBlock}
          {feedBlock}

          {/* The persistent chat, compact: a glanceable digest of the last
              two exchanges — full text only when expanded. The input below
              doubles as its composer. */}
          <div className="hark-chat">
            {chatHead(false)}
            {digestRows.length === 0 ? (
              <div className="vc-empty">{t("chat_empty")}</div>
            ) : (
              <div className="hark-digest">
                {digestRows.map((r) =>
                  r.kind === "tools" ? (
                    <div key={r.key} className="vd-row">
                      <span className="vd-label" />
                      <span className="vd-tools">
                        › {t("tools_ran", { n: r.n })}{" "}
                        <span className={r.err ? "err" : "ok"}>{r.err ? "✗" : "✓"}</span>
                      </span>
                    </div>
                  ) : (
                    <div key={r.key} className={`vd-row ${r.kind}`}>
                      <span className="vd-label">
                        {r.kind === "user" ? t("tag_you") : r.kind === "sys" ? "!" : assistantName}
                      </span>
                      <span className="vd-text">{r.text}</span>
                    </div>
                  ),
                )}
              </div>
            )}
          </div>

          {planBlock}
          {picksBlock}

          <div className="mother-input">
            <input
              placeholder={t("mother_input")}
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") submit(input);
              }}
              disabled={!!busy}
            />
            <button
              className={`mic ${recording ? "recording" : ""}`}
              onClick={onMic}
              title="falar"
            >
              <Mic size={16} />
            </button>
          </div>

          {/* Spoken suggestions: click = send. All three answer cheap. */}
          <div className="mother-sugs">
            {[t("sug_spend"), t("sug_running"), t("sug_board")].map((s) => (
              <button key={s} className="mother-sug" onClick={() => submit(s)}>
                “{s}”
              </button>
            ))}
          </div>

          {projectsBlock}
        </>
        )
      )}
    </div>
  );
}
