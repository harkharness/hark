import { useCallback, useEffect, useRef, useState } from "react";
import { emit, listen } from "@tauri-apps/api/event";
import { CircleQuestionMark, FolderPlus, Mic, ShieldCheck, Target } from "lucide-react";
import * as ipc from "./lib/ipc";
import { setLang, t } from "./lib/i18n";
import type { VoiceCandidate, VoicePlan, VoxEvent } from "./types";

type Stage =
  | { s: "listening" }
  | { s: "thinking"; text: string }
  | { s: "confirm"; text: string; plan: Extract<VoicePlan, { kind: "work" }> }
  /** Target too close to call: numbered options, never a silent guess.
   *  Empty instruction = picking just opens/fronts the chat. */
  | { s: "candidates"; text: string; instruction: string; options: VoiceCandidate[] }
  | { s: "running"; text: string; target: string }
  | { s: "note"; text: string; tone: "ok" | "warn" }
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

  const hide = useCallback(() => {
    window.clearTimeout(timer.current);
    busyRef.current = false;
    setStage({ s: "listening" });
    ipc.hudHide().catch(() => {});
  }, []);

  /** What the user said this round — feeds the mother's action log. */
  const lastText = useRef("");
  const record = useCallback((target: string, status: string) => {
    emit("vox", {
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
              await ipc.hearStop().catch(() => {});
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
    await ipc.speak(`Achei ${options.length}. Qual delas?`).catch(() => {});
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
      await ipc.approve(plan.request_id, plan.allow).catch(() => {});
      record(plan.label, plan.allow ? "permitido em voz" : "negado em voz");
      const said = plan.allow ? "Permitido." : "Negado.";
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
        record(
          "vox",
          `respondido em voz${reply.cost_usd ? ` · $${reply.cost_usd.toFixed(2)}` : ""}`,
        );
        // The spoken turn also draws in the mother's unified thread.
        emit("vox", {
          kind: "chat_echo",
          question: text,
          reply: { fala: reply.fala, cost_usd: reply.cost_usd, model: reply.model },
          work: false,
        }).catch(() => {});
        ipc.speak(reply.fala).catch(() => {});
        window.clearTimeout(timer.current);
        timer.current = window.setTimeout(hide, 6000);
      } catch (err) {
        finish(`erro: ${err}`, "warn", 2800);
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
        ? `Para ${target}, mas ${plan.warnings[0]}. Sigo, ou compacto antes?`
        : `Para ${target}. Confirmo?`;
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
      await ipc.voxChatSend(text);
      emit("vox", { kind: "chat_echo", question: text, work: true }).catch(() => {});
      record("vox", "despachado");
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
    setStage({ s: "listening" });
    let text = "";
    try {
      text = (await ipc.hearOnce("hud")).trim();
    } catch (err) {
      // Another surface holds the mic: take over once (cut + retry).
      if (String(err).includes("mic_busy")) {
        await ipc.hearStop().catch(() => {});
        await sleep(180);
        try {
          text = (await ipc.hearOnce("hud")).trim();
        } catch {
          hide();
          return;
        }
      } else {
        hide();
        return;
      }
    }
    if (!text) {
      hide();
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
    ipc.configRead().then((s) => setLang(s.values.ui_language)).catch(() => {});
    startRef.current();
    const un = listen<VoxEvent>("vox", (e) => {
      if (e.payload.kind !== "hud_listen") return;
      // Hotkey while something lingers (note/answer/candidates/confirm):
      // the user wants to talk again — drop the leftover and re-arm.
      const s = stageRef.current.s;
      if (s === "note" || s === "answer" || s === "candidates" || s === "confirm") {
        window.clearTimeout(timer.current);
        cancelRef.current = true;
        if (verdictListening.current) ipc.hearStop().catch(() => {});
        busyRef.current = false;
      }
      startRef.current();
    });
    return () => {
      un.then((f) => f());
    };
  }, []);

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      const st = stageRef.current;
      if (e.key === "Escape") {
        if (verdictListening.current) {
          cancelRef.current = true;
          ipc.hearStop().catch(() => {});
          hide();
        } else if (st.s === "listening") {
          ipc.hearStop().catch(() => {});
        } else {
          hide();
        }
      }
      if (e.key === "Enter" && st.s === "confirm") {
        window.clearTimeout(timer.current);
        cancelRef.current = true;
        if (verdictListening.current) ipc.hearStop().catch(() => {});
        execute(st.plan);
      }
      // Digits pick a candidate (1-based on screen).
      if (st.s === "candidates" && /^[1-9]$/.test(e.key)) {
        const cand = st.options[Number(e.key) - 1];
        if (cand) {
          cancelRef.current = true;
          if (verdictListening.current) ipc.hearStop().catch(() => {});
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
