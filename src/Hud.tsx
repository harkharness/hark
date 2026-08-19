import { useCallback, useEffect, useRef, useState } from "react";
import { emit, listen } from "@tauri-apps/api/event";
import { CircleQuestionMark, FolderPlus, Mic, Target } from "lucide-react";
import * as ipc from "./lib/ipc";
import type { VoicePlan, VoxEvent } from "./types";

type Stage =
  | { s: "listening" }
  | { s: "thinking"; text: string }
  | { s: "confirm"; text: string; plan: Extract<VoicePlan, { kind: "work" }> }
  | { s: "running"; text: string; target: string }
  | { s: "note"; text: string; tone: "ok" | "warn" }
  | { s: "asking"; text: string }
  | { s: "answer"; text: string };

const CONFIRM_MS = 1600;

/**
 * The floating voice HUD: one global ear over every window. It shows the
 * transcription, then WHERE the sentence will land — Enter confirms now,
 * Esc cancels, silence confirms after a beat. Windows are views; this is
 * the command surface.
 */
export default function Hud() {
  const [stage, setStage] = useState<Stage>({ s: "listening" });
  const stageRef = useRef(stage);
  stageRef.current = stage;
  const timer = useRef<number>(0);
  // ONE run at a time, whatever fires it (mount, StrictMode's double
  // mount in dev, hotkey re-shows). The UI stage is a label, not a lock:
  // its initial value IS "listening", so guarding on it let two captures
  // start in parallel — two transcriptions, two plans, two voices.
  const busyRef = useRef(false);

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

  /** Local commands the HUD can run itself (zero tokens). */
  const runCommand = useCallback(
    async (cmd: import("./types").TaskCommandResult) => {
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
      } else if (cmd.kind === "session_candidates") {
        const first = cmd.candidates[0];
        if (!first) {
          finish(`nenhuma sessão sobre "${cmd.query}"`, "warn", 2600);
          return;
        }
        const task = await ipc.taskFromSession(first.session_id).catch(() => null);
        if (task?.workspace) {
          const name = task.workspace.split("/").filter(Boolean).pop() ?? "projeto";
          await ipc
            .openProjectWindow(name, task.workspace, task.title, first.session_id)
            .catch(() => {});
          record(task.title, "retomada");
          finish(`→ ${task.title} · retomada`, "ok", 1500);
        } else {
          finish("sessão sem projeto registrado", "warn", 2600);
        }
      } else if (cmd.kind === "set_mode") {
        // Mode switching needs a window's focused worker; the HUD has none.
        finish("troca de modo é na janela do chat — seletor ou /modo", "warn", 2600);
      } else if (cmd.kind === "not_found") {
        finish(`nada bate com "${cmd.query}"`, "warn", 2600);
      } else {
        // Board actions (rename/pin/archive/switch/open) resolve on the
        // backend already; just acknowledge.
        const title = "title" in cmd ? cmd.title : "";
        await ipc.focusMain().catch(() => {});
        finish(`✓ ${cmd.kind}${title ? ` · ${title}` : ""}`, "ok", 1400);
      }
    },
    [finish],
  );

  const start = useCallback(async () => {
    if (busyRef.current) return;
    busyRef.current = true;
    setStage({ s: "listening" });
    let text = "";
    try {
      text = (await ipc.hearOnce()).trim();
    } catch {
      hide();
      return;
    }
    if (!text) {
      hide();
      return;
    }
    lastText.current = text;
    setStage({ s: "thinking", text });
    let plan: VoicePlan;
    try {
      plan = await ipc.planUtterance(text);
    } catch (err) {
      finish(`não entendi: ${err}`, "warn", 2600);
      return;
    }
    if (plan.kind === "command") {
      await runCommand(plan.command);
    } else if (plan.kind === "question") {
      setStage({ s: "asking", text });
      try {
        const reply = await ipc.askText(plan.question);
        setStage({ s: "answer", text: reply.fala });
        record(
          "vox",
          `respondido em voz${reply.cost_usd ? ` · $${reply.cost_usd.toFixed(2)}` : ""}`,
        );
        ipc.speak(reply.fala).catch(() => {});
        window.clearTimeout(timer.current);
        timer.current = window.setTimeout(hide, 6000);
      } catch (err) {
        finish(`erro: ${err}`, "warn", 2800);
      }
    } else if (plan.kind === "work") {
      setStage({ s: "confirm", text, plan });
      window.clearTimeout(timer.current);
      timer.current = window.setTimeout(() => execute(plan), CONFIRM_MS);
    } else {
      finish('sem alvo — fale "na task X" ou "no projeto Y"', "warn", 3000);
    }
  }, [execute, finish, hide, runCommand]);

  // Every hotkey press re-arms the HUD; the first mount starts by itself.
  useEffect(() => {
    start();
    const un = listen<VoxEvent>("vox", (e) => {
      if (e.payload.kind !== "hud_listen") return;
      // Hotkey while a note/answer still lingers: the user wants to talk
      // again — drop the leftover and re-arm.
      const s = stageRef.current.s;
      if (s === "note" || s === "answer") {
        window.clearTimeout(timer.current);
        busyRef.current = false;
      }
      start();
    });
    return () => {
      un.then((f) => f());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      const st = stageRef.current;
      if (e.key === "Escape") {
        if (st.s === "listening") ipc.hearStop().catch(() => {});
        else hide();
      }
      if (e.key === "Enter" && st.s === "confirm") {
        window.clearTimeout(timer.current);
        execute(st.plan);
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
          <span className="hud-live">ouvindo… fale (Esc encerra a captura)</span>
        </div>
      )}
      {stage.s === "thinking" && (
        <div className="hud-row">
          <Mic size={14} className="hud-icon" />
          <span className="hud-text">“{stage.text}”</span>
          <span className="hud-sub">roteando…</span>
        </div>
      )}
      {stage.s === "confirm" && (
        <>
          <div className="hud-row">
            <Mic size={14} className="hud-icon" />
            <span className="hud-text">“{stage.text}”</span>
          </div>
          <div className="hud-row hud-dest">
            <span className="hud-sub">vai para</span>
            <span className={`hud-chip ${stage.plan.new_task ? "new" : ""}`}>
              {stage.plan.new_task ? <FolderPlus size={12} /> : <Target size={12} />}
              {stage.plan.task_title ?? `novo chat em ${stage.plan.project_name ?? "?"}`}
              {stage.plan.project_name && stage.plan.task_title
                ? ` · ${stage.plan.project_name}`
                : ""}
            </span>
            <span className="hud-keys">Enter confirma · Esc cancela</span>
          </div>
          <div className="hud-progress" style={{ animationDuration: `${CONFIRM_MS}ms` }} />
        </>
      )}
      {stage.s === "running" && (
        <div className="hud-row">
          {waveform}
          <span className="hud-text">→ {stage.target}</span>
          <span className="hud-sub">despachando…</span>
        </div>
      )}
      {stage.s === "asking" && (
        <div className="hud-row">
          <CircleQuestionMark size={14} className="hud-icon" />
          <span className="hud-text">“{stage.text}”</span>
          <span className="hud-sub">perguntando ao vox…</span>
        </div>
      )}
      {stage.s === "answer" && (
        <div className="hud-row">
          <span className="hud-text hud-answer">{stage.text}</span>
        </div>
      )}
      {stage.s === "note" && (
        <div className="hud-row">
          <span className={`hud-text ${stage.tone}`}>{stage.text}</span>
        </div>
      )}
    </div>
  );
}
