// The translation gate of the docs site (docs/site/<lang>/…), pure: every
// page the English tree has must exist in every other language, carry the
// hash of the English page it was translated from (stale otherwise), and
// keep the same skeleton (heading levels, code blocks, internal links,
// frontmatter keys). Nothing here reads the disk: docs-check.mjs does.
import { createHash } from "node:crypto";

export const LANGS = ["en", "pt-br", "es"];
const SOURCE = "en";

/** Frontmatter (between the first two `---` lines) and the body after it. */
function split(md) {
  const m = /^---\r?\n([\s\S]*?)\r?\n---\r?\n?/.exec(md);
  if (!m) return { front: "", body: md, keys: [] };
  const keys = m[1].split(/\r?\n/).map((l) => /^([\w-]+)\s*:/.exec(l)?.[1]).filter(Boolean);
  return { front: m[1], body: md.slice(m[0].length), keys };
}

/** What a translation must keep: the shape of the page, not its words. */
export function skeleton(md) {
  const { body, keys } = split(md);
  const headings = [];
  let codeBlocks = 0;
  let inFence = false;
  for (const line of body.split(/\r?\n/)) {
    if (/^(```|~~~)/.test(line)) {
      if (!inFence) codeBlocks += 1;
      inFence = !inFence;
      continue;
    }
    if (inFence) continue;
    const h = /^(#{1,6})\s+\S/.exec(line);
    if (h) headings.push(h[1].length);
  }
  const links = (body.match(/\]\(\/docs\//g) ?? []).length;
  return { headings, codeBlocks, frontmatter: keys.filter((k) => k !== "source_hash").sort(), links };
}

/** The identity of an English page as its translations know it. */
export function sourceHash(md) {
  return createHash("sha256").update(md.trimEnd()).digest("hex").slice(0, 12);
}

function hashOf(md) {
  return /^source_hash:\s*([0-9a-f]+)\s*$/m.exec(split(md).front)?.[1] ?? null;
}

/** The translation with the hash of `source` written into its frontmatter. */
export function stamp(translation, source) {
  const h = sourceHash(source);
  const { front, body } = split(translation);
  if (!front && !/^---/.test(translation)) return `---\nsource_hash: ${h}\n---\n\n${translation}`;
  const lines = front.split(/\r?\n/).filter((l) => !/^source_hash\s*:/.test(l));
  lines.push(`source_hash: ${h}`);
  return `---\n${lines.join("\n")}\n---\n${body.startsWith("\n") ? "" : "\n"}${body}`;
}

/**
 * The errors of a tree `{ lang: { "path.md": content } }`, one line each,
 * `[]` when the three languages agree. Order: by English path, then by
 * language, then orphans.
 */
export function check(tree, langs = LANGS) {
  const errors = [];
  const en = tree[SOURCE] ?? {};
  const others = langs.filter((l) => l !== SOURCE);
  for (const path of Object.keys(en).sort()) {
    const want = sourceHash(en[path]);
    const shape = skeleton(en[path]);
    for (const lang of others) {
      const md = tree[lang]?.[path];
      if (md === undefined) {
        errors.push(`${lang}/${path}: missing (${SOURCE}/${path} has no translation)`);
        continue;
      }
      const has = hashOf(md);
      if (!has) errors.push(`${lang}/${path}: no source_hash (stamp it after translating)`);
      else if (has !== want) errors.push(`${lang}/${path}: stale (source_hash ${has}, ${SOURCE} is now ${want})`);
      const s = skeleton(md);
      if (s.headings.join() !== shape.headings.join())
        errors.push(`${lang}/${path}: headings differ (${SOURCE} ${shape.headings.map((n) => `h${n}`).join(",")} vs ${s.headings.map((n) => `h${n}`).join(",")})`);
      if (s.codeBlocks !== shape.codeBlocks) errors.push(`${lang}/${path}: code blocks differ (${SOURCE} ${shape.codeBlocks} vs ${s.codeBlocks})`);
      if (s.links !== shape.links) errors.push(`${lang}/${path}: internal links differ (${SOURCE} ${shape.links} vs ${s.links})`);
      if (s.frontmatter.join() !== shape.frontmatter.join())
        errors.push(`${lang}/${path}: frontmatter keys differ (${SOURCE} ${shape.frontmatter.join(",")} vs ${s.frontmatter.join(",")})`);
    }
  }
  for (const lang of others) {
    for (const path of Object.keys(tree[lang] ?? {}).sort()) {
      if (!(path in en)) errors.push(`${lang}/${path}: orphan (no ${SOURCE}/${path})`);
    }
  }
  return errors;
}
