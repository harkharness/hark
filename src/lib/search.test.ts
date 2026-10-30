import { describe, expect, it } from "vitest";
import { sections, searchItems, type SearchItem } from "./search";

const items: SearchItem[] = [
  { kind: "task", id: "t1", title: "Migração do webhook retomada", sub: "worker vivo", when: "2026-10-30T10:00:00Z", attention: true },
  { kind: "task", id: "t2", title: "Oncall SRE", when: "2026-10-28T10:00:00Z" },
  { kind: "page", id: "hark/adr-004", title: "ADR 004: janela única", sub: "hark › Decisões", when: "2026-10-29T10:00:00Z" },
  { kind: "project", id: "/p/hark", title: "hark", sub: "/p/hark" },
  { kind: "session", id: "s1", title: "conte de 1 a 600", when: "2026-09-08T10:00:00Z" },
  { kind: "action", id: "new-chat", title: "Nova conversa" },
];

describe("searchItems: every word must appear, accents aside, in the kind asked", () => {
  it("matches title or subtitle, any kind", () => {
    expect(searchItems(items, "migracao", "all").map((i) => i.id)).toEqual(["t1"]);
    expect(searchItems(items, "decisoes adr", "all").map((i) => i.id)).toEqual(["hark/adr-004"]);
  });
  it("a kind narrows; an empty query keeps everything of that kind", () => {
    expect(searchItems(items, "", "task").map((i) => i.id)).toEqual(["t1", "t2"]);
    expect(searchItems(items, "hark", "project").map((i) => i.id)).toEqual(["/p/hark"]);
    expect(searchItems(items, "hark", "session")).toEqual([]);
  });
});

describe("sections: what the palette shows before you type", () => {
  it("attention first, then recents newest first, then actions", () => {
    const s = sections(items, "", "all");
    expect(s.map((x) => x.id)).toEqual(["attention", "recent", "actions"]);
    expect(s[0].items.map((i) => i.id)).toEqual(["t1"]);
    expect(s[1].items.map((i) => i.id)).toEqual(["t1", "hark/adr-004", "t2", "s1"]);
    expect(s[2].items.map((i) => i.id)).toEqual(["new-chat"]);
  });
  it("with a query the hits group by kind, in a fixed order, no empty groups", () => {
    const s = sections(items, "a", "all");
    expect(s.map((x) => x.id)).toEqual(["task", "page", "project", "session", "action"]);
    expect(sections(items, "zzz", "all")).toEqual([]);
  });
  it("one kind is one flat group", () => {
    const s = sections(items, "", "page");
    expect(s.map((x) => x.id)).toEqual(["page"]);
  });
});
