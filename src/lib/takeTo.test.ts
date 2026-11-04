import { describe, expect, it } from "vitest";
import { destinations } from "./takeTo";

const recent = [
  { title: "Billing v2", sessionId: "s1" },
  { title: "Redesign da home", sessionId: "s2" },
  { title: "Retry queue", sessionId: "s3" },
  { title: "Alerts", sessionId: "s4" },
  { title: "Old one", sessionId: "s5" },
];

describe("where a page can be taken", () => {
  it("offers Hark, the origin chat first among chats, a few recent ones and a new chat", () => {
    const out = destinations("Redesign da home", recent, "hark-landing");
    expect(out.map((d) => d.kind)).toEqual(["hark", "origin", "chat", "chat", "chat", "new"]);
    expect(out[1]).toMatchObject({ kind: "origin", title: "Redesign da home", sessionId: "s2" });
    expect(out.filter((d) => d.kind === "chat").map((d) => d.title)).toEqual(["Billing v2", "Retry queue", "Alerts"]);
    expect(out[5]).toMatchObject({ kind: "new", project: "hark-landing" });
  });

  it("without an origin on the board there is no origin row, and nothing is picked silently", () => {
    const out = destinations(undefined, recent.slice(0, 2), "hark");
    expect(out.map((d) => d.kind)).toEqual(["hark", "chat", "chat", "new"]);
    expect(out.every((d) => !("default" in d && d.default))).toBe(true);
  });
});
