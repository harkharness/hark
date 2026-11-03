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
 *  back. The page's own scripts are left out of the frame and whatever
 *  is not a slide (its pager, a progress strip) is hidden: the pager is
 *  Hark's, always. */
const DECK_RUNTIME = `<style data-hark-deck>
  html, body { margin: 0 !important; width: ${STAGE_W}px; height: ${STAGE_H}px; overflow: hidden !important; }
  [data-hark-slide] { position: absolute !important; inset: 0 !important; width: ${STAGE_W}px !important; height: ${STAGE_H}px !important; min-height: 0 !important; box-sizing: border-box; overflow: hidden; display: none !important; opacity: 1 !important; transform: none !important; visibility: visible !important; transition: none !important; }
  [data-hark-slide][data-hark-active] { display: block !important; }
  [data-hark-hidden] { display: none !important; }
</style>
<script data-hark-deck>
(function () {
  function slides() {
    var marked = Array.prototype.slice.call(document.querySelectorAll('section.slide, [data-slide]'));
    if (marked.length) return marked;
    var top = Array.prototype.slice.call(document.querySelectorAll('body > section'));
    if (top.length) return top;
    return Array.prototype.slice.call(document.querySelectorAll('body > * > section'));
  }
  var all = slides();
  var at = 0;
  // Mark the slides; hide every body child that holds none (the page's
  // own pager, a progress strip) and, inside a container of slides, every
  // child that is not one.
  all.forEach(function (s) { s.setAttribute('data-hark-slide', ''); });
  Array.prototype.forEach.call(document.body.children, function (el) {
    if (el.tagName === 'SCRIPT' || el.tagName === 'STYLE') return;
    var holds = el.hasAttribute('data-hark-slide') || el.querySelector('[data-hark-slide]');
    if (!holds) { el.setAttribute('data-hark-hidden', ''); return; }
    if (!el.hasAttribute('data-hark-slide')) {
      Array.prototype.forEach.call(el.children, function (k) {
        if (!k.hasAttribute('data-hark-slide') && !k.querySelector('[data-hark-slide]') && k.tagName !== 'STYLE') k.setAttribute('data-hark-hidden', '');
      });
    }
  });
  function show(n) {
    if (!all.length) return;
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
  show(0);
})();
</script>`;

/** The page's own <script> blocks, gone: in the deck frame only Hark's
 *  runtime runs. The file is untouched (artifactStripPager does that). */
export function withoutScripts(html: string): string {
  return html.replace(/<script[\s>][\s\S]*?<\/script>/gi, "");
}

/** The document the deck frame loads: the page without its scripts, with
 *  Hark's runtime at the end of the body (the slides exist by then). */
export function deckDocument(html: string): string {
  const clean = withoutScripts(html);
  const at = clean.search(/<\/body>/i);
  if (at >= 0) return clean.slice(0, at) + DECK_RUNTIME + clean.slice(at);
  return clean + DECK_RUNTIME;
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
