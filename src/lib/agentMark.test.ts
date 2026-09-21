import { describe, expect, it } from "vitest";
import { glyphFor, hueFor, markStateOf, monogramOf } from "./agentMark";

describe("which glyph an agent gets", () => {
  it("draws the vendor's glyph when the catalog names a vendor we drew", () => {
    expect(glyphFor("Anthropic")).toBe("anthropic");
    expect(glyphFor("Google")).toBe("google");
    expect(glyphFor("OpenAI")).toBe("openai");
  });

  it("reads the vendor whatever its casing", () => {
    expect(glyphFor("anthropic")).toBe("anthropic");
    expect(glyphFor("OPENAI")).toBe("openai");
  });

  it("falls back to a monogram for an agent the user wrote themselves", () => {
    // A twin in config.toml carries no vendor: it still has to be
    // recognizable at a glance, so it gets a letter instead of a glyph.
    expect(glyphFor("")).toBe("monogram");
    expect(glyphFor("Some Startup")).toBe("monogram");
  });
});

describe("the monogram", () => {
  it("is the id's first letter, upper case", () => {
    expect(monogramOf("qwen-local")).toBe("Q");
    expect(monogramOf("zeta")).toBe("Z");
  });

  it("skips a leading separator rather than drawing it", () => {
    expect(monogramOf("_scratch")).toBe("S");
    expect(monogramOf("-x")).toBe("X");
  });

  it("never comes back empty", () => {
    expect(monogramOf("")).toBe("?");
    expect(monogramOf("___")).toBe("?");
  });
});

describe("the hue behind a monogram", () => {
  it("is the same every time for the same id", () => {
    expect(hueFor("qwen-local")).toBe(hueFor("qwen-local"));
  });

  it("separates two ids the user is likely to have side by side", () => {
    expect(hueFor("claude-gw")).not.toBe(hueFor("qwen-local"));
  });

  it("is always one of the palette's colors", () => {
    for (const id of ["a", "bb", "ccc", "dddd", "eeeee", "ffffff", "ggggggg"])
      expect(hueFor(id)).toMatch(/^#[0-9a-f]{6}$/);
  });
});

describe("the ring around the mark, which carries the state", () => {
  const base = { selected: false, detected: true, enabled: true };

  it("marks the agent that drives the sessions", () => {
    expect(markStateOf({ ...base, selected: true })).toBe("on");
  });

  it("marks one that is installed and available", () => {
    expect(markStateOf(base)).toBe("idle");
  });

  it("marks one switched off in config", () => {
    expect(markStateOf({ ...base, enabled: false })).toBe("off");
  });

  it("marks one that is on but absent from the PATH", () => {
    expect(markStateOf({ ...base, detected: false })).toBe("missing");
  });

  it("calls an agent that is both off and absent off: the switch is the fix", () => {
    expect(markStateOf({ selected: false, detected: false, enabled: false })).toBe("off");
  });
});
