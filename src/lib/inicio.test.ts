import { describe, expect, it } from "vitest";
import { days } from "./inicio";

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
