// The composer's pure text layer: fence/quote/code/link segmentation for
// the mirror, and the attachment helpers. Lives OUTSIDE the component file
// so Vite's Fast Refresh keeps working (component files must export only
// components) — and so these stay plain functions, testable without React.

/** A pasted screenshot: thumbnail on top, "[image N]" reference in prose. */
export type Attachment = { dataUrl: string };

/** data URL → (media_type, base64) pair the backend expects. */
export function toImagePair(a: Attachment): [string, string] {
  return [a.dataUrl.slice(5, a.dataUrl.indexOf(";")), a.dataUrl.split(",")[1]];
}

/** A fence line is ``` plus at most a SHORT language token ("ts",
 *  "typescript") and NOTHING else — "``` some prose" is prose, and so is
 *  ```` ```averylongword ````: real language ids stop at 12 chars, and a
 *  longer run is someone's text that must never vanish into a marker. */
const FENCE_LINE = /(^|\n)```[\w+-]{0,12}(\n|$)/g;

/** Is this whole line a fence marker? One definition for the parser and
 *  the keydown helpers, so they can never disagree. */
export function isFenceLine(line: string): boolean {
  return /^```[\w+-]{0,12}$/.test(line);
}

/** Inside an UNTERMINATED block at this offset? Counts fence lines above
 *  the caret's line: odd means a block is open — a typed ``` there is a
 *  CLOSER, and the auto-pair must stay out of the way. */
export function insideOpenFence(text: string, caret: number): boolean {
  const lineStart = text.lastIndexOf("\n", Math.max(0, caret - 1)) + 1;
  const above = text.slice(0, lineStart);
  const fences = above.split("\n").filter(isFenceLine).length;
  return fences % 2 === 1;
}

/** Typing ``` and pressing space or enter opens a block: the fence line,
 *  an empty line the caret lands on, the closing fence — and a line BELOW
 *  the closing fence, which is the way out.
 *
 *  That last line is not cosmetic. Without it the closing fence is the
 *  draft's last line; the down arrow parks the caret on it, the fence
 *  paints invisible, and the next keystroke lands INSIDE the marker —
 *  "```" becomes "```texto", stops being a fence, and the whole box
 *  disappears with no way back. */
export function openFencePair(text: string, caret: number): { text: string; caret: number } {
  const opened = `${text.slice(0, caret)}\n`;
  return { text: `${opened}\n\`\`\`\n${text.slice(caret)}`, caret: opened.length };
}

/** Leave the block the caret is standing in, whatever shape it is in:
 *  close it if it was never closed, add the line under the closing fence
 *  if there wasn't one, and land the caret on that line.
 *
 *  This exists because "just press down" is not a promise the composer
 *  can keep — the fences paint invisible, so a caret parked on one looks
 *  identical to a caret past it, and a draft can always end mid-block. */
export function exitFence(text: string, caret: number): { text: string; caret: number } {
  const lines = text.split("\n");
  let line = 0;
  for (let at = 0; line < lines.length; line++) {
    const end = at + lines[line].length;
    if (caret <= end) break;
    at = end + 1;
  }
  const out = [...lines];
  let close = out.findIndex((l, i) => i >= line && isFenceLine(l));
  if (close === -1) {
    // Unterminated: close it right under the code, never past prose the
    // user wrote below.
    out.splice(line + 1, 0, "```");
    close = line + 1;
  }
  if (close === out.length - 1) out.push("");
  return {
    text: out.join("\n"),
    caret: out.slice(0, close + 1).join("\n").length + 1,
  };
}

/** Should the down arrow LEAVE the block instead of moving a line? Yes
 *  when the caret is inside one and the line below is either nothing at
 *  all or the closing fence itself — the fence paints invisible, so
 *  stepping onto it looks like the arrow did nothing, and whatever you
 *  type next destroys the marker. */
