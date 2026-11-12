---
title: Translations
description: The app speaks Portuguese and English; these docs speak three languages, and a gate keeps them in step.
sidebar:
  order: 6
---

## The app

The interface speaks Portuguese and English (`src/lib/i18n.ts`); a new language is a new table with the same keys. Titles, headings, menu items, buttons, placeholders and tooltips start with a capital letter; only text that lives inside a sentence stays lowercase. The spoken strings have their own table.

## These docs

The pages live in the repository under `docs/site/<lang>/`, one tree per language: `en` is the source, `pt-br` and `es` are translations with the same file paths. A page without a translation falls back to English on the site with a notice; the gate below makes sure that never lasts.

### The gate

`node scripts/docs-check.mjs` runs on every push and pull request, and before every release. It fails when:

- an English page has no twin in a language, or a translation has no English source;
- a translation was made from an older English page: each translation carries `source_hash` in its frontmatter, the hash of the English page it was translated from;
- a translation's skeleton drifted: heading levels, number of code blocks, number of internal links, frontmatter keys.

### Translating a page

1. Edit or add the English page.
2. Write the same page in `pt-br/` and `es/`, same path, same headings and code blocks, links pointing at the same language's pages (`/docs/pt-br/…`).
3. Stamp the translations with the hash of the English page:

```bash
node scripts/docs-check.mjs --stamp docs/site/pt-br/use/start/install.md docs/site/es/use/start/install.md
```

4. Run `node scripts/docs-check.mjs`; it says when everything is in step.

The check does not judge the quality of a translation. That is what review on the pull request is for.
