import { describe, expect, it } from "vitest";
import { breadcrumb, colourIndex, projColour, scopeOf } from "./scope";
import type { Project } from "../types";

const hark: Project = { name: "hark", path: "/Users/x/Projects/hark" };
const landing: Project = { name: "landing", path: "/Users/x/Projects/landing" };

describe("scopeOf: the window state of today read as one scope", () => {
  it("is 'all' when nothing is forced, drafted or focused", () => {
    expect(scopeOf({})).toEqual({ kind: "all" });
  });
  it("is the project when a project window has no chat open", () => {
    expect(scopeOf({ forcedProject: hark })).toEqual({ kind: "project", project: hark });
  });
  it("is the draft's project while a new chat is being started", () => {
    expect(scopeOf({ forcedProject: hark, draftChat: landing })).toEqual({ kind: "project", project: landing });
  });
  it("is the chat when one is focused, with the project it lives in", () => {
    expect(
      scopeOf({ forcedProject: hark, focusedTask: { title: "Billing endpoint v2", sessionId: "s-bill" }, taskProject: hark }),
    ).toEqual({ kind: "chat", title: "Billing endpoint v2", sessionId: "s-bill", project: hark });
  });
  it("keeps a chat without a session as a chat, not a project", () => {
    expect(scopeOf({ focusedTask: { title: "Nova", sessionId: "" } })).toEqual({ kind: "chat", title: "Nova", sessionId: undefined, project: undefined });
  });
});

describe("breadcrumb: what the chip and the title bar print", () => {
  it("names nobody for 'all'", () => {
    expect(breadcrumb({ kind: "all" })).toEqual([]);
  });
  it("goes project then chat", () => {
    expect(breadcrumb({ kind: "chat", title: "Billing endpoint v2", project: hark })).toEqual(["hark", "Billing endpoint v2"]);
    expect(breadcrumb({ kind: "chat", title: "Solta" })).toEqual(["Solta"]);
    expect(breadcrumb({ kind: "project", project: hark })).toEqual(["hark"]);
    expect(breadcrumb({ kind: "page", title: "ADR 004", path: "decisoes/adr-004.md", space: "hark" })).toEqual(["hark", "ADR 004"]);
  });
});

describe("colourIndex: a project's colour slot is its registration order", () => {
  const projects = [hark, landing];
  it("finds the project of a path or a subpath", () => {
    expect(colourIndex(hark.path, projects)).toBe(0);
    expect(colourIndex(`${landing.path}/src`, projects)).toBe(1);
  });
  it("does not mistake a sibling with the same prefix", () => {
    expect(colourIndex("/Users/x/Projects/hark-landing", projects)).toBeUndefined();
    expect(colourIndex("", projects)).toBeUndefined();
    expect(colourIndex(undefined, projects)).toBeUndefined();
  });
  it("maps the slot to a css token and wraps past the palette", () => {
    expect(projColour(0)).toBe("var(--proj-0)");
    expect(projColour(5)).toBe("var(--proj-0)");
    expect(projColour(undefined)).toBeUndefined();
  });
});
