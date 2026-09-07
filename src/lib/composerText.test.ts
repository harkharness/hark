import { describe, expect, it } from "vitest";
import { fenceSegments, insideOpenFence, isFenceLine, openFencePair, paint } from "./composerText";

describe("fence lines", () => {
  it("is a fence only when the whole line is ``` plus a short language", () => {
    expect(isFenceLine("```")).toBe(true);
    expect(isFenceLine("```ts")).toBe(true);
    // The bug from the screenshot: typing into the closing fence's line
    // must STOP it being a fence, so nothing silently swallows the text.
    expect(isFenceLine("```Quando tento digitar fora da caixa")).toBe(false);
    expect(isFenceLine("``` prosa")).toBe(false);
  });
});

describe("insideOpenFence", () => {
  const block = "```\nconst x = 1;\n```\n";

  it("is true on the code line and false after the closing fence", () => {
    expect(insideOpenFence(block, block.indexOf("const"))).toBe(true);
    expect(insideOpenFence(block, block.length)).toBe(false);
  });

  it("is true while the block is still unterminated", () => {
    const open = "```\nconst x = 1;";
    expect(insideOpenFence(open, open.length)).toBe(true);
  });
});

describe("openFencePair", () => {
  it("opens a block and leaves the caret on the empty line inside", () => {
    const { text, caret } = openFencePair("```", 3);
    expect(text.slice(0, caret)).toBe("```\n");
    expect(insideOpenFence(text, caret)).toBe(true);
  });

  it("keeps the language token typed before the space", () => {
    const { text } = openFencePair("```ts", 5);
    expect(text.startsWith("```ts\n")).toBe(true);
  });

  it("leaves a line BELOW the closing fence so the block can be left", () => {
    // The screenshot bug: with the closing ``` as the last line of the
    // draft, the down arrow parks the caret ON that (invisible) line and
    // typing there destroys the fence — there was nowhere below to go.
    const { text } = openFencePair("```", 3);
    expect(text.endsWith("```\n")).toBe(true);
    expect(insideOpenFence(text, text.length)).toBe(false);
  });

  it("opens the block on its own line when prose comes before it", () => {
    const before = "olha isso:\n```";
    const { text, caret } = openFencePair(before, before.length);
    expect(text.startsWith("olha isso:\n```\n")).toBe(true);
    expect(insideOpenFence(text, caret)).toBe(true);
  });

  it("keeps whatever followed the caret after the block", () => {
    const { text } = openFencePair("```depois", 3);
    expect(text.endsWith("depois")).toBe(true);
  });
});

/** The painted span covering a character offset — parts carry no offsets
 *  of their own, and every fence looks alike by text. */
function partAt(painted: ReturnType<typeof paint>, offset: number) {
  let at = 0;
  for (const p of painted) {
    if (offset >= at && offset < at + p.text.length) return p;
    at += p.text.length;
  }
  return undefined;
}

describe("a fence the caret stands on shows itself", () => {
  it("reveals the closing marker when the caret parks on its line", () => {
    const { text } = openFencePair("```", 3);
    const closingAt = text.indexOf("```", 4);
    // Without this the fence is an invisible line you type into by
    // accident — the box vanishes and nothing says why.
    expect(partAt(paint(text, closingAt + 1), closingAt)?.cls).toContain("cm-live");
  });

  it("keeps it hidden while the caret is on the code line", () => {
    const { text, caret } = openFencePair("```", 3);
    const closingAt = text.indexOf("```", 4);
    expect(partAt(paint(text, caret), closingAt)?.cls ?? "").not.toContain("cm-live");
  });
});

describe("the mirror paints a block the moment it opens", () => {
  it("treats a freshly paired block as fenced, empty body and all", () => {
    const { text } = openFencePair("```", 3);
    const fenced = fenceSegments(text).filter((s) => s.fenced);
    expect(fenced).toHaveLength(1);
    // One box to measure: the segment carries a fid on every part.
    expect(paint(text, 4).some((p) => p.fid === 0)).toBe(true);
  });
});
