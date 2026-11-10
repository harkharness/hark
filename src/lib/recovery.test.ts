import { describe, expect, it } from "vitest";
import { recoverSession, sessionVanished } from "./recovery";

const task = { title: "Testando Artefatos Hark", sessionId: "953dcee8", workspace: "/home/ana/proj/tulipa" };
const hit = (id: string, cwd: string | null) => ({ session_id: id, title: "x", cwd });

describe("recovering a chat whose session is gone", () => {
  it("only a vanished log is grounds for a replacement", () => {
    expect(sessionVanished("vanished: sessão não está no índice")).toBe(true);
    expect(sessionVanished(new Error("vanished: No such file"))).toBe(true);
    expect(sessionVanished("database is locked")).toBe(false);
    expect(sessionVanished("")).toBe(false);
  });

  it("a locked index or any other failure never swaps the session", () => {
    expect(recoverSession("database is locked", task, hit("90c66c1f", task.workspace))).toBeNull();
  });

  it("the replacement must live in the chat's own folder", () => {
    expect(recoverSession("vanished: gone", task, hit("abc", "/home/ana/proj/tulipa/api"))?.session_id).toBe("abc");
    expect(recoverSession("vanished: gone", task, hit("90c66c1f", "/home/ana/proj/hark"))).toBeNull();
    expect(recoverSession("vanished: gone", task, hit("abc", null))).toBeNull();
  });

  it("a chat with no folder of its own can take any session but the one it lost", () => {
    const loose = { ...task, workspace: undefined };
    expect(recoverSession("vanished: gone", loose, hit("abc", "/anywhere"))?.session_id).toBe("abc");
    expect(recoverSession("vanished: gone", loose, hit("953dcee8", "/anywhere"))).toBeNull();
    expect(recoverSession("vanished: gone", loose, null)).toBeNull();
  });
});