export function wantsExit(text: string, caret: number): boolean {
  if (!insideOpenFence(text, caret)) return false;
  const nl = text.indexOf("\n", caret);
  if (nl === -1) return true;
  const rest = text.slice(nl + 1);
  const cut = rest.indexOf("\n");
  return isFenceLine(cut === -1 ? rest : rest.slice(0, cut));
}

/** Does the draft contain a fenced block at all? Drives the monospace
 *  switch: prose stays proportional until code is actually present. */
export function hasFence(text: string): boolean {
  FENCE_LINE.lastIndex = 0;
  return FENCE_LINE.test(text);
}

/** Split a draft into fenced and unfenced runs, IN ORDER, keeping every
 *  character — the mirror must be glyph-for-glyph identical to the
 *  textarea or the caret drifts away from the text under it. */
export function fenceSegments(text: string): { text: string; fenced: boolean }[] {
  const out: { text: string; fenced: boolean }[] = [];
  const fence = new RegExp(FENCE_LINE.source, "g");
  let at = 0;
  let open: number | null = null;
  let m: RegExpExecArray | null;
  while ((m = fence.exec(text))) {
    if (open === null) {
      open = m.index + (m[1] ? 1 : 0);
      if (open > at) out.push({ text: text.slice(at, open), fenced: false });
    } else {
      const end = m.index + m[0].length;
      out.push({ text: text.slice(open, end), fenced: true });
      at = end;
      open = null;
    }
  }
  // An unterminated fence is still a code block being written — that is
  // the whole point: the box appears while you type inside it.
  if (open !== null) out.push({ text: text.slice(open), fenced: true });
  else if (at < text.length) out.push({ text: text.slice(at), fenced: false });
  return out;
}

/** Any marker the mirror styles? Drives the textarea's transparent-glyph
 *  switch: plain prose stays a plain visible textarea. */
export function hasRich(text: string): boolean {
  return (
    hasFence(text) || /(^|\n)> /.test(text) || /`[^`\n]+`/.test(text) || /https?:\/\//.test(text)
  );
}

export type InlinePart = {
  text: string;
  kind: "plain" | "marker" | "code" | "qmark" | "quote" | "link";
};

/** Links in one run of text: a markdown [label](url) renders as the
 *  label alone (markers and url hidden, revealed on the caret's line),
 *  and a bare url highlights as itself. */
function linkParts(run: string, base: "plain" | "quote"): InlinePart[] {
  const parts: InlinePart[] = [];
  const token = /\[([^\]\n]+)\]\((https?:\/\/[^\s)]+)\)|https?:\/\/[^\s<>]+/g;
  let at = 0;
  let m: RegExpExecArray | null;
  while ((m = token.exec(run))) {
    if (m.index > at) parts.push({ text: run.slice(at, m.index), kind: base });
    if (m[1]) {
      parts.push({ text: "[", kind: "marker" });
      parts.push({ text: m[1], kind: "link" });
      parts.push({ text: `](${m[2]})`, kind: "marker" });
    } else {
      parts.push({ text: m[0], kind: "link" });
    }
    at = m.index + m[0].length;
  }
  if (at < run.length) parts.push({ text: run.slice(at), kind: base });
  return parts;
}

/** A fenced block split into marker lines (the ``` fences, hidden by CSS)
 *  and the code body — every character preserved, in order. */
export function fenceParts(block: string): { text: string; marker: boolean }[] {
  const parts: { text: string; marker: boolean }[] = [];
  const open = block.match(/^```[\w+-]{0,12}\n?/);
  let body = block;
  if (open) {
    parts.push({ text: open[0], marker: true });
    body = block.slice(open[0].length);
  }
  const close = body.match(/(^|\n)```[\w+-]{0,12}\n?$/);
  if (close) {
    const at = close.index! + (close[1] ? 1 : 0);
    if (at > 0) parts.push({ text: body.slice(0, at), marker: false });
    parts.push({ text: body.slice(at), marker: true });
  } else if (body) {
    parts.push({ text: body, marker: false });
  }
  return parts;
}

