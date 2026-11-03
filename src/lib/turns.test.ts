import { describe, expect, it } from "vitest";
import { staleTurns } from "./turns";

const GENERAL = "";
const now = 100_000;

describe("staleTurns: a clock on screen must have a turn behind it", () => {
  it("ends the clock of a chat whose session has no turn in flight, once it is old enough", () => {
    const turns = { "Testando Artefatos": { startedAt: now - 60_000 }, Fresh: { startedAt: now - 2_000 } };
    const inFlight: { task_id: string; session_id: string }[] = [];
    const sessionOf = (k: string) => (k === "Testando Artefatos" ? "s-1" : k === "Fresh" ? "s-2" : null);
    expect(staleTurns(turns, inFlight, sessionOf, GENERAL, now)).toEqual(["Testando Artefatos"]);
  });
  it("keeps a clock whose session is in flight, and one it cannot map to a session", () => {
    const turns = { Billing: { startedAt: now - 60_000 }, Mystery: { startedAt: now - 60_000 } };
    const inFlight = [{ task_id: "t-1", session_id: "s-b" }];
    const sessionOf = (k: string) => (k === "Billing" ? "s-b" : null);
    expect(staleTurns(turns, inFlight, sessionOf, GENERAL, now)).toEqual([]);
  });
  it("the general chat's clock follows the hark-chat task, not a session", () => {
    const turns = { [GENERAL]: { startedAt: now - 60_000 } };
    expect(staleTurns(turns, [{ task_id: "hark-chat", session_id: "" }], () => null, GENERAL, now)).toEqual([]);
    expect(staleTurns(turns, [], () => null, GENERAL, now)).toEqual([GENERAL]);
  });
});
