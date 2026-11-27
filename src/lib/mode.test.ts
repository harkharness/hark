import { describe, expect, it } from "vitest";
import { chatMode, windowAllows, windowMode } from "./mode";

describe("windowMode: the mode a new chat will be born with", () => {
  it("is what was picked in this window", () => {
    expect(windowMode("plan", "auto")).toBe("plan");
  });

  it("is the configured default when nothing was picked", () => {
    expect(windowMode(null, "auto")).toBe("auto");
  });

  it("is manual when the config has none: that is what an agent started without a mode gets", () => {
    // An empty string is what a config without worker_mode reads as. It
    // used to pass for a mode and the pill fell back to its first row.
    expect(windowMode(null, "")).toBe("manual");
    expect(windowMode(null, undefined)).toBe("manual");
  });
});

describe("chatMode: the mode the pill shows", () => {
  it("is the window's while no chat is open", () => {
    expect(chatMode(undefined, "auto")).toBe("auto");
  });

  it("is the chat's own once it is live, whatever the window says now", () => {
    expect(chatMode({ mode: "plan" }, "auto")).toBe("plan");
  });

  it("is manual for a live chat started without a mode, not the window's later pick", () => {
    expect(chatMode({}, "auto")).toBe("manual");
    expect(chatMode({ mode: null }, "auto")).toBe("manual");
  });
});

describe("windowAllows: a permission the window answers without asking", () => {
  const ask = (more: object = {}) => ({ tool_name: "Bash", ...more });

  it("answers a tool the person said always to allow in this chat", () => {
    expect(windowAllows(ask(), new Set(["Bash"]))).toBe(true);
    expect(windowAllows(ask(), new Set(["Edit"]))).toBe(false);
  });

  it("answers anything when the chat's mode was relaxed over a running turn", () => {
    // The backend says so on the ask itself: nothing here to lose on a
    // rename or a reload.
    expect(windowAllows(ask({ mode_relaxed: true }), undefined)).toBe(true);
  });

  it("asks when there is no rule and the mode is not relaxed", () => {
    expect(windowAllows(ask(), undefined)).toBe(false);
    expect(windowAllows(ask({ mode_relaxed: false }), new Set())).toBe(false);
  });

  it("never answers an ask that touches production, whatever the rules say", () => {
    const risky = ask({ mode_relaxed: true, prod_risk: "kubectl apply on prod" });
    expect(windowAllows(risky, new Set(["Bash"]))).toBe(false);
  });
});
