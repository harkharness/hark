import { describe, expect, it } from "vitest";
import { notificationLine, conversationsOfToday, days } from "./inicio";
import { inicioThread } from "./threads";

const turn = (ts: string, question: string) => ({ ts, question, fala: "ok", body: "" });

describe("the days before today in the Início sidebar", () => {
  it("groups the journal by local day, newest first, today left out", () => {
    const out = days(
      [
        turn("2026-11-02T09:00:00-03:00", "o que ficou?"),
        turn("2026-11-03T10:00:00-03:00", "status do billing"),
        turn("2026-11-03T18:00:00-03:00", "fecha o dia"),
        turn("2026-11-04T08:00:00-03:00", "bom dia"),
      ],
      new Date("2026-11-04T12:00:00-03:00"),
    );
    expect(out.map((d) => d.date)).toEqual(["2026-11-03", "2026-11-02"]);
    expect(out[0].count).toBe(2);
    expect(out[0].first).toBe("status do billing");
  });

  it("knows yesterday and names the weekday otherwise", () => {
    const out = days(
      [turn("2026-11-03T10:00:00-03:00", "a"), turn("2026-10-30T10:00:00-03:00", "b")],
      new Date("2026-11-04T12:00:00-03:00"),
      "pt",
    );
    expect(out[0].label).toBe("ontem · ter 03");
    expect(out[1].label).toBe("sex 30");
  });

  it("a turn with an unreadable timestamp is skipped, not a crash", () => {
    expect(days([turn("garbage", "x")], new Date("2026-11-04T12:00:00-03:00"))).toEqual([]);
  });
});

describe("the greeting that opens the day", () => {
  it("says good morning, afternoon or evening, with the name", async () => {
    const { greeting } = await import("./inicio");
    expect(greeting(new Date("2026-11-04T08:00:00"), "Ana", "pt")).toBe("Bom dia, Ana");
    expect(greeting(new Date("2026-11-04T15:00:00"), "Ana", "pt")).toBe("Boa tarde, Ana");
    expect(greeting(new Date("2026-11-04T21:00:00"), "Ana", "en")).toBe("Good evening, Ana");
    expect(greeting(new Date("2026-11-04T08:00:00"), "", "en")).toBe("Good morning");
  });
});

describe("the conversations of today in the Início sidebar", () => {
  const now = new Date("2026-11-18T15:00:00-03:00");
  const t2 = (ts: string, question: string, conv?: string) => ({ ts, question, fala: "ok", body: "", conv });

  it("lists each one once, by its first question, the one talked to last on top", () => {
    const out = conversationsOfToday(
      [
        t2("2026-11-18T09:00:00-03:00", "o que ficou pendente?"),
        t2("2026-11-18T10:00:00-03:00", "como está o billing?", "c-a"),
        t2("2026-11-18T10:30:00-03:00", "e o deploy?", "c-b"),
        t2("2026-11-18T11:00:00-03:00", "quem revisa?", "c-a"),
      ],
      now,
    );
    expect(out.map((c) => c.id)).toEqual(["c-a", "c-b"]);
    expect(out[0]).toMatchObject({ title: "como está o billing?", count: 2 });
  });

  it("leaves out the day's own thread (it is Hoje) and the days before", () => {
    const out = conversationsOfToday(
      [t2("2026-11-17T10:00:00-03:00", "ontem", "c-old"), t2("2026-11-18T09:00:00-03:00", "sem conversa")],
      now,
    );
    expect(out).toEqual([]);
  });
});

describe("notificationLine: what a bell row leaves in Início's conversation", () => {
  const asked = {
    id: "n1",
    kind: "permission_asked" as const,
    target: { kind: "chat" as const, title: "Update main", session_id: "s-1" },
    project: "site",
    text: "pediu permissão: Bash",
    at: "2026-11-27T10:00:00Z",
  };

  it("is filed in Início's own thread, never in whatever chat is on screen", () => {
    // Untagged, it used to land in the open chat: the chat that asked got
    // a line about itself, with a way to "go to" where the person was.
    expect(notificationLine(asked, { conversation: null })?.task).toBe("");
    expect(notificationLine(asked, { conversation: "c7" })?.task).toBe(inicioThread("c7"));
  });

  it("names the chat and carries the way to it", () => {
    const line = notificationLine(asked, { conversation: null });
    expect(line?.text).toBe("Update main · pediu permissão: Bash");
    expect(line?.goto).toEqual({ title: "Update main", sessionId: "s-1" });
  });

  it("says nothing about the chat in focus on Início: it already spoke for itself", () => {
    expect(notificationLine(asked, { conversation: null, focusTitle: "Update main" })).toBeNull();
  });

  it("has no way to go for a row that is not about a chat", () => {
    const page = { ...asked, kind: "artifact_ready" as const, target: { kind: "page" as const, space: "docs", id: "plan" }, text: "pronto" };
    const line = notificationLine(page, { conversation: null });
    expect(line?.text).toBe("plan · pronto");
    expect(line?.goto).toBeUndefined();
  });
});
