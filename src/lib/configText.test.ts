import { describe, expect, it } from "vitest";
import { sectionLine } from "./configText";

describe("finding a table in config.toml", () => {
  it("returns the 1-based line of the header", () => {
    const text = 'model = "sonnet"\n\n[agents.gemini]\nargs = []\n\n[agents.claude-twin]\nplugin = "claude"\n';
    expect(sectionLine(text, "agents.claude-twin")).toBe(6);
  });

  it("does not mistake a sub-table or a longer name for the table itself", () => {
    const text = '[agents.twin.models]\nlight = "x"\n[agents.twinkle]\nplugin = "acp"\n';
    expect(sectionLine(text, "agents.twin")).toBeNull();
  });

  it("tolerates spaces inside the brackets and a trailing comment", () => {
    expect(sectionLine('[ agents.twin ]   # the gateway one\nplugin = "claude"\n', "agents.twin")).toBe(1);
  });
});
