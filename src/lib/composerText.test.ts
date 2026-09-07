import { describe, expect, it } from "vitest";
import {
  exitFence,
  fenceSegments,
  insideOpenFence,
  isFenceLine,
  openFencePair,
  paint,
  wantsExit,
} from "./composerText";

describe("exitFence — the way out, whatever state the block is in", () => {
  it("closes an unterminated block and lands after it", () => {
    const open = "```\nconst x = 1;";
    const { text, caret } = exitFence(open, open.length);
    expect(insideOpenFence(text, caret)).toBe(false);
    expect(text).toBe("```\nconst x = 1;\n```\n");
    expect(caret).toBe(text.length);
  });

  it("adds the missing line when the closing fence is the last one", () => {
    const closed = "```\nconst x = 1;\n```";
    const { text, caret } = exitFence(closed, 6);
    expect(text).toBe("```\nconst x = 1;\n```\n");
    expect(insideOpenFence(text, caret)).toBe(false);
  });

  it("reuses the line that is already there instead of piling up blanks", () => {
    const after = "```\nconst x = 1;\n```\ndepois";
    const { text, caret } = exitFence(after, 6);
    expect(text).toBe(after);
    expect(text.slice(caret)).toBe("depois");
  });

  it("leaves prose after the block alone when the block is unterminated", () => {
    // Caret inside an open block that has prose under it: closing must
    // happen right after the code, not swallow the paragraph.
    const messy = "```\ncode\nmais code";
    const { text, caret } = exitFence(messy, messy.indexOf("mais"));
    expect(text.startsWith("```\ncode\nmais code\n```")).toBe(true);
    expect(insideOpenFence(text, caret)).toBe(false);
  });
});

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

describe("wantsExit — when the down arrow means 'leave'", () => {
  it("leaves from the last line of an unterminated block", () => {
    const open = "```\nFwkeijf\nskfoek";
    expect(wantsExit(open, open.length)).toBe(true);
  });

  it("leaves when the line below is the closing fence", () => {
    const closed = "```\ncode\n```\n";
    expect(wantsExit(closed, closed.indexOf("code"))).toBe(true);
  });

  it("moves normally while there is still code below", () => {
    const two = "```\nprimeira\nsegunda\n```\n";
    expect(wantsExit(two, two.indexOf("primeira"))).toBe(false);
  });

  it("does nothing outside a block", () => {
    const prose = "só texto\nmais texto";
    expect(wantsExit(prose, 0)).toBe(false);
  });
});

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
