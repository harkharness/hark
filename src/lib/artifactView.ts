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
    if (document.activeElement && document.activeElement.isContentEditable) return;
    if (e.key === 'ArrowRight' || e.key === ' ' || e.key === 'PageDown') { e.preventDefault(); show(at + 1); }
    if (e.key === 'ArrowLeft' || e.key === 'PageUp') { e.preventDefault(); show(at - 1); }
    if (e.key === 'Escape') parent.postMessage({ hark: 'escape' }, '*');
  });
  show(0);
})();
</script>`;

/** Hark's in-place editor (fase 14.2): a double click on a text element
 *  makes it editable; leaving it (blur, or Enter in a heading) sends the
 *  element's old and new inner html to the parent, which places the
 *  change in the file by its old text. Escape puts the old text back.
 *  The page's own layout is never touched: only words change here. */
const EDIT_RUNTIME = `<style data-hark-edit>
  [data-hark-editing] { outline: 2px solid #6ea8fe !important; outline-offset: 2px; cursor: text; }
</style>
<script data-hark-edit>
(function () {
  var BLOCKS = /^(H1|H2|H3|H4|H5|H6|P|LI|TD|TH|BLOCKQUOTE|FIGCAPTION|LABEL|SPAN|A|STRONG|EM|B|I|SMALL|CODE|PRE|DT|DD|DIV|BUTTON|SUMMARY)$/;
  function target(el) {
    while (el && el !== document.body) {
      if (BLOCKS.test(el.tagName) && el.textContent && el.textContent.trim() && !el.querySelector('section,article,table,ul,ol,img,svg,video,iframe')) return el;
      el = el.parentElement;
    }
    return null;
  }
  var open = null;
  var before = '';
  var outerBefore = '';
  function close(keep) {
    if (!open) return;
    var el = open; open = null;
    // Removed, not set to false: the attribute would ride into the file.
    el.removeAttribute('contenteditable');
    el.removeAttribute('data-hark-editing');
    var after = el.innerHTML;
    if (!keep) { el.innerHTML = before; return; }
    // The whole element first (a word repeated in <title> is still one
    // h1), the words alone as the fallback.
    if (after !== before) parent.postMessage({ hark: 'edit', before: outerBefore, after: el.outerHTML, inner: { before: before, after: after } }, '*');
  }
  document.addEventListener('dblclick', function (e) {
    var el = target(e.target);
    if (!el || open === el) return;
    close(true);
    open = el; before = el.innerHTML; outerBefore = el.outerHTML;
    el.contentEditable = 'true';
    el.setAttribute('data-hark-editing', '');
    el.focus();
    var sel = window.getSelection(); if (sel) { sel.removeAllRanges(); var r = document.createRange(); r.selectNodeContents(el); sel.addRange(r); }
  });
  document.addEventListener('keydown', function (e) {
    if (!open) return;
    if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); close(false); }
    if (e.key === 'Enter' && !e.shiftKey && /^H[1-6]$/.test(open.tagName)) { e.preventDefault(); close(true); }
  }, true);
  document.addEventListener('focusout', function () { setTimeout(function () { if (open && document.activeElement !== open) close(true); }, 0); });
  // The window asks what sits at a point (a mark was drawn there): the
  // element's words come back, so the anchor can name them.
  window.addEventListener('message', function (e) {
    var m = e.data || {};
    if (m.hark !== 'probe') return;
    var el = document.elementFromPoint(m.x, m.y);
    while (el && el !== document.body && !(el.innerText && el.innerText.trim())) el = el.parentElement;
    var text = el && el !== document.body ? (el.innerText || '').trim().slice(0, 160) : '';
    parent.postMessage({ hark: 'probe', id: m.id, text: text, tag: el ? el.tagName : '' }, '*');
  });
  // A passage selected while not editing: a comment may anchor to it.
  document.addEventListener('mouseup', function () {
    if (open) return;
    var s = window.getSelection();
    var text = s ? s.toString().trim() : '';
    if (text) parent.postMessage({ hark: 'quote', text: text.slice(0, 500) }, '*');
  });
})();
</script>`;

/** The document a frame loads when the person may edit words in place:
 *  the page with Hark's edit runtime at the end of the body. */
export function editableDocument(html: string): string {
  const at = html.search(/<\/body>/i);
  if (at >= 0) return html.slice(0, at) + EDIT_RUNTIME + html.slice(at);
  return html + EDIT_RUNTIME;
}

/** The file after one in-place edit: `before` (the element's old inner
 *  html) must sit in the file exactly once, or the edit is refused (null)
 *  rather than placed on a guess. */
export function applyTextEdit(file: string, candidates: { before: string; after: string }[]): string | null {
  for (const { before, after } of candidates) {
    if (!before || before === after) continue;
    const first = file.indexOf(before);
    if (first < 0 || file.indexOf(before, first + 1) >= 0) continue;
    return file.slice(0, first) + after + file.slice(first + before.length);
  }
  return null;
}

/** The page's own <script> blocks, gone: in the deck frame only Hark's
 *  runtime runs. The file is untouched (artifactStripPager does that). */
export function withoutScripts(html: string): string {
  return html.replace(/<script[\s>][\s\S]*?<\/script>/gi, "");
}

/** The document the deck frame loads: the page without its scripts, with
 *  Hark's runtime at the end of the body (the slides exist by then). */
export function deckDocument(html: string): string {
  const clean = withoutScripts(html);
  const runtime = DECK_RUNTIME + EDIT_RUNTIME;
  const at = clean.search(/<\/body>/i);
  if (at >= 0) return clean.slice(0, at) + runtime + clean.slice(at);
  return clean + runtime;
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
