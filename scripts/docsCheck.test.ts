import { describe, expect, it } from "vitest";
import { check, skeleton, sourceHash, stamp } from "./docs-check-lib.mjs";

const en = `---\ntitle: Install\ndescription: One command.\n---\n\n## One command\n\n\`\`\`bash\ncurl x | bash\n\`\`\`\n\n### Prefer to click?\n\nSee [first run](/docs/en/use/first-run/).\n`;
const pt = (hash: string, body = `## Um comando\n\n\`\`\`bash\ncurl x | bash\n\`\`\`\n\n### Prefere clicar?\n\nVeja [a primeira execução](/docs/pt-br/use/first-run/).\n`) =>
  `---\ntitle: Instalar\ndescription: Um comando.\nsource_hash: ${hash}\n---\n\n${body}`;

describe("the skeleton of a page", () => {
  it("is its heading levels, code blocks, frontmatter keys and internal links", () => {
    expect(skeleton(en)).toEqual({ headings: [2, 3], codeBlocks: 1, frontmatter: ["description", "title"], links: 1 });
  });
  it("ignores source_hash among the keys, since only translations carry it", () => {
    expect(skeleton(pt("abc")).frontmatter).toEqual(["description", "title"]);
  });
});

describe("the source hash", () => {
  it("is stable and ignores trailing whitespace", () => {
    expect(sourceHash(en)).toBe(sourceHash(en + "\n\n  "));
    expect(sourceHash(en)).toMatch(/^[0-9a-f]{12}$/);
  });
  it("stamp writes it into a translation's frontmatter, replacing an old one", () => {
    const out = stamp(pt("old"), en);
    expect(out).toContain(`source_hash: ${sourceHash(en)}`);
    expect(out).not.toContain("old");
    expect(stamp(pt("x").replace("source_hash: x\n", ""), en)).toContain(`source_hash: ${sourceHash(en)}`);
  });
});

describe("check: three languages, one tree", () => {
  const h = sourceHash(en);
  it("passes when every page exists in every language, fresh and with the same skeleton", () => {
    expect(check({ en: { "use/install.md": en }, "pt-br": { "use/install.md": pt(h) }, es: { "use/install.md": pt(h) } })).toEqual([]);
  });
  it("names a missing translation and an orphan", () => {
    const errors = check({ en: { "use/install.md": en }, "pt-br": {}, es: { "use/install.md": pt(h), "use/extra.md": pt(h) } });
    expect(errors).toContain("pt-br/use/install.md: missing (en/use/install.md has no translation)");
    expect(errors).toContain("es/use/extra.md: orphan (no en/use/extra.md)");
  });
  it("names a stale translation, with both hashes", () => {
    const errors = check({ en: { "use/install.md": en }, "pt-br": { "use/install.md": pt("000000000000") }, es: { "use/install.md": pt(h) } });
    expect(errors).toEqual([`pt-br/use/install.md: stale (source_hash 000000000000, en is now ${h})`]);
  });
  it("names a translation without a hash", () => {
    const errors = check({ en: { "use/install.md": en }, "pt-br": { "use/install.md": pt(h).replace(`source_hash: ${h}\n`, "") }, es: { "use/install.md": pt(h) } });
    expect(errors).toEqual(["pt-br/use/install.md: no source_hash (stamp it after translating)"]);
  });
  it("names a skeleton that drifted: headings, code blocks, links", () => {
    const errors = check({ en: { "use/install.md": en }, "pt-br": { "use/install.md": pt(h, "## Um comando\n\nSem código, sem link.\n") }, es: { "use/install.md": pt(h) } });
    expect(errors).toEqual([
      "pt-br/use/install.md: headings differ (en h2,h3 vs h2)",
      "pt-br/use/install.md: code blocks differ (en 1 vs 0)",
      "pt-br/use/install.md: internal links differ (en 1 vs 0)",
    ]);
  });
  it("a language folder that does not exist counts as every page missing", () => {
    expect(check({ en: { "a.md": en } }).length).toBe(2);
  });
});
