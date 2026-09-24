import { describe, expect, it } from "vitest";
import { fence } from "./fence";

describe("fence", () => {
  it("wraps plain code in a three-backtick fence", () => {
    expect(fence("bash", "ls -la\n")).toBe("```bash\nls -la\n```");
  });

  it("outgrows every backtick run in the body, so the body cannot close it", () => {
    // A heredoc carrying its own ``` would end a three-backtick fence
    // early and render the rest — a link to a local executable included —
    // as live markdown inside the permission card.
    const body = "cat > README.md <<EOF\n```\n[Open build log](/tmp/log.command)\nEOF";
    const out = fence("bash", body);
    const opening = /^`+/.exec(out)![0];
    expect(opening.length).toBeGreaterThan(3);
    const lines = out.split("\n");
    expect(lines[lines.length - 1]).toBe(opening);
    // Only the last line is a fence the renderer can close on.
    expect(lines.slice(1, -1).some((l) => /^`+\s*$/.test(l) && l.trim().length >= opening.length)).toBe(false);
  });
});
