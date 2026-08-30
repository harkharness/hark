import { useCallback, useEffect, useRef, useState } from "react";
import { emit, listen } from "@tauri-apps/api/event";
import { CircleQuestionMark, FolderPlus, Mic, ShieldCheck, Target } from "lucide-react";
import * as ipc from "./lib/ipc";
import { setLang, setSpeechLang, st, t } from "./lib/i18n";
import { agentError, isAuthError } from "./lib/format";
import type { VoiceCandidate, VoicePlan, HarkEvent } from "./types";

type Stage =
  | { s: "listening" }
  /** The model file is still being read — say so, with the clock running:
   *  on a cold Intel Mac this lasts long enough to read as broken. */
  | { s: "loading"; size: string | null; secs: number }
  /** Capture ended, whisper is grinding. On CPU this takes ~3x the length
   *  of the utterance, so it needs its own face and its own Esc. */
  | { s: "transcribing"; secs: number }
  | { s: "thinking"; text: string }
  | { s: "confirm"; text: string; plan: Extract<VoicePlan, { kind: "work" }> }
  /** Target too close to call: numbered options, never a silent guess.
   *  Empty instruction = picking just opens/fronts the chat. */
  | { s: "candidates"; text: string; instruction: string; options: VoiceCandidate[] }
  | { s: "running"; text: string; target: string }
  | { s: "note"; text: string; tone: "ok" | "warn" }
  /** External dictation: the mic is someone else's tool; type/dictate here. */
  | { s: "type" }
  | { s: "asking"; text: string }
  | { s: "answer"; text: string };

const CONFIRM_MS = 1600;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/**
 * The floating voice HUD: one global ear over every window. It shows the
 * transcription, then WHERE the sentence will land. Two speeds: the chat
 * on screen executes after a silent beat; anything search-resolved SPEAKS
 * the destination and waits for a spoken yes/no/rephrase. Windows are
 * views; this is the command surface.
 */
