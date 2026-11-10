import { describe, expect, it } from "vitest";
import { applyTextEdit, canvasSize, deckDocument, editableDocument, fitZoom, hasOwnScript, slideCount } from "./artifactView";

const plainDeck = `<!doctype html><html><head><meta name="hark-kind" content="slides"><style>section{min-height:100vh}</style></head>
<body><section><h1>One</h1></section><section><h2>Two</h2></section><section><h2>Three</h2></section></body></html>`;
const ownPager = `<html><head><style>.slide{opacity:0}.slide.active{opacity:1}</style></head><body>
<section class="slide active">1</section><section class="slide">2</section><script>let i=0;</script></body></html>`;

describe("a deck is sections; Hark turns the pages", () => {
  it("counts the slides, by class or by top-level section", () => {
    expect(slideCount(plainDeck)).toBe(3);
    expect(slideCount(ownPager)).toBe(2);
    expect(slideCount("<html><body><h1>no slides</h1></body></html>")).toBe(0);
  });
  it("runs Hark's runtime in every deck and never the page's own script", () => {
    expect(hasOwnScript(plainDeck)).toBe(false);
    expect(hasOwnScript(ownPager)).toBe(true);
    const doc = deckDocument(plainDeck);
    expect(doc).toContain("hark-deck");
    expect(doc).toContain("postMessage");
    expect(doc.indexOf("hark-deck")).toBeLessThan(doc.indexOf("</body>"));
    const own = deckDocument(ownPager);
    expect(own).toContain("hark-deck");
    expect(own).not.toContain("let i=0");
    expect(own).toContain('<section class="slide">2</section>');
  });
});

describe("a design is a canvas: its size, and the zoom that fits", () => {
  it("reads the canvas size from the page, with a default", () => {
    expect(canvasSize('<meta name="hark-canvas" content="1440x2400">')).toEqual({ w: 1440, h: 2400 });
    expect(canvasSize("<meta name='hark-canvas' content='800 x 600'>")).toEqual({ w: 800, h: 600 });
    expect(canvasSize("<html></html>")).toEqual({ w: 1440, h: 900 });
    expect(canvasSize('<meta name="hark-canvas" content="banana">')).toEqual({ w: 1440, h: 900 });
  });
  it("fits the whole canvas in the room, never above 1:1", () => {
    expect(fitZoom(720, 450, 1440, 900)).toBe(0.5);
    expect(fitZoom(2000, 2000, 1440, 900)).toBe(1);
    expect(fitZoom(0, 0, 1440, 900)).toBeGreaterThan(0);
  });
});

describe("a page's text edited in place (fase 14.2)", () => {
  const page = `<html><body><section><h1>One</h1><p>Hello</p></section><section><h1>Two</h1><p>Hello</p></section></body></html>`;
  it("carries Hark's edit runtime into every frame, decks included", () => {
    const doc = editableDocument("<html><body><h1>x</h1></body></html>");
    expect(doc).toContain("hark-edit");
    expect(doc).toContain("contentEditable");
    expect(doc).toContain("removeAttribute('contenteditable')");
    expect(doc).toContain("hark: 'quote'");
    expect(doc).toContain("hark: 'probe'");
    expect(doc).not.toContain("contentEditable = 'false'");
    expect(doc.indexOf("hark-edit")).toBeLessThan(doc.indexOf("</body>"));
    expect(editableDocument("<h1>no body</h1>")).toContain("hark-edit");
    expect(deckDocument(page)).toContain("hark-edit");
  });
  it("replaces the one place the old text sits, and refuses an ambiguous or absent one", () => {
    expect(applyTextEdit(page, [{ before: "One", after: "Um" }])).toBe(page.replace("One", "Um"));
    expect(applyTextEdit(page, [{ before: "Hello", after: "Oi" }])).toBeNull();
    expect(applyTextEdit(page, [{ before: "Nope", after: "x" }])).toBeNull();
    expect(applyTextEdit(page, [{ before: "", after: "x" }])).toBeNull();
    expect(applyTextEdit(page, [{ before: "One", after: "One" }])).toBeNull();
  });
  it("tries the whole element first: a title repeated in <title> is still placed by its tag", () => {
    const deck = `<html><head><title>Billing v2</title></head><body><h1>Billing v2</h1><p>Billing v2 ships.</p></body></html>`;
    const out = applyTextEdit(deck, [
      { before: "<h1>Billing v2</h1>", after: "<h1>Billing v2 beta</h1>" },
      { before: "Billing v2", after: "Billing v2 beta" },
    ]);
    expect(out).toBe(deck.replace("<h1>Billing v2</h1>", "<h1>Billing v2 beta</h1>"));
    // The element serialised differently from the file (quotes): the inner text is the fallback.
    const quoted = `<html><body><h2 class='x'>Only here</h2></body></html>`;
    expect(applyTextEdit(quoted, [{ before: '<h2 class="x">Only here</h2>', after: '<h2 class="x">Here</h2>' }, { before: "Only here", after: "Here" }])).toBe(quoted.replace("Only here", "Here"));
  });
});
