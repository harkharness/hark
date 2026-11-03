/**
 * What Hark adds around a page by its kind. A page is content; the
 * viewer is Hark's. A deck is a run of sections and Hark turns the pages
 * (pager, filmstrip, keys, present); a design is a canvas Hark zooms and
 * pans; a document is a reading column. The agent writes none of that.
 */

/** The stage every deck is written for. */
export const STAGE_W = 1280;
export const STAGE_H = 720;

/** The deck brought its own script (a pager, say): Hark leaves it alone
 *  and only gives it the stage. */
export function hasOwnScript(html: string): boolean {
  return /<script[\s>]/i.test(html);
}

/** How many slides: sections marked as slides, else top-level sections. */
export function slideCount(html: string): number {
  const marked = (html.match(/<section[^>]*(class=["'][^"']*\bslide\b|data-slide)/gi) ?? []).length;
  if (marked > 0) return marked;
  return (html.match(/<section[\s>]/gi) ?? []).length;
}

/** Hark's deck runtime: every slide is a stage-sized layer, one shown at
 *  a time; the parent says which through postMessage and hears the count
 *  back. Injected only when the deck has no script of its own. */
const DECK_RUNTIME = `<style data-hark-deck>
  html, body { margin: 0; width: ${STAGE_W}px; height: ${STAGE_H}px; overflow: hidden; }
  body > section, .hark-deck > section { position: absolute; inset: 0; width: ${STAGE_W}px; min-height: ${STAGE_H}px; height: ${STAGE_H}px; box-sizing: border-box; overflow: hidden; display: none; }
  body > section[data-hark-active], .hark-deck > section[data-hark-active] { display: block; }
</style>
<script data-hark-deck>
(function () {
  var slides = function () { return Array.prototype.slice.call(document.querySelectorAll('body > section, .hark-deck > section')); };
  var at = 0;
  function show(n) {
    var all = slides(); if (!all.length) return;
    at = Math.max(0, Math.min(all.length - 1, n));
    all.forEach(function (s, i) { if (i === at) s.setAttribute('data-hark-active', ''); else s.removeAttribute('data-hark-active'); });
    parent.postMessage({ hark: 'slide', at: at, count: all.length }, '*');
  }
  window.addEventListener('message', function (e) {
    var m = e.data || {}; if (m.hark === 'slide') show(m.n);
  });
  window.addEventListener('keydown', function (e) {
    if (e.key === 'ArrowRight' || e.key === ' ' || e.key === 'PageDown') { e.preventDefault(); show(at + 1); }
    if (e.key === 'ArrowLeft' || e.key === 'PageUp') { e.preventDefault(); show(at - 1); }
    if (e.key === 'Escape') parent.postMessage({ hark: 'escape' }, '*');
  });
  window.addEventListener('DOMContentLoaded', function () { show(0); });
})();
</script>`;

/** The document the deck frame loads: the page, with Hark's runtime
 *  before </head> when the page did not bring a pager of its own. */
export function deckDocument(html: string): string {
  if (hasOwnScript(html)) return html;
  const at = html.search(/<\/head>/i);
  if (at >= 0) return html.slice(0, at) + DECK_RUNTIME + html.slice(at);
  const body = html.search(/<body[\s>]/i);
  if (body >= 0) return html.slice(0, body) + DECK_RUNTIME + html.slice(body);
  return DECK_RUNTIME + html;
}

/** A design says its size with <meta name="hark-canvas" content="WxH">. */
export function canvasSize(html: string): { w: number; h: number } {
  const m = html.match(/<meta[^>]+name=["']hark-canvas["'][^>]*content=["']\s*(\d+)\s*[x×]\s*(\d+)\s*["']/i);
  if (m) return { w: Number(m[1]), h: Number(m[2]) };
  return { w: 1440, h: 900 };
}

export const ZOOMS = [0.25, 0.5, 0.75, 1, 1.5, 2];

/** The zoom at which the whole canvas is on screen, never past 1:1. */
export function fitZoom(roomW: number, roomH: number, w: number, h: number): number {
  const z = Math.min(roomW / w, roomH / h, 1);
  return Math.max(0.05, Number.isFinite(z) ? z : 0.05);
}
