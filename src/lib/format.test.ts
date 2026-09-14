import { describe, expect, it } from "vitest";
import { costLabel, shortModel } from "./format";

describe("the footer of a reply", () => {
  it("shortens a Claude model id but leaves an agent id alone", () => {
    // Observed: an answer through claude-agent-acp was signed "acp" — the
    // agent id "claude-acp" had its prefix cut as if it were a model name.
    expect(shortModel("claude-fable-5-1")).toBe("fable-5-1");
    expect(shortModel("claude-opus-5[1m]")).toBe("opus-5[1m]");
    expect(shortModel("claude-acp")).toBe("claude-acp");
    expect(shortModel("gemini-2.5-flash")).toBe("gemini-2.5-flash");
    expect(shortModel(undefined)).toBe("?");
  });

  it("never prints a price nobody reported", () => {
    // The same answer read "$0.0000". The subscription paid for 27k tokens
    // of opus; the agent just did not say so. Absence is not zero.
    expect(costLabel(0.0123)).toBe("$0.0123");
    expect(costLabel(0)).toBe("$0.0000");
    expect(costLabel(undefined)).not.toContain("$");
    expect(costLabel(undefined).length).toBeGreaterThan(0);
  });
});
