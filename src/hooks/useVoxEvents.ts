// The single subscription point for backend events. Workers stream rich
// events ("vox") and permission requests ("vox-permission"); this hook
// translates them into transcript messages and worker-chip state.

import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import * as ipc from "../lib/ipc";
import type { LiveWorker, Msg, PermissionAsk, VoxEvent } from "../types";

type Handlers = {
  labelFor: (taskId: string) => string;
  push: (m: Msg) => void;
  setMessages: React.Dispatch<React.SetStateAction<Msg[]>>;
  setLiveWorkers: React.Dispatch<React.SetStateAction<Record<string, LiveWorker>>>;
  onWorkerExit: (taskId: string) => void;
  onSessionStarted: (taskId: string, sessionId: string) => void;
  speakRef: React.RefObject<boolean>;
  refresh: () => void;
};

export function useVoxEvents(h: Handlers) {
  useEffect(() => {
    const un1 = listen<VoxEvent>("vox", (e) => {
      const ev = e.payload;
      if (ev.kind === "assistant_text") {
        h.push({ who: "vox", text: ev.text, task: h.labelFor(ev.task_id) });
      } else if (ev.kind === "worker") {
        h.push({ who: "tool", name: ev.name, input: ev.input, task: h.labelFor(ev.task_id) });
      } else if (ev.kind === "tool_result") {
        h.push({
          who: "output",
          content: ev.content,
          error: ev.is_error,
          task: h.labelFor(ev.task_id),
        });
      } else if (ev.kind === "worker_turn") {
        const label = h.labelFor(ev.task_id);
        // The final assistant_text often equals the result: don't show twice.
        h.setMessages((old) => {
          const lastVox = [...old]
            .reverse()
            .find((m) => m.who === "vox" && m.task === label);
          if (lastVox && "text" in lastVox && lastVox.text === ev.text) {
            return old.map((m) =>
              m === lastVox ? { ...m, cost: ev.cost_usd, model: ev.model } : m,
            );
          }
          return [
            ...old,
            { who: "vox", text: ev.text, cost: ev.cost_usd, model: ev.model, task: label },
          ];
        });
        h.setLiveWorkers((old) =>
          old[ev.task_id]
            ? { ...old, [ev.task_id]: { ...old[ev.task_id], status: "turn_done" } }
            : old,
        );
        if (h.speakRef.current)
          ipc.speak(
            ev.is_error
              ? `A task ${label} falhou, olha a tela.`
              : `Task ${label} terminou o turno.`,
          ).catch(() => {});
        h.refresh();
      } else if (ev.kind === "worker_exit") {
        h.push({
          who: "sys",
          text: `worker ${h.labelFor(ev.task_id)} encerrado`,
          task: h.labelFor(ev.task_id),
        });
        h.onWorkerExit(ev.task_id);
        h.refresh();
      } else if (ev.kind === "session_started") {
        h.onSessionStarted(ev.task_id, ev.session_id);
        h.refresh();
      } else if (ev.kind === "status") {
        h.push({ who: "sys", text: ev.text });
      }
    });

    // Permission requests land INLINE in the thread, never as a blocking
    // modal: other workers must keep streaming while one waits.
    const un2 = listen<PermissionAsk>("vox-permission", (e) => {
      const ask = e.payload;
      h.push({
        who: "permission",
        requestId: ask.request_id,
        tool: ask.tool_name,
        input: ask.input,
        task: h.labelFor(ask.task_id),
      });
      h.setLiveWorkers((old) =>
        old[ask.task_id]
          ? { ...old, [ask.task_id]: { ...old[ask.task_id], status: "awaiting" } }
          : old,
      );
      if (h.speakRef.current)
        ipc.speak(`${ask.tool_name} pede permissão.`).catch(() => {});
    });

    return () => {
      un1.then((f) => f());
      un2.then((f) => f());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
}
