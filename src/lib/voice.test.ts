import { describe, expect, it } from "vitest";
import { confirmSpeed, escapePress, micPress, targetLabel } from "./voice";
import type { VoicePlan } from "../types";

const work = (over: Partial<Extract<VoicePlan, { kind: "work" }>> = {}): Extract<VoicePlan, { kind: "work" }> => ({
  kind: "work",
  instruction: "roda os testes",
  task_title: "Retry queue fix",
  project_name: "hark",
  new_task: false,
  confidence: "high",
  warnings: [],
  ...over,
});

describe("confirmSpeed: silence confirms only the sure, warning-free, existing target", () => {
  it("fast for a high-confidence existing chat; a verdict otherwise", () => {
    expect(confirmSpeed(work())).toBe("fast");
    expect(confirmSpeed(work({ confidence: "low" }))).toBe("verdict");
    expect(confirmSpeed(work({ new_task: true }))).toBe("verdict");
    expect(confirmSpeed(work({ warnings: ["sessão pesada"] }))).toBe("verdict");
  });
});

describe("targetLabel: what the chip says", () => {
  it("the task, else a new chat in the project, else the fallback", () => {
    expect(targetLabel(work(), "novo chat")).toBe("Retry queue fix");
    expect(targetLabel(work({ task_title: null }), "novo chat")).toBe("hark");
    expect(targetLabel(work({ task_title: null, project_name: null }), "novo chat")).toBe("novo chat");
  });
});

// 23/11: "termino de falar, clico no microfone ou aperto Esc, e ele apaga
// tudo o que eu falei". The words are never thrown away by a single press.
describe("the microphone button during a round", () => {
  it("ends the capture and sends what was said while it is listening", () => {
    expect(micPress("listening")).toBe("send");
  });
  it("waits while the words are being read: they are on their way", () => {
    for (const s of ["transcribing", "loading", "thinking", "asking", "running"] as const) {
      expect(micPress(s)).toBe("wait");
    }
  });
  it("starts a new round from rest or from something that only lingers", () => {
    for (const s of ["idle", "note", "answer", "candidates", "confirm", "type"] as const) {
      expect(micPress(s)).toBe("start");
    }
  });
});

describe("Esc during a round", () => {
  it("ends the capture and keeps the words while it is listening", () => {
    expect(escapePress("listening", 0, 10_000)).toBe("send");
  });
  it("only warns the first time the words are being read, and cancels on the second", () => {
    expect(escapePress("transcribing", 0, 10_000)).toBe("warn");
    expect(escapePress("transcribing", 9_000, 10_000)).toBe("abort");
    // A warning that went stale is a first press again.
    expect(escapePress("transcribing", 1_000, 10_000)).toBe("warn");
  });
  it("gives up a model that is still loading, and closes anything else", () => {
    expect(escapePress("loading", 0, 10_000)).toBe("abort");
    expect(escapePress("answer", 0, 10_000)).toBe("close");
    expect(escapePress("confirm", 0, 10_000)).toBe("close");
  });
});
