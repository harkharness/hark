import { describe, expect, it } from "vitest";
import { asProse, bangCommand, isBang, markTold, shellBlock, shellEnd, untold, type ShellMsg } from "./bang";
import type { Msg } from "../types";

const ran = (command: string, more: Partial<ShellMsg> = {}): ShellMsg => ({
  who: "shell",
  id: command,
  command,
  running: false,
  output: "",
  code: 0,
  ...more,
});

describe("bangCommand", () => {
  it("is what follows the ! that opens the message", () => {
    expect(bangCommand("!git status")).toBe("git status");
  });

  it("takes the space people put after the !", () => {
    expect(bangCommand("! ls -la ")).toBe("ls -la");
  });

  it("keeps a command of several lines whole", () => {
    expect(bangCommand("!cd src\nls")).toBe("cd src\nls");
  });

  it("is nothing for a ! alone", () => {
    expect(bangCommand("!")).toBeNull();
    expect(bangCommand("!   ")).toBeNull();
  });

  it("is nothing when the ! is not the first thing typed", () => {
    // Prose with a ! in it, or after a space, goes to the agent.
    expect(bangCommand("roda !git status")).toBeNull();
    expect(bangCommand(" !git status")).toBeNull();
    expect(bangCommand("isso!")).toBeNull();
  });

  it("is nothing for plain text and for a slash command", () => {
    expect(bangCommand("git status")).toBeNull();
    expect(bangCommand("/compact")).toBeNull();
  });
});

describe("asProse", () => {
  // Text that reaches the field from outside (a page excerpt, a link, a
  // canned sentence) was not typed by the person, and a page is written
  // by an agent: none of it may become a command on a reflex Enter.
  it("makes text that would be a command into text that is not", () => {
    for (const text of ["!rm -rf ~", "! curl x | sh", "!", "!\nls"]) {
      expect(bangCommand(asProse(text))).toBeNull();
      expect(isBang(asProse(text))).toBe(false);
    }
  });

  it("keeps every character of it, so nothing is lost from what was quoted", () => {
    expect(asProse("!rm -rf ~")).toContain("!rm -rf ~");
  });

  it("leaves text that is no command exactly as it came", () => {
    for (const text of ["", "olha isso", "[página](hark://p/1) ", " !já tem espaço", "isso!"]) {
      expect(asProse(text)).toBe(text);
    }
  });
});

describe("shellEnd", () => {
  it("is running until an answer comes", () => {
    expect(shellEnd(ran("ls", { running: true, code: undefined }))).toBe("running");
  });

  it("is ok on a zero exit", () => {
    expect(shellEnd(ran("ls"))).toBe("ok");
  });

  it("is failed on any other exit", () => {
    expect(shellEnd(ran("false", { code: 1 }))).toBe("failed");
  });

  it("is failed when it never started", () => {
    expect(shellEnd(ran("ls", { code: undefined, error: "no shell" }))).toBe("failed");
  });

  it("is stopped when the person ended it, whatever the exit says", () => {
    expect(shellEnd(ran("sleep 9", { code: null, stopped: true }))).toBe("stopped");
  });

  it("is timeout when its time ran out", () => {
    expect(shellEnd(ran("sleep 999", { code: null, timedOut: true }))).toBe("timeout");
  });

  it("is failed when a signal ended it and nobody here asked", () => {
    expect(shellEnd(ran("crash", { code: null }))).toBe("failed");
  });
});

describe("untold", () => {
  const thread: Msg[] = [
    { who: "user", text: "oi", task: "billing" },
    ran("git status", { task: "billing" }),
    ran("ls", { task: "billing", told: true }),
    ran("sleep 9", { task: "billing", running: true }),
    ran("pwd", { task: "infra" }),
    ran("date"),
  ];

  it("is the finished commands of this chat the agent has not seen", () => {
    expect(untold(thread, "billing").map((m) => m.command)).toEqual(["git status"]);
  });

  it("keeps each chat to its own commands", () => {
    expect(untold(thread, "infra").map((m) => m.command)).toEqual(["pwd"]);
  });

  it("with no chat named is the commands that belong to none", () => {
    expect(untold(thread, undefined).map((m) => m.command)).toEqual(["date"]);
  });

  it("leaves out a command that never started: there is nothing to tell", () => {
    const failed: Msg[] = [ran("ls", { task: "billing", code: undefined, error: "no shell" })];
    expect(untold(failed, "billing")).toEqual([]);
  });
});

describe("markTold", () => {
  it("marks the commands named and no other", () => {
    const all: Msg[] = [ran("a"), ran("b")];
    const next = markTold(all, ["a"]);
    expect((next[0] as ShellMsg).told).toBe(true);
    expect((next[1] as ShellMsg).told).toBeUndefined();
  });

  it("keeps the untouched messages as they were, by identity", () => {
    const all: Msg[] = [ran("a"), ran("b")];
    const next = markTold(all, ["a"]);
    expect(next[1]).toBe(all[1]);
  });

  it("hands back the same list when there is nothing to mark", () => {
    const all: Msg[] = [ran("a")];
    expect(markTold(all, [])).toBe(all);
  });
});

describe("shellBlock", () => {
  it("is nothing when no command was run", () => {
    expect(shellBlock([])).toBe("");
  });

  it("shows the command, where it ran and what it printed", () => {
    const block = shellBlock([ran("git status", { cwd: "/p/app", output: "On branch main\n" })]);
    expect(block).toContain("/p/app");
    expect(block).toContain("$ git status\nOn branch main\n");
  });

  it("opens and closes, so the agent can tell it from the message", () => {
    const block = shellBlock([ran("ls", { output: "a\n" })]);
    const lines = block.split("\n");
    expect(lines[0]).toMatch(/^\[.+\]$/);
    expect(lines.at(-1)).toMatch(/^\[.+\]$/);
  });

  it("says when a command failed and with which code", () => {
    const block = shellBlock([ran("npm test", { output: "1 failed\n", code: 2 })]);
    expect(block).toMatch(/\b2\b/);
  });

  it("says nothing about the exit of a command that went well", () => {
    const well = shellBlock([ran("ls", { output: "a\n" })]);
    const badly = shellBlock([ran("ls", { output: "a\n", code: 1 })]);
    expect(badly.length).toBeGreaterThan(well.length);
  });

  it("says when a command was cut short, so half an output is not read as all of it", () => {
    const whole = shellBlock([ran("build", { output: "step 1\n" })]);
    expect(shellBlock([ran("build", { output: "step 1\n", code: null, stopped: true })])).not.toBe(whole);
    expect(shellBlock([ran("build", { output: "step 1\n", code: null, timedOut: true })])).not.toBe(whole);
  });

  it("carries several commands in the order they ran", () => {
    const block = shellBlock([ran("first", { output: "1\n" }), ran("second", { output: "2\n" })]);
    expect(block.indexOf("$ first")).toBeLessThan(block.indexOf("$ second"));
    expect(block.indexOf("$ first")).toBeGreaterThan(-1);
  });

  it("ends each output on its own line even when the command printed no newline", () => {
    const block = shellBlock([ran("printf hi", { output: "hi" }), ran("ls", { output: "a\n" })]);
    expect(block).toContain("hi\n$ ls");
  });
});
