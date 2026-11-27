import { describe, expect, it } from "vitest";
import { chatFolder, draftThread, harkOwnedSession, inicioThread, isDraftThread, rekeyMessages, rekeyRecord } from "./threads";
import type { Msg } from "../types";

const msgs: Msg[] = [
  { who: "user", text: "a", task: "old name" } as Msg,
  { who: "hark", text: "b", task: "other" } as Msg,
  { who: "user", text: "c" } as Msg,
];

describe("a chat renamed under a live thread", () => {
  it("moves every message of the old label to the new one, leaving the rest", () => {
    const out = rekeyMessages(msgs, "old name", "New name");
    expect(out.map((m) => ("task" in m ? m.task : null))).toEqual(["New name", "other", null]);
    expect(out[1]).toBe(msgs[1]);
  });
  it("returns the same array when nothing matches or the names are equal", () => {
    expect(rekeyMessages(msgs, "nope", "x")).toBe(msgs);
    expect(rekeyMessages(msgs, "old name", "old name")).toBe(msgs);
  });
  it("moves a per-thread record the same way, the target winning a clash", () => {
    expect(rekeyRecord({ a: 1, b: 2 }, "a", "c")).toEqual({ b: 2, c: 1 });
    expect(rekeyRecord({ a: 1, b: 2 }, "a", "b")).toEqual({ b: 2 });
    const same = { a: 1 };
    expect(rekeyRecord(same, "z", "y")).toBe(same);
  });
});

describe("a session that is Hark's own", () => {
  it("is the one the state file names, or any one whose cwd is the data dir", () => {
    expect(harkOwnedSession({ session_id: "s1", cwd: "/Users/ana/Projects/x" }, "/Users/ana/.hark", "s1")).toBe(true);
    expect(harkOwnedSession({ session_id: "s2", cwd: "/Users/ana/.hark" }, "/Users/ana/.hark/", null)).toBe(true);
    expect(harkOwnedSession({ session_id: "s3", cwd: "/Users/ana/.hark/sub" }, "/Users/ana/.hark", null)).toBe(true);
  });
  it("is not a project chat", () => {
    expect(harkOwnedSession({ session_id: "s4", cwd: "/Users/ana/Projects/x" }, "/Users/ana/.hark", "s1")).toBe(false);
    expect(harkOwnedSession({ session_id: "s5", cwd: "/Users/ana/.harkness" }, "/Users/ana/.hark", null)).toBe(false);
    expect(harkOwnedSession({ session_id: "s6" }, null, null)).toBe(false);
  });
});

describe("a new chat's draft has a thread of its own", () => {
  it("is keyed by the project, so two drafts never share a page and neither is the general thread", () => {
    const a = draftThread("/Users/x/Projects/hark");
    const b = draftThread("/Users/x/Projects/landing");
    expect(a).not.toBe("");
    expect(a).not.toBe(b);
    expect(draftThread("/Users/x/Projects/hark")).toBe(a);
  });

  it("is told from a chat's title, whatever the title says", () => {
    expect(isDraftThread(draftThread("/p"))).toBe(true);
    expect(isDraftThread("novo chat em hark")).toBe(false);
    expect(isDraftThread("")).toBe(false);
  });
});

describe("a conversation of Início has a thread of its own", () => {
  it("is keyed by its id, and is neither the general thread nor a draft", () => {
    expect(inicioThread("c-a")).not.toBe("");
    expect(inicioThread("c-a")).not.toBe(inicioThread("c-b"));
    expect(isDraftThread(inicioThread("c-a"))).toBe(false);
  });
});

describe("chatFolder: the folder the chat on screen works in", () => {
  const board = [
    { title: "Update main from upstream", workspace: "/w/site", session_ids: ["s-1"] },
    { title: "Fix login", workspace: "/w/app/web", session_ids: ["s-2"] },
  ];
  const sessions = [{ session_id: "s-9", title: "old talk", cwd: "/w/notes" }];
  const none = { board: [], sessions: [] };

  it("is the folder the chat was opened in, before anything else knows the chat", () => {
    // A chat born from "+": its name on screen is still the first words
    // of the message, the board already holds another title for it.
    const chat = { title: "In this project go back", sessionId: "", workspace: "/w/site" };
    expect(chatFolder(chat, { board, sessions })).toBe("/w/site");
  });

  it("is the live worker's folder when the chat was not opened with one", () => {
    const chat = { title: "In this project go back", sessionId: "" };
    expect(chatFolder(chat, { ...none, worker: { workspace: "/w/site" } })).toBe("/w/site");
  });

  it("is the board card's folder, found by the chat's title", () => {
    expect(chatFolder({ title: "Fix login", sessionId: "" }, { board, sessions })).toBe("/w/app/web");
  });

  it("is the board card's folder, found by the session when the title differs", () => {
    expect(chatFolder({ title: "renamed since", sessionId: "s-1" }, { board, sessions })).toBe("/w/site");
  });

  it("is the folder the session ran in, for a chat that is on no card", () => {
    expect(chatFolder({ title: "old talk", sessionId: "s-9" }, { board, sessions })).toBe("/w/notes");
  });

  it("is nothing when nobody knows, and for no chat at all", () => {
    expect(chatFolder({ title: "stray", sessionId: "s-0" }, { board, sessions })).toBeUndefined();
    expect(chatFolder(null, { board, sessions })).toBeUndefined();
  });

  it("skips an empty folder instead of stopping at it", () => {
    const chat = { title: "Fix login", sessionId: "s-2", workspace: "" };
    expect(chatFolder(chat, { board, sessions, worker: { workspace: "" } })).toBe("/w/app/web");
  });
});
