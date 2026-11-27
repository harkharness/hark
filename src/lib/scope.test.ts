import { describe, expect, it } from "vitest";
import { breadcrumb, colourIndex, projColour, projectsInView, scopeChoices, scopeOf } from "./scope";
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

describe("scopeOf: an open artifact page", () => {
  it("is the scope while the page is on screen, whatever chat was focused before", () => {
    const page = { title: "ADR 004", path: "decisoes/adr-004.md", space: "hark" };
    expect(scopeOf({ focusedTask: { title: "Billing", sessionId: "s1" }, page })).toEqual({ kind: "page", ...page });
    expect(breadcrumb(scopeOf({ page }))).toEqual(["hark", "ADR 004"]);
  });
});

describe("scopeChoices: what the chip offers when it is a picker", () => {
  it("offers every project after 'all', and marks the one in force", () => {
    const rows = scopeChoices([hark, landing], { kind: "project", project: landing });
    expect(rows).toEqual([
      { project: null, current: false },
      { project: hark, current: false },
      { project: landing, current: true },
    ]);
  });
  it("marks 'all' when nobody is in command", () => {
    expect(scopeChoices([hark], { kind: "all" })?.[0]).toEqual({ project: null, current: true });
  });
  it("offers nothing inside a chat or a page: the target was chosen by opening it", () => {
    expect(scopeChoices([hark], { kind: "chat", title: "t", project: hark })).toBeNull();
    expect(scopeChoices([hark], { kind: "page", title: "t", path: "p", space: "s" })).toBeNull();
  });
});

describe("projectsInView: whose files, terminal and session the panels show", () => {
  const all = [hark, landing];

  it("is the chat's project alone: a panel beside a chat is about that chat", () => {
    const scope = scopeOf({ focusedTask: { title: "fix login", sessionId: "s1" }, taskProject: landing });
    expect(projectsInView(all, scope)).toEqual([landing]);
  });

  it("is the project alone while a new chat is being started in it", () => {
    expect(projectsInView(all, scopeOf({ draftChat: hark }))).toEqual([hark]);
  });

  it("is every project where nobody is in command", () => {
    expect(projectsInView(all, scopeOf({}))).toEqual(all);
  });

  it("is every project for a chat whose folder is no project: better all than none", () => {
    const scope = scopeOf({ focusedTask: { title: "stray", sessionId: "s2" } });
    expect(projectsInView(all, scope)).toEqual(all);
  });
});
