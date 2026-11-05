import { describe, expect, it } from "vitest";
import { offerTask } from "./taskNudge";

describe("the offer of a task", () => {
  it("shows after some turns of yours, with no task, until you say not now", () => {
    expect(offerTask(3, false, false)).toBe(true);
    expect(offerTask(2, false, false)).toBe(false);
    expect(offerTask(5, true, false)).toBe(false);
    expect(offerTask(5, false, true)).toBe(false);
  });
});