/** Inline code pairs in one run of text: `x` becomes marker + code +
 *  marker. A lone backtick pairs with nothing and stays plain. */
function codeParts(run: string, base: "plain" | "quote"): InlinePart[] {
  const parts: InlinePart[] = [];
  const pair = /`([^`\n]+)`/g;
  let at = 0;
  let m: RegExpExecArray | null;
  while ((m = pair.exec(run))) {
    if (m.index > at) parts.push(...linkParts(run.slice(at, m.index), base));
    parts.push({ text: "`", kind: "marker" });
    parts.push({ text: m[1], kind: "code" });
    parts.push({ text: "`", kind: "marker" });
    at = m.index + m[0].length;
  }
  if (at < run.length) parts.push(...linkParts(run.slice(at), base));
  return parts;
}

/** An unfenced run, line-aware: "> " at line start becomes a quote (the
 *  marker hidden, a bar painted in its place); `code` pairs become chips.
 *  Glyph-for-glyph identical to the input — only kinds are added. */
export function inlineParts(run: string): InlinePart[] {
  const parts: InlinePart[] = [];
  let at = 0;
  while (at <= run.length) {
    const end = run.indexOf("\n", at);
    const stop = end === -1 ? run.length : end;
    const line = run.slice(at, stop);
    const quoted = line.startsWith("> ");
    if (quoted) {
      parts.push({ text: "> ", kind: "qmark" });
      parts.push(...codeParts(line.slice(2), "quote"));
    } else if (line) {
      parts.push(...codeParts(line, "plain"));
    }
    if (end === -1) break;
    parts.push({ text: "\n", kind: "plain" });
    at = end + 1;
  }
  return parts;
}

/** One painted span of the mirror: exact text, css class, and — for
 *  parts inside a fenced block — the block's index, so the render can
 *  group them under one measurable wrapper (the full-width box). */
export type Painted = { text: string; cls: string; fid?: number };

/** The whole draft as mirror spans, Obsidian-live-preview style: markers
 *  paint invisible EXCEPT on the caret's line, where they reveal (dim) so
 *  the structure is never navigated blind. Glyph-for-glyph identical. */
export function paint(text: string, caret: number): Painted[] {
  const lineStart = text.lastIndexOf("\n", Math.max(0, caret - 1)) + 1;
  const lineEndRaw = text.indexOf("\n", caret);
  const lineEnd = lineEndRaw === -1 ? text.length : lineEndRaw;
  const out: Painted[] = [];
  let at = 0;
  const push = (part: string, cls: string, fid?: number) => {
    const from = at;
    at += part.length;
    // A marker on the caret's line shows itself, dimmed, for editing.
    if (cls.includes("cm-marker") || cls === "cm-qmark") {
      const onCaretLine = from <= lineEnd && at > lineStart;
      if (onCaretLine) cls = `${cls} cm-live`;
    }
    out.push({ text: part, cls, fid });
  };
  let fid = 0;
  for (const seg of fenceSegments(text)) {
    if (seg.fenced) {
      for (const p of fenceParts(seg.text)) {
        push(p.text, p.marker ? "cm-marker" : "cm-fence", fid);
      }
      fid += 1;
    } else {
      for (const p of inlineParts(seg.text)) {
        push(p.text, p.kind === "plain" ? "" : p.kind === "marker" ? "cm-marker" : `cm-${p.kind}`);
      }
    }
  }
  return out;
}

/** Consecutive painted parts grouped by fenced block, so each block gets
 *  ONE wrapper span the box-measuring effect can read. */
export function groupBlocks(parts: Painted[]): { fid?: number; parts: Painted[] }[] {
  const groups: { fid?: number; parts: Painted[] }[] = [];
  for (const p of parts) {
    const last = groups[groups.length - 1];
    if (last && last.fid === p.fid) last.parts.push(p);
    else groups.push({ fid: p.fid, parts: [p] });
  }
  return groups;
}
