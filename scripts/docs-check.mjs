#!/usr/bin/env node
// The translation gate, on disk. Fails (exit 1) when docs/site/<lang>
// disagree; `--stamp <translation>…` writes the hash of the English page
// into those translations after you updated them.
//
//   node scripts/docs-check.mjs [--root docs/site]
//   node scripts/docs-check.mjs --stamp docs/site/pt-br/use/install.md …
import { readdirSync, readFileSync, writeFileSync, existsSync, statSync } from "node:fs";
import { join, relative, sep } from "node:path";
import { LANGS, check, stamp } from "./docs-check-lib.mjs";

const args = process.argv.slice(2);
const root = args.includes("--root") ? args[args.indexOf("--root") + 1] : "docs/site";

function pages(dir) {
  const out = {};
  if (!existsSync(dir)) return out;
  const walk = (d) => {
    for (const name of readdirSync(d)) {
      const p = join(d, name);
      if (statSync(p).isDirectory()) walk(p);
      else if (/\.mdx?$/.test(name)) out[relative(dir, p).split(sep).join("/")] = readFileSync(p, "utf8");
    }
  };
  walk(dir);
  return out;
}

if (args.includes("--stamp")) {
  const files = args.slice(args.indexOf("--stamp") + 1).filter((a) => !a.startsWith("--"));
  for (const file of files) {
    const m = new RegExp(`^(.*)/(${LANGS.join("|")})/(.+)$`).exec(file.split(sep).join("/"));
    if (!m || m[2] === "en") {
      console.error(`docs-check: ${file} is not a translation under ${root}/<lang>/`);
      process.exit(2);
    }
    const source = join(m[1], "en", m[3]);
    if (!existsSync(source)) {
      console.error(`docs-check: ${file} has no English source at ${source}`);
      process.exit(2);
    }
    writeFileSync(file, stamp(readFileSync(file, "utf8"), readFileSync(source, "utf8")));
    console.log(`stamped ${file}`);
  }
  process.exit(0);
}

const tree = Object.fromEntries(LANGS.map((l) => [l, pages(join(root, l))]));
const errors = check(tree);
const n = Object.keys(tree.en).length;
if (errors.length) {
  for (const e of errors) console.error(`docs-check: ${e}`);
  console.error(`docs-check: ${errors.length} problem(s) over ${n} English page(s). Translate, then stamp: node scripts/docs-check.mjs --stamp <file>`);
  process.exit(1);
}
console.log(`docs-check: ${n} page(s) in ${LANGS.length} languages, all in step`);
