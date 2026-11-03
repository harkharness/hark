import { describe, expect, it } from "vitest";
import { demoInvoke, DEMO_COMMANDS } from "./demo";
import ipcSource from "./ipc?raw";

/** Every command the IPC seam can send, read from its source. */
const commandsInIpc = () => [...new Set([...ipcSource.matchAll(/invoke(?:<[^>]*>)?\("([a-z_]+)"/g)].map((m) => m[1]))];

describe("demo mode: the window with invented data, no backend", () => {
  it("answers every command the seam knows, so no screen hits a hole", () => {
    const missing = commandsInIpc().filter((c) => !DEMO_COMMANDS.has(c));
    expect(missing).toEqual([]);
  });
  it("carries invented projects and a board, never a real path", async () => {
    const o = await demoInvoke<{ projects: { path: string }[]; board: unknown[]; user_name: string }>("overview");
    expect(o.projects.length).toBeGreaterThan(1);
    expect(o.board.length).toBeGreaterThan(3);
    for (const p of o.projects) expect(p.path.startsWith("/Users/ana/")).toBe(true);
    expect(o.user_name).toBe("Ana");
  });
  it("reads a session's transcript with tool calls, so the thread looks lived in", async () => {
    const t = await demoInvoke<{ entries: { role: string }[] }>("read_transcript", { sessionId: "s-billing", limit: 50 });
    expect(t.entries.map((e) => e.role)).toContain("tool_use");
    expect(t.entries.at(-1)?.role).toBe("assistant");
  });
});
