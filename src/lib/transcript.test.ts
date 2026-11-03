import { describe, expect, it } from "vitest";
import { alreadyShown } from "./transcript";
import type { Msg } from "../types";

const user = (text: string, task = "t", ts?: number): Msg => ({ who: "user", text, task, ts });
const hark = (text: string, task = "t", ts?: number): Msg => ({ who: "hark", text, task, ts });
const tool = (name: string, input: string, task = "t"): Msg => ({ who: "tool", name, input, task });
const out = (content: string, task = "t"): Msg => ({ who: "output", content, error: false, task });

describe("alreadyShown: a log entry the thread already carries", () => {
  it("matches a live message by thread, role and text, whatever the timestamps say", () => {
    const shown = [user("faz html", "t", 1), tool("Write", "{a}"), out("ok"), hark("Fiz", "t", 2)];
    expect(alreadyShown(shown, user("faz html", "t", 999))).toBe(true);
    expect(alreadyShown(shown, tool("Write", "{a}"))).toBe(true);
    expect(alreadyShown(shown, out("ok"))).toBe(true);
    expect(alreadyShown(shown, hark("Fiz", "t", 999))).toBe(true);
  });
  it("is not fooled by the same words in another thread or another role", () => {
    const shown = [user("faz html", "a", 1), hark("ok", "a", 2)];
    expect(alreadyShown(shown, user("faz html", "b"))).toBe(false);
    expect(alreadyShown(shown, hark("faz html", "a"))).toBe(false);
    expect(alreadyShown(shown, tool("Write", "ok"))).toBe(false);
  });
  it("looks only at the recent tail, so an old identical line does not swallow a new one", () => {
    const shown = [user("de novo", "t", 1), ...Array.from({ length: 60 }, (_, i) => hark(`linha ${i}`, "t", i + 2))];
    expect(alreadyShown(shown, user("de novo", "t"))).toBe(false);
    expect(alreadyShown(shown, user("de novo", "t"), 100)).toBe(true);
  });
});
