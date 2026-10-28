import { describe, expect, it } from "vitest";
import { confirmSpeed, targetLabel } from "./voice";
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
