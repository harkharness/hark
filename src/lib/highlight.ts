// Shared highlight.js core for the file viewer. Only the grammars this
// app actually shows, to keep the bundle small (Markdown.tsx keeps its
// own list for rehype-highlight).

import hljs from "highlight.js/lib/core";
import bash from "highlight.js/lib/languages/bash";
import css from "highlight.js/lib/languages/css";
import diff from "highlight.js/lib/languages/diff";
import ini from "highlight.js/lib/languages/ini";
import json from "highlight.js/lib/languages/json";
import markdown from "highlight.js/lib/languages/markdown";
import python from "highlight.js/lib/languages/python";
import rust from "highlight.js/lib/languages/rust";
import sql from "highlight.js/lib/languages/sql";
import typescript from "highlight.js/lib/languages/typescript";
import xml from "highlight.js/lib/languages/xml";
import yaml from "highlight.js/lib/languages/yaml";

hljs.registerLanguage("bash", bash);
hljs.registerLanguage("css", css);
hljs.registerLanguage("diff", diff);
hljs.registerLanguage("ini", ini);
hljs.registerLanguage("json", json);
hljs.registerLanguage("markdown", markdown);
hljs.registerLanguage("python", python);
hljs.registerLanguage("rust", rust);
hljs.registerLanguage("sql", sql);
hljs.registerLanguage("typescript", typescript);
hljs.registerLanguage("xml", xml);
hljs.registerLanguage("yaml", yaml);

const EXT_TO_LANG: Record<string, string> = {
  sh: "bash", bash: "bash", zsh: "bash",
  css: "css",
  diff: "diff", patch: "diff",
  toml: "ini", ini: "ini",
  json: "json",
  md: "markdown", markdown: "markdown",
  py: "python",
  rs: "rust",
  sql: "sql",
  ts: "typescript", tsx: "typescript", js: "typescript", jsx: "typescript", mjs: "typescript",
  html: "xml", xml: "xml", svg: "xml",
  yml: "yaml", yaml: "yaml",
};

export function languageFor(path: string): string | null {
  const ext = path.split(".").pop()?.toLowerCase() ?? "";
  return EXT_TO_LANG[ext] ?? null;
}

function escapeHtml(text: string): string {
  return text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

/** Highlighted HTML for a file's content (plain-escaped when unknown). */
export function highlightFile(path: string, content: string): string {
  const lang = languageFor(path);
  if (!lang) return escapeHtml(content);
  try {
    return hljs.highlight(content, { language: lang }).value;
  } catch {
    return escapeHtml(content);
  }
}