export default function Hud() {
  const [stage, setStage] = useState<Stage>({ s: "listening" });
  const stageRef = useRef(stage);
  stageRef.current = stage;
  const timer = useRef<number>(0);
  // ONE run at a time, whatever fires it (mount, StrictMode's double
  // mount in dev, hotkey re-shows). The UI stage is a label, not a lock.
  const busyRef = useRef(false);
  // Esc during a verdict listen: abort the round, not just the capture.
  const cancelRef = useRef(false);
  const verdictListening = useRef(false);
  // Each start() gets a generation; a stale loading-wait loop that wakes
  // up after a re-arm sees a newer generation and steps aside.
  const runSeq = useRef(0);
  // The 1s clock behind the transcribing stage; self-clears on stage change.
  const phaseTimer = useRef<number>(0);

  const hide = useCallback(() => {
    window.clearTimeout(timer.current);
    busyRef.current = false;
    setStage({ s: "listening" });
    ipc.hudHide().catch(() => {});
  }, []);

  /** What the user said this round — feeds the mother's action log. */
  const lastText = useRef("");
  const record = useCallback((target: string, status: string) => {
    emit("hark", {
      kind: "voice_action",
      utterance: lastText.current,
      target,
      status,
    }).catch(() => {});
  }, []);

  const finish = useCallback(
    (text: string, tone: "ok" | "warn", ms: number) => {
      setStage({ s: "note", text, tone });
      window.clearTimeout(timer.current);
      timer.current = window.setTimeout(hide, ms);
    },
    [hide],
  );

  const execute = useCallback(
    async (plan: Extract<VoicePlan, { kind: "work" }>) => {
      const target = plan.task_title ?? plan.project_name ?? "novo chat";
      setStage({ s: "running", text: plan.instruction, target });
      try {
        await ipc.voiceExecute(plan);
        finish(`→ ${target} · despachado`, "ok", 1400);
      } catch (err) {
        finish(`falhou: ${err}`, "warn", 3200);
      }
    },
    [finish],
  );

  /**
   * Hear ONE verdict utterance. Never armed while the app speaks (callers
   * await ipc.speak first — whisper would transcribe our own voice), takes
   * the mic over if another surface holds it, retries once on noise.
   */
  const listenVerdict = useCallback(
    async (
      labels: string[],
      actions: [string, string[]][] = [],
    ): Promise<ipc.VerdictOut | null> => {
      verdictListening.current = true;
      try {
        for (let round = 0; round < 2; round++) {
          let heard = "";
          try {
            heard = (await ipc.hearOnce("hud")).trim();
          } catch (err) {
            if (String(err).includes("mic_busy")) {
              // Taking over means the other surface's turn is dead — abort
              // it whole (its transcription would only waste the CPU).
              await ipc.hearAbort().catch(() => {});
              await sleep(180);
              continue;
            }
            return null;
          }
          if (cancelRef.current || !heard) return null;
          const verdict = await ipc.interpretVerdict(heard, labels, actions).catch(() => null);
          if (!verdict) return null;
          if (verdict.kind !== "unknown") return verdict;
          // Noise: one silent retry, then the keys/click take over.
        }
        return null;
      } finally {
        verdictListening.current = false;
      }
    },
    [],
  );

  /** Front the chat a pick landed on — or dispatch into it when the
   *  sentence carried work. The 19/08 bug was ending on focusMain(). */
  async function pickCandidate(instruction: string, cand: VoiceCandidate) {
    if (instruction) {
      await execute({
        kind: "work",
        instruction,
        task_title: cand.title,
        session_id: cand.session_id,
        workspace: cand.workspace,
        project_name: cand.project_name,
        new_task: false,
        confidence: "high",
        warnings: [],
      });
      return;
    }
    // No work to send: opening IS the action. Resolve the owning
    // project (adopts the session as a board task when needed).
    let ws = cand.workspace ?? null;
    let title = cand.title;
    if (!ws && cand.session_id) {
      const task = await ipc.taskFromSession(cand.session_id).catch(() => null);
      ws = task?.workspace ?? null;
      title = task?.title ?? title;
    }
    if (!ws) {
      finish("sessão sem projeto registrado", "warn", 2600);
      return;
    }
    const name = cand.project_name ?? ws.split("/").filter(Boolean).pop() ?? "projeto";
    await ipc
      .openProjectWindow(name, ws, title, cand.session_id ?? undefined)
      .catch(() => {});
    record(title, "aberta");
    finish(`→ ${title} · aberta`, "ok", 1400);
  }

  /** Show options, ask out loud, hear the pick ("a primeira", the name,
   *  "não", or a whole new sentence). Digits/click stay alive as fallback. */
  async function offerCandidates(text: string, instruction: string, options: VoiceCandidate[]) {
    setStage({ s: "candidates", text, instruction, options });
    await ipc.speak(st("sp_found_n_which", { n: options.length })).catch(() => {});
    await sleep(150);
    const verdict = await listenVerdict(options.map((o) => o.title));
    if (cancelRef.current || !verdict) return; // keys/click still live
    if (verdict.kind === "pick") {
      const cand = options[verdict.index];
      if (cand) await pickCandidate(instruction, cand);
    } else if (verdict.kind === "deny") {
      hide();
    } else if (verdict.kind === "instruction") {
      await handle(verdict.text);
    }
  }

  /** Local commands the HUD can run itself (zero tokens). */
  async function runCommand(cmd: import("./types").TaskCommandResult) {
    if (cmd.kind === "open_project" || cmd.kind === "new_chat") {
      await ipc.openProjectWindow(cmd.title, cmd.path).catch(() => {});
      if (cmd.instruction) {
        await ipc.chatStart(cmd.path, cmd.instruction).catch(() => {});
        record(cmd.title, "chat iniciado");
        finish(`→ ${cmd.title} · chat iniciado`, "ok", 1400);
      } else {
        record(cmd.title, "aberto");
        finish(`→ ${cmd.title} · aberto`, "ok", 1200);
      }
    } else if (cmd.kind === "open_hq") {
      await ipc.focusMain(cmd.tab).catch(() => {});
      finish(`→ ${cmd.tab} na janela mãe`, "ok", 1100);
    } else if (cmd.kind === "open" || cmd.kind === "switch") {
      // Going (back) to a chat fronts ITS project window — never the
      // mother (the old catch-all fronted the wrong window).
      await pickCandidate("", { title: cmd.title, session_id: cmd.session_id });
    } else if (cmd.kind === "task_candidates") {
      await offerCandidates(
        lastText.current,
        "",
        cmd.candidates.map((c) => ({
          title: c.title,
          session_id: c.session_id,
          workspace: c.workspace,
        })),
      );
    } else if (cmd.kind === "session_candidates") {
      if (cmd.candidates.length === 0) {
        finish(`nenhuma sessão sobre "${cmd.query}"`, "warn", 2600);
        return;
      }
      await offerCandidates(
        lastText.current,
        "",
        cmd.candidates.map((c) => ({
          title: c.title,
          session_id: c.session_id,
          workspace: c.cwd,
        })),
      );
    } else if (cmd.kind === "project_offer") {
      // Dirs on disk that match what was said: picking one registers it
      // and opens its window — the spoken intent, completed.
      setStage({
        s: "candidates",
        text: lastText.current,
        instruction: "",
        options: cmd.candidates.map((path) => ({
          title: path.split("/").pop() ?? path,
          session_id: undefined,
          workspace: path,
          project_name: undefined,
        })),
      });
      await ipc.speak(st("sp_found_dirs", { n: cmd.candidates.length })).catch(() => {});
      await sleep(150);
      const verdict = await listenVerdict(cmd.candidates.map((p) => p.split("/").pop() ?? p));
      if (cancelRef.current || !verdict) return;
      if (verdict.kind === "pick") {
        const path = cmd.candidates[verdict.index];
        if (!path) return;
        const entry = await ipc.projectAdd(path).catch(() => null);
        if (!entry) {
          finish("não consegui registrar o projeto", "warn", 2600);
          return;
        }
        await ipc.openProjectWindow(entry.name, entry.path).catch(() => {});
        record(entry.name, "projeto registrado");
        finish(`→ ${entry.name} · registrado e aberto`, "ok", 1500);
      } else if (verdict.kind === "deny") {
        hide();
      } else if (verdict.kind === "instruction") {
        await handle(verdict.text);
      }
    } else if (cmd.kind === "project_added") {
      // Registering IS half the intent — the window is the other half.
      await ipc.openProjectWindow(cmd.title, cmd.path).catch(() => {});
      record(cmd.title, "projeto registrado");
      finish(`→ ${cmd.title} · registrado e aberto`, "ok", 1500);
    } else if (cmd.kind === "project_error") {
      finish(`projeto: ${cmd.title}`, "warn", 3000);
    } else if (cmd.kind === "open_settings") {
      await ipc.focusMain("settings").catch(() => {});
      finish("→ configurações na janela mãe", "ok", 1200);
    } else if (cmd.kind === "set_mode") {
      // Mode switching needs a window's focused worker; the HUD has none.
      finish("troca de modo é na janela do chat — seletor ou /modo", "warn", 2600);
    } else if (cmd.kind === "not_found") {
      finish(`nada bate com "${cmd.query}"`, "warn", 2600);
    } else {
      // Board bookkeeping (rename/pin/archive) resolves on the backend;
      // just acknowledge.
      const title = "title" in cmd ? cmd.title : "";
      finish(`✓ ${cmd.kind}${title ? ` · ${title}` : ""}`, "ok", 1400);
    }
  }

  /** Route one utterance (fresh capture or a spoken rephrase). */
  async function handle(text: string) {
    lastText.current = text;
    setStage({ s: "thinking", text });
    let plan: VoicePlan;
    try {
      plan = await ipc.planUtterance(text);
    } catch (err) {
      finish(`não entendi: ${err}`, "warn", 2600);
      return;
    }
    if (plan.kind === "permission_answer") {
      // A card was waiting somewhere and the user just said "pode"/"nega".
      // No feed row: the verdict is not an action — the permission card
      // itself resolves everywhere and IS the durable record.
      await ipc.approve(plan.request_id, plan.allow).catch(() => {});
      // "sempre pode": the owning window records the standing rule.
      if (plan.allow && plan.always) {
        emit("hark", { kind: "allow_rule", label: plan.label, tool: plan.tool }).catch(() => {});
      }
      const said = plan.allow ? (plan.always ? st("sp_allowed_always") : st("sp_allowed")) : st("sp_denied");
      finish(`${plan.allow ? "✓" : "✗"} ${plan.tool} · ${plan.label}`, "ok", 1600);
      ipc.speak(said).catch(() => {});
    } else if (plan.kind === "command") {
      await runCommand(plan.command);
    } else if (plan.kind === "question") {
      // Surface router: needs external tools or produces content → the
      // mother's persistent chat (full settings + MCP). The bare ask has
      // neither — sending Slack work there was the 21/08 incident.
      const lane = await ipc.askLane(text).catch(() => "lean");
      if (lane === "work") {
        await sendToMotherChat(text);
        return;
      }
      setStage({ s: "asking", text });
      try {
        const reply = await ipc.askText(plan.question);
        setStage({ s: "answer", text: reply.fala });
        // No feed row: the thread IS the record for conversation turns —
        // the feed only narrates what happens OUTSIDE the chat.
        // The spoken turn draws in the mother's unified thread.
        emit("hark", {
          kind: "chat_echo",
          question: text,
          reply: { fala: reply.fala, cost_usd: reply.cost_usd, model: reply.model },
          work: false,
        }).catch(() => {});
        ipc.speak(reply.fala).catch(() => {});
        window.clearTimeout(timer.current);
        timer.current = window.setTimeout(hide, 6000);
      } catch (err) {
        // The spoken question is a conversation turn even when it fails:
        // it lands in the mother's thread with the coded error, instead of
        // evaporating with this toast (caught live 26/08 — an expired
        // OAuth left "nothing happened" and no trace).
        // One classifier for every surface (typed or spoken must fail
        // identically): auth errors echo WITH the runnable /login block.
        const auth = isAuthError(err);
        const fence = "```";
        const text_err = auth
          ? `${agentError(err)}\n\n${t("auth_fix")}\n\n${fence}bash\nclaude /login\n${fence}`
          : agentError(err);
        emit("hark", {
          kind: "chat_echo",
          question: text,
          reply: { fala: text_err },
          work: false,
        }).catch(() => {});
        if (auth) ipc.speak(st("sp_login_needed")).catch(() => {});
        finish(agentError(err), "warn", 3600);
      }
    } else if (plan.kind === "work") {
      setStage({ s: "confirm", text, plan });
      window.clearTimeout(timer.current);
      // Two speeds: the chat on screen (high) executes after a silent
      // beat; a search-resolved or brand-new target (low) — or ANY local
      // warning — is SPOKEN and waits for a verdict, never on silence.
      const hasWarnings = plan.warnings.length > 0;
      if (plan.confidence === "high" && !plan.new_task && !hasWarnings) {
        timer.current = window.setTimeout(() => execute(plan), CONFIRM_MS);
        return;
      }
      const target = plan.task_title ?? `novo chat em ${plan.project_name ?? "?"}`;
      const prompt = hasWarnings
        ? st("sp_confirm_warn", { target, warn: plan.warnings[0] })
        : st("sp_confirm_to", { target });
      await ipc.speak(prompt).catch(() => {});
      await sleep(150);
      const verdict = await listenVerdict(
        [],
        hasWarnings
          ? [["compact_first", ["compacta antes", "compactar antes", "compacta primeiro"]]]
          : [],
      );
      if (cancelRef.current || !verdict) return; // Enter/Esc still live
      if (verdict.kind === "confirm") await execute(plan);
      else if (verdict.kind === "action" && verdict.id === "compact_first") {
        const target2 = plan.task_title ?? plan.project_name ?? "novo chat";
        setStage({ s: "running", text: plan.instruction, target: target2 });
        try {
          await ipc.voiceExecute(plan, true);
          finish(`→ ${target2} · compactado e despachado`, "ok", 1600);
        } catch (err) {
          finish(`falhou: ${err}`, "warn", 3200);
        }
      } else if (verdict.kind === "deny") finish("cancelado", "warn", 1200);
      else if (verdict.kind === "instruction") await handle(verdict.text);
    } else if (plan.kind === "candidates") {
      await offerCandidates(text, plan.instruction, plan.options);
    } else {
      // The grammar found no target. Before admitting that, let the
      // light classifier read the sentence against the real catalog —
      // free when it is clearly not an order, ~1¢ when it could be.
      const intent = await ipc.classifyUtterance(text, []).catch(() => null);
      if (intent && intent.kind === "open_session" && intent.session_id) {
        await pickCandidate(intent.instruction ?? "", {
          title: intent.session_title ?? text,
          session_id: intent.session_id,
        });
        return;
      }
      if (intent && intent.kind === "dispatch" && intent.instruction && intent.session_id) {
        await pickCandidate(intent.instruction, {
          title: intent.session_title ?? "sessão em foco",
          session_id: intent.session_id,
        });
        return;
      }
      if (intent && intent.kind === "clarify" && intent.options.length > 0) {
        await offerCandidates(
          text,
          intent.instruction ?? "",
          intent.options.map((o) => ({ title: o.title, session_id: o.session_id })),
        );
        return;
      }
      // No project target: work that needs tools still has a home — the
      // mother's chat (the Slack case). Only then admit "no target".
      const lane = await ipc.askLane(text).catch(() => "lean");
      if (lane === "work") {
        await sendToMotherChat(text);
        return;
      }
      await ipc.speak("Não achei o alvo. Fala a task ou o projeto.").catch(() => {});
      finish('sem alvo — "na task X" ou "no projeto Y"', "warn", 300);
      await sleep(150);
      busyRef.current = false;
      start();
    }
  }

  /** Real work with no project target → the mother's persistent chat.
   *  The reply comes back as a worker turn: the mother draws it in the
   *  thread and SPEAKS its first sentence — the HUD just hands off. */
  async function sendToMotherChat(text: string) {
    setStage({ s: "running", text, target: "chat" });
    try {
      await ipc.harkChatSend(text);
      emit("hark", { kind: "chat_echo", question: text, work: true }).catch(() => {});
      ipc.speak("Mandei pro chat. Já te respondo.").catch(() => {});
      finish("→ chat · despachado", "ok", 1600);
    } catch (err) {
      finish(`chat: ${err}`, "warn", 3000);
    }
  }

  async function start() {
    if (busyRef.current) return;
    busyRef.current = true;
    cancelRef.current = false;
    const mine = ++runSeq.current;
    setStage({ s: "listening" });
    let text = "";
    const opened = Date.now();
    let busyRetries = 0;
    for (;;) {
      try {
        text = (await ipc.hearOnce("hud")).trim();
        break;
      } catch (err) {
        const raw = String(err);
        if (raw.includes("mic_busy")) {
          // Another surface holds the mic: take the turn over (abort it —
          // its transcription would only waste the CPU) and retry. The
          // abort lands between whisper's compute blocks, which on a slow
          // CPU is a matter of seconds — so retry for a while, not once.
          if (busyRetries++ >= 8) {
            hide();
            return;
          }
          await ipc.hearAbort().catch(() => {});
          await sleep(250);
        } else if (raw.includes("mic_loading")) {
          // Not an error — a wait. Keep asking with the clock on screen:
          // when the model lands, the very next call starts the capture.
          const size = raw.split("mic_loading:")[1]?.trim() || null;
          setStage({
            s: "loading",
            size,
            secs: Math.floor((Date.now() - opened) / 1000),
          });
          await sleep(1000);
        } else if (raw.includes("mic_external")) {
          // stt = "external": the HUD becomes a text field. Dictation apps
          // write wherever the caret is — this is the caret.
          setStage({ s: "type" });
          return;
        } else if (raw.includes("mic_no_model")) {
          setStage({ s: "note", text: t("mic_no_model"), tone: "warn" });
          setTimeout(hide, 3600);
          return;
        } else {
          // mic_aborted lands here too: Esc already closed the HUD.
          hide();
          return;
        }
        if (cancelRef.current || runSeq.current !== mine) {
          return; // Esc or a re-arm took over while we waited
        }
      }
    }
    if (cancelRef.current || runSeq.current !== mine || !text) {
      if (runSeq.current === mine) hide();
      return;
    }
    await handle(text);
  }
  const startRef = useRef(start);
  startRef.current = start;
  const pickRef = useRef(pickCandidate);
  pickRef.current = pickCandidate;

  // Every hotkey press re-arms the HUD; the first mount starts by itself.
  useEffect(() => {
    // UI language for this window (stage changes repaint with it).
    ipc.configRead().then((s) => { setLang(s.values.ui_language); setSpeechLang(s.values.language); }).catch(() => {});
    startRef.current();
    const un = listen<HarkEvent>("hark", (e) => {
      const p = e.payload;
      if (p.kind === "mic" && p.owner === "hud") {
        // Backend phase events drive the mic's face — but only while OUR
        // capture owns the screen (a verdict listen keeps its own stage).
        const s = stageRef.current.s;
        if (p.phase === "capturing" && (s === "listening" || s === "loading")) {
          setStage({ s: "listening" });
        } else if (p.phase === "transcribing" && s === "listening") {
          const started = Date.now();
          window.clearInterval(phaseTimer.current);
          setStage({ s: "transcribing", secs: 0 });
          phaseTimer.current = window.setInterval(() => {
            if (stageRef.current.s !== "transcribing") {
              window.clearInterval(phaseTimer.current);
              return;
            }
            setStage({
              s: "transcribing",
              secs: Math.floor((Date.now() - started) / 1000),
            });
          }, 1000);
        }
        return;
      }
      if (p.kind !== "hud_listen") return;
      // Hotkey while something lingers (note/answer/candidates/confirm) or
      // while whisper still grinds a dead turn: the user wants to talk
      // again — drop the leftover whole and re-arm.
      const s = stageRef.current.s;
      if (
        s === "note" || s === "answer" || s === "candidates" ||
        s === "confirm" || s === "transcribing"
      ) {
        window.clearTimeout(timer.current);
        cancelRef.current = true;
        if (verdictListening.current || s === "transcribing") {
          ipc.hearAbort().catch(() => {});
        }
        busyRef.current = false;
      }
      startRef.current();
    });
    return () => {
      window.clearInterval(phaseTimer.current);
      un.then((f) => f());
    };
  }, []);

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      const st = stageRef.current;
      if (e.key === "Escape") {
        if (verdictListening.current) {
          cancelRef.current = true;
          ipc.hearAbort().catch(() => {});
          hide();
        } else if (st.s === "listening") {
          // Still capturing: Esc ends the capture and KEEPS the words —
          // that is the promise printed next to the waveform.
          ipc.hearStop().catch(() => {});
        } else if (st.s === "transcribing" || st.s === "loading") {
          // The words no longer matter (or never existed): kill the turn
          // wherever it is and free the CPU.
          cancelRef.current = true;
          ipc.hearAbort().catch(() => {});
          hide();
        } else {
          hide();
        }
      }
      if (e.key === "Enter" && st.s === "confirm") {
        window.clearTimeout(timer.current);
        cancelRef.current = true;
        if (verdictListening.current) ipc.hearAbort().catch(() => {});
        execute(st.plan);
      }
      // Digits pick a candidate (1-based on screen).
      if (st.s === "candidates" && /^[1-9]$/.test(e.key)) {
        const cand = st.options[Number(e.key) - 1];
        if (cand) {
          cancelRef.current = true;
          if (verdictListening.current) ipc.hearAbort().catch(() => {});
          pickRef.current(st.instruction, cand);
        }
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [execute, hide]);

  const waveform = (
    <span className="hud-wave">
      <span /><span /><span /><span /><span />
    </span>
  );

  return (
    <div className="hud">
      {stage.s === "listening" && (
        <div className="hud-row">
          {waveform}
          <span className="hud-live">{t("hud_listening")}</span>
        </div>
      )}
      {stage.s === "loading" && (
        <div className="hud-row">
          {waveform}
          <span className="hud-text warn">
            {stage.size
              ? t("mic_loading_sized", { size: stage.size, secs: stage.secs })
              : t("mic_loading_plain", { secs: stage.secs })}
          </span>
        </div>
      )}
      {stage.s === "transcribing" && (
        <div className="hud-row">
          {waveform}
          <span className="hud-live">{t("hud_transcribing", { secs: stage.secs })}</span>
        </div>
      )}
      {stage.s === "thinking" && (
        <div className="hud-row">
          <Mic size={14} className="hud-icon" />
          <span className="hud-text">“{stage.text}”</span>
          <span className="hud-sub">{t("hud_routing")}</span>
        </div>
      )}
      {stage.s === "confirm" && (
        <>
          <div className="hud-row">
            <Mic size={14} className="hud-icon" />
            <span className="hud-text">“{stage.text}”</span>
          </div>
          <div className="hud-row hud-dest">
            <span className="hud-sub">{t("hud_goes_to")}</span>
            <span className={`hud-chip ${stage.plan.new_task ? "new" : ""}`}>
              {stage.plan.new_task ? <FolderPlus size={12} /> : <Target size={12} />}
              {stage.plan.task_title ?? t("hud_new_chat_in", { name: stage.plan.project_name ?? "?" })}
              {stage.plan.project_name && stage.plan.task_title
                ? ` · ${stage.plan.project_name}`
                : ""}
            </span>
            <span className="hud-keys">
              {stage.plan.confidence === "high" &&
              !stage.plan.new_task &&
              stage.plan.warnings.length === 0
                ? t("hud_keys_fast")
                : t("hud_keys_verdict")}
            </span>
          </div>
          {stage.plan.warnings.map((w) => (
            <div key={w} className="hud-row hud-warning">
              ⚠ {w} {t("hud_warn_hint")}
            </div>
          ))}
          {stage.plan.confidence === "high" &&
            !stage.plan.new_task &&
            stage.plan.warnings.length === 0 && (
              <div className="hud-progress" style={{ animationDuration: `${CONFIRM_MS}ms` }} />
            )}
        </>
      )}
      {stage.s === "candidates" && (
        <>
          <div className="hud-row">
            <Mic size={14} className="hud-icon" />
            <span className="hud-text">“{stage.text}”</span>
            <span className="hud-sub">{t("hud_which")}</span>
          </div>
          <div className="hud-cands">
            {stage.options.map((o, i) => (
              <button key={i} onClick={() => pickRef.current(stage.instruction, o)}>
                <i>{i + 1}</i>
                <span className="hud-cand-title">{o.title}</span>
                {o.project_name && <span className="hud-cand-proj">{o.project_name}</span>}
              </button>
            ))}
          </div>
        </>
      )}
      {stage.s === "running" && (
        <div className="hud-row">
          {waveform}
          <span className="hud-text">→ {stage.target}</span>
          <span className="hud-sub">{t("hud_dispatching")}</span>
        </div>
      )}
      {stage.s === "asking" && (
        <div className="hud-row">
          <CircleQuestionMark size={14} className="hud-icon" />
          <span className="hud-text">“{stage.text}”</span>
          <span className="hud-sub">{t("hud_asking")}</span>
        </div>
      )}
      {stage.s === "answer" && (
        <div className="hud-row">
          <span className="hud-text hud-answer">{stage.text}</span>
        </div>
      )}
      {stage.s === "type" && (
        <input
          className="hud-type"
          autoFocus
          placeholder={t("hud_type_placeholder")}
          onKeyDown={(e) => {
            if (e.key === "Escape") hide();
            if (e.key === "Enter") {
              const text = (e.target as HTMLInputElement).value.trim();
              if (text) void handle(text);
            }
          }}
        />
      )}
      {stage.s === "note" && (
        <div className="hud-row">
          <span className={`hud-text ${stage.tone}`}>
            {stage.text.startsWith("✓") && <ShieldCheck size={13} className="hud-icon" />}
            {stage.text}
          </span>
        </div>
      )}
    </div>
  );
}
