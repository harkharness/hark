// Generates the artboards of the "Arquivos" unified-window canvas.
// Every value below is lifted from src/styles.css (tokens, .frame, .filetab,
// .filespanel, .tree-row, .viewer, .md) — the mockups draw the app's own
// components, not an approximation of them. Run: node build.mjs
import { writeFileSync } from "node:fs";

const T = {
  bgDeep: "#07090d",
  bg: "#0b0e12",
  panel: "#11151b",
  panel2: "#161c25",
  strong: "#e9eef6",
  text: "#d7dde6",
  dim: "#7a8698",
  accent: "#7aa2f7",
  ok: "#9ece6a",
  warn: "#e0af68",
  err: "#f7768e",
  border: "rgba(255,255,255,.07)",
  borderStrong: "rgba(255,255,255,.13)",
  keyword: "#bb9af7",
  meta: "#7dcfff",
  ui: `-apple-system, "SF Pro Text", "Segoe UI", system-ui, sans-serif`,
  mono: `"SF Mono", ui-monospace, Menlo, monospace`,
};

// lucide paths (24×24 grid), the icon set the app uses.
const PATHS = {
  search: `<circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/>`,
  chevronDown: `<path d="m6 9 6 6 6-6"/>`,
  chevronRight: `<path d="m9 18 6-6-6-6"/>`,
  x: `<path d="M18 6 6 18"/><path d="m6 6 12 12"/>`,
  maximize: `<polyline points="15 3 21 3 21 9"/><polyline points="9 21 3 21 3 15"/><line x1="21" x2="14" y1="3" y2="10"/><line x1="3" x2="10" y1="21" y2="14"/>`,
  panelLeftClose: `<rect width="18" height="18" x="3" y="3" rx="2"/><path d="M9 3v18"/><path d="m16 15-3-3 3-3"/>`,
  panelLeftOpen: `<rect width="18" height="18" x="3" y="3" rx="2"/><path d="M9 3v18"/><path d="m14 9 3 3-3 3"/>`,
  pencil: `<path d="M21.174 6.812a1 1 0 0 0-3.986-3.987L3.842 16.174a2 2 0 0 0-.5.83l-1.321 4.352a.5.5 0 0 0 .623.622l4.353-1.32a2 2 0 0 0 .83-.497z"/><path d="m15 5 4 4"/>`,
  eye: `<path d="M2.062 12.348a1 1 0 0 1 0-.696 10.75 10.75 0 0 1 19.876 0 1 1 0 0 1 0 .696 10.75 10.75 0 0 1-19.876 0"/><circle cx="12" cy="12" r="3"/>`,
  save: `<path d="M15.2 3a2 2 0 0 1 1.4.6l3.8 3.8a2 2 0 0 1 .6 1.4V19a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z"/><path d="M17 21v-7a1 1 0 0 0-1-1H8a1 1 0 0 0-1 1v7"/><path d="M7 3v4a1 1 0 0 0 1 1h7"/>`,
  folderOpen: `<path d="m6 14 1.5-2.9A2 2 0 0 1 9.24 10H20a2 2 0 0 1 1.94 2.5l-1.54 6a2 2 0 0 1-1.95 1.5H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h3.9a2 2 0 0 1 1.69.9l.81 1.2a2 2 0 0 0 1.67.9H18a2 2 0 0 1 2 2v2"/>`,
  folderTree: `<path d="M20 10a1 1 0 0 0 1-1V6a1 1 0 0 0-1-1h-2.5a1 1 0 0 1-.8-.4l-.9-1.2A1 1 0 0 0 15 3h-2a1 1 0 0 0-1 1v5a1 1 0 0 0 1 1Z"/><path d="M20 21a1 1 0 0 0 1-1v-3a1 1 0 0 0-1-1h-2.9a1 1 0 0 1-.88-.55l-.42-.85a1 1 0 0 0-.92-.6H13a1 1 0 0 0-1 1v5a1 1 0 0 0 1 1Z"/><path d="M3 5a2 2 0 0 0 2 2h3"/><path d="M3 3v13a2 2 0 0 0 2 2h3"/>`,
  kanban: `<rect x="3" y="3" width="18" height="18" rx="2"/><path d="M9 3v18M15 3v18"/>`,
  message: `<path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/>`,
  mic: `<path d="M12 2a3 3 0 0 1 3 3v6a3 3 0 0 1-6 0V5a3 3 0 0 1 3-3zM5 11a7 7 0 0 0 14 0M12 18v4"/>`,
  send: `<path d="M3 12l18-8-8 18-2-8z"/>`,
  terminal: `<path d="M4 17l6-6-6-6M12 19h8"/>`,
  panelLeft: `<rect width="18" height="18" x="3" y="3" rx="2"/><path d="M9 3v18"/>`,
  plus: `<path d="M5 12h14M12 5v14"/>`,
};

const icon = (name, size = 13, color = "currentColor", sw = 2) =>
  `<svg width="${size}" height="${size}" viewBox="0 0 24 24" fill="none" stroke="${color}" stroke-width="${sw}" stroke-linecap="round" stroke-linejoin="round" style="flex: 0 0 auto">${PATHS[name]}</svg>`;

const esc = (s) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

// ---- frame chrome (.frame / .frame-head / .frame-title / .filetab / .frame-ctl) ----

// .frame-actions button.frame-ctl: 24×22, radius 7, dim, opacity .45 at rest.
// Drawn a touch stronger so a still picture keeps them legible.
const ctl = (name, title) =>
  `<span title="${title}" style="width: 24px; height: 22px; border-radius: 7px; color: ${T.dim}; opacity: .6; display: inline-flex; align-items: center; justify-content: center">${icon(name, 13)}</span>`;

function tabsStrip(tabs) {
  if (!tabs.length) return "";
  const items = tabs
    .map((tab) => {
      const on = tab.on;
      return `<span title="${esc(tab.abs)}" style="display: inline-flex; align-items: center; gap: 6px; color: ${on ? T.strong : T.dim}; font-size: 11.5px; padding: 3px 10px; border-radius: 7px; white-space: nowrap; background: ${on ? T.panel2 : "transparent"}; flex: 0 0 auto">${
        tab.dirty ? `<span style="width: 8px; height: 8px; border-radius: 50%; background: ${T.warn}; flex: 0 0 auto"></span>` : ""
      }${esc(tab.name)}<span style="display: inline-flex; color: ${T.dim}; opacity: ${on ? ".6" : "0"}; padding: 0 2px">${icon("x", 11)}</span></span>`;
    })
    .join("");
  return `<div style="display: flex; align-items: center; gap: 3px; flex: 1 1 auto; min-width: 0; overflow: hidden">${items}</div>`;
}

/** .frame-head: 6px 8px 6px 12px, gap 8 — 34px tall. The ONE new control is
 * the tree toggle, first thing after the (invisible at rest) grip. */
function frameHead({ treeOpen, tabs, rail = true, title = "Arquivos" }) {
  const toggle = `<span title="${treeOpen ? "recolher a árvore" : "abrir a árvore"}" style="width: 24px; height: 22px; border-radius: 7px; color: ${treeOpen ? T.text : T.dim}; display: inline-flex; align-items: center; justify-content: center; margin-left: -6px; flex: 0 0 auto">${icon(treeOpen ? "panelLeftClose" : "panelLeftOpen", 13)}</span>`;
  return `<div style="display: flex; align-items: center; gap: 8px; padding: 6px 8px 6px 12px; flex: 0 0 auto">
    ${toggle}
    <span style="color: ${T.strong}; font-size: 12.5px; font-weight: 600; flex: 0 0 auto">${title}</span>
    ${tabsStrip(tabs)}
    <span style="margin-left: auto; display: flex; align-items: center; gap: 2px; flex: 0 0 auto">
      ${rail ? ctl("chevronDown", "recolher painel") : ""}
      ${ctl("maximize", "tela cheia")}
      ${ctl("x", "fechar janela (continua rodando por baixo)")}
    </span>
  </div>`;
}

// ---- the tree column (.filespanel-search / .filespanel-proj / .tree-row) ----

const VOX_TREE = [
  ["d", ".design"],
  ["d", ".github"],
  ["d", "crates"],
  ["d", "dist"],
  ["d", "docs"],
  ["d", "node_modules"],
  ["d", "releases"],
  ["d", "scripts"],
  ["d", "spikes"],
  ["d", "src"],
  ["d", "src-tauri"],
  ["d", "target"],
  ["d", "tests"],
  ["f", ".gitignore"],
  ["f", ".gitleaks.toml"],
  ["f", ".gitmodules"],
  ["f", "Cargo.lock"],
  ["f", "Cargo.toml"],
  ["f", "index.html"],
  ["f", "install.sh"],
  ["f", "LICENSE-APACHE"],
  ["f", "LICENSE-MIT"],
  ["f", "package-lock.json"],
  ["f", "package.json"],
  ["f", "README.md"],
];

function treeRow(kind, name, { depth = 0, active = false } = {}) {
  // .tree-row: block, 12px, padding 2px 8px (+12 per depth), radius 4; dirs dim
  // with the text caret, files in --text. NEW: the open file gets the tab's
  // "on" look (panel-2 fill, strong text).
  const color = active ? T.strong : kind === "d" ? T.dim : T.text;
  const bg = active ? T.panel2 : "transparent";
  const caret = kind === "d" ? `<span style="color: ${T.dim}; margin-right: 4px">▸</span>` : "";
  return `<div style="display: block; width: 100%; color: ${color}; font-size: 12px; padding: 2px 8px 2px ${8 + depth * 12}px; border-radius: 4px; background: ${bg}; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; box-sizing: border-box">${caret}${esc(name)}</div>`;
}

function projectRow(name, open) {
  // .filespanel-proj: 12px --text, gap 6, padding 6px 4px, chevron 13.
  return `<div style="display: flex; align-items: center; gap: 6px; color: ${T.text}; font-size: 12px; padding: 6px 4px">${icon(open ? "chevronDown" : "chevronRight", 13)}${esc(name)}</div>`;
}

function searchRow({ query = "" } = {}) {
  // .filespanel-search: gap 8, padding 8px 12px (10 here: the column is narrow),
  // hairline below; input = panel-2 fill, radius 8, 7px 10px, 12px.
  const value = query
    ? `<span style="color: ${T.text}">${esc(query)}</span>`
    : `<span style="color: ${T.dim}">filtrar arquivos…</span>`;
  const clear = query ? `<span style="color: ${T.dim}; display: inline-flex">${icon("x", 11)}</span>` : "";
  const focus = query ? `color-mix(in srgb, ${T.accent} 45%, transparent)` : "transparent";
  return `<div style="display: flex; align-items: center; gap: 8px; padding: 8px 10px; border-bottom: 1px solid ${T.border}; color: ${T.dim}; flex: 0 0 auto">
    ${icon("search", 13)}
    <div style="flex: 1; display: flex; align-items: center; gap: 6px; background: ${T.panel2}; border: 1px solid ${focus}; border-radius: 8px; font-size: 12px; padding: 6px 10px; min-width: 0"><span style="flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap">${value}</span>${clear}</div>
  </div>`;
}

function treeColumn({ activePath = "README.md", width = 232 } = {}) {
  const rows = VOX_TREE.map(([kind, name]) => treeRow(kind, name, { active: kind === "f" && name === activePath })).join("");
  return `<div style="width: ${width}px; flex: 0 0 auto; display: flex; flex-direction: column; min-height: 0; background: ${T.panel}">
    ${searchRow()}
    <div style="flex: 1; overflow: hidden; padding: 6px 8px; display: flex; flex-direction: column">
      ${projectRow("workspace-fabrica", false)}
      ${projectRow("vox", true)}
      <div style="margin: 2px 0 8px">${rows}</div>
    </div>
  </div>`;
}

/** The filter replaces the trees (as today); a hit is basename + dim folder. */
function hitsColumn({ query, hits, width = 232 }) {
  const rows = hits
    .map(
      (hit, i) => `<div style="display: flex; align-items: baseline; gap: 8px; width: 100%; color: ${T.text}; font-family: ${T.mono}; font-size: 12px; padding: 6px 10px; border-radius: 4px; background: ${i === 0 ? "rgba(122,162,247,.15)" : "transparent"}; box-sizing: border-box; min-width: 0">
        <span style="flex: 0 1 auto; overflow: hidden; text-overflow: ellipsis; white-space: nowrap">${esc(hit.name)}</span>
        <span style="color: ${T.dim}; font-size: 11px; flex: 1 1 auto; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; direction: rtl; text-align: left">${esc(hit.dir)}</span>
      </div>`,
    )
    .join("");
  return `<div style="width: ${width}px; flex: 0 0 auto; display: flex; flex-direction: column; min-height: 0; background: ${T.panel}">
    ${searchRow({ query })}
    <div style="padding: 6px; overflow: hidden">${rows}</div>
  </div>`;
}

// ---- the viewer (.viewer-head / .viewer-code / .viewer-md) ----

function viewerButton(name, title, { disabled = false } = {}) {
  // .viewer-actions button: panel-2 fill, radius 6, 4px 10px, icon 13.
  return `<span title="${title}" style="display: inline-flex; align-items: center; background: ${T.panel2}; color: ${T.text}; border-radius: 6px; padding: 4px 10px; opacity: ${disabled ? ".45" : "1"}">${icon(name, 13)}</span>`;
}

function viewerHead({ project = "vox", rel = "README.md", mode = "read", dirty = false, status = "" }) {
  const actions =
    mode === "edit"
      ? viewerButton("save", "salvar (Cmd+S)", { disabled: !dirty }) + viewerButton("eye", "voltar à visualização")
      : viewerButton("pencil", "editar localmente");
  return `<div style="display: flex; align-items: center; gap: 6px; padding: 8px 12px; border-bottom: 1px solid ${T.border}; font-size: 12px; flex: 0 0 auto; min-width: 0">
    <span style="color: ${T.dim}; flex: 0 0 auto">${esc(project)}/</span>
    <span title="~/Projects/${esc(project)}/${esc(rel)}" style="color: ${T.accent}; font-family: ${T.mono}; overflow: hidden; text-overflow: ellipsis; white-space: nowrap">${esc(rel)}</span>
    ${dirty ? `<span title="edições não salvas (Cmd+S)" style="width: 8px; height: 8px; border-radius: 50%; background: ${T.warn}; flex: 0 0 auto"></span>` : ""}
    ${status ? `<span style="color: ${T.ok}; font-size: 11px">${esc(status)}</span>` : ""}
    <span style="margin-left: auto; display: flex; gap: 6px; flex: 0 0 auto">${actions}</span>
  </div>`;
}

const README_SRC = [
  "# Hark",
  "",
  "[![test](https://github.com/jhonmike/hark-harness/actions/workflows/test.yml/badge.svg)](https://github.com/jhonmike/hark-harness/actions/workflows/test.yml)",
  "",
  "A local, voice-first cockpit for [Claude Code](https://code.claude.com). Talk to",
  "your machine like JARVIS: ask what you were working on, hear the answer out",
  "loud, and dispatch real work into your existing Claude Code sessions — by voice",
  "or text, across every project on your disk.",
  "",
  "Hark never calls the Anthropic API directly. It drives the `claude` CLI you",
  "already have installed and authenticated, so all usage draws from your existing",
  "subscription. No API key, no separate billing, no telemetry.",
  "",
  "> Status: a working desktop app in active development (macOS, Apple Silicon",
  "> and Intel; Linux is on the backlog). Expect sharp edges; the voice loop,",
  "> windows, boards, cost ledger and the persistent assistant chat all work today.",
  "",
  "## Install (from a release)",
  "",
  "Binaries ship from the PUBLIC releases-only repo",
  "([harkharness/hark](https://github.com/harkharness/hark)) — this source repo",
  "stays private and is never exposed through them. Anyone can install with:",
  "",
  "```bash",
  "curl -fsSL https://harkharness.web.app/install.sh | bash",
  "```",
  "",
  "That downloads the latest public release for your architecture, installs the",
  "`hark` CLI into `~/.local/bin`, drops `hark.app` into `/Applications` and",
  "clears the quarantine bit (the app is not code-signed yet).",
  "",
  "After installing: `hark setup` downloads the whisper speech model (~466MB),",
];

const INSTALL_SRC = [
  "#!/usr/bin/env bash",
  "# Hark installer — thin wrapper over the public one. Binaries ship from the",
  "# public releases-only repo (harkharness/hark); this private repo never",
  "# exposes source through them.",
  "set -euo pipefail",
  'exec /bin/bash -c "$(curl -fsSL https://harkharness.web.app/install.sh)"',
  "",
];

// highlight.js tokens as the app paints them (.hljs-* rules in styles.css).
function mdLine(raw) {
  if (/^#{1,6} /.test(raw)) return `<span style="color: ${T.accent}; font-weight: 700">${esc(raw)}</span>`;
  if (/^> ?/.test(raw)) return `<span style="color: ${T.dim}; font-style: italic">${esc(raw)}</span>`;
  let s = esc(raw);
  s = s.replace(/\[([^\]]*)\]\(([^)]*)\)/g, (_m, text, url) => `[<span style="color: ${T.ok}">${text}</span>](${url})`);
  s = s.replace(/\*\*([^*]+)\*\*/g, `<span style="font-weight: 700">**$1**</span>`);
  return s;
}

function shLine(raw) {
  if (raw.startsWith("#!")) return `<span style="color: ${T.meta}">${esc(raw)}</span>`;
  if (raw.startsWith("#")) return `<span style="color: ${T.dim}; font-style: italic">${esc(raw)}</span>`;
  // Split the string literal off FIRST: the keyword span carries quotes of
  // its own, and a string regex run after it swallowed the whole line.
  const quote = raw.indexOf('"');
  const head = quote >= 0 ? raw.slice(0, quote) : raw;
  const str = quote >= 0 ? raw.slice(quote) : "";
  const painted = esc(head).replace(/^(set|exec)\b/, `<span style="color: ${T.keyword}">$1</span>`);
  return painted + (str ? `<span style="color: ${T.ok}">${esc(str)}</span>` : "");
}

/** Source with the NEW gutter: .viewer-code metrics (12px mono, lh 1.55,
 * 12px 14px padding, --code-bg), numbers dim 11px right-aligned, the
 * target line ("README.md:21" clicked in the chat) tinted with the accent. */
function sourceView(lines, { lang = "md", targetLine = 0, height } = {}) {
  const paint = lang === "sh" ? shLine : mdLine;
  const rows = lines
    .map((line, i) => {
      const n = i + 1;
      const hit = n === targetLine;
      return `<div style="display: flex; gap: 12px; line-height: 1.55; background: ${hit ? "rgba(122,162,247,.10)" : "transparent"}; margin: 0 -14px; padding: 0 14px"><span style="width: 30px; flex: 0 0 auto; text-align: right; color: ${hit ? T.accent : T.dim}; font-size: 11px; user-select: none">${n}</span><span style="white-space: pre; flex: 1; min-width: 0; overflow: hidden">${paint(line)}</span></div>`;
    })
    .join("");
  return `<div style="flex: 1; min-height: 0; overflow: hidden; margin: 0; padding: 12px 14px; font-family: ${T.mono}; font-size: 12px; background: ${T.bg}; color: ${T.text}; tab-size: 4${height ? `; height: ${height}px` : ""}">${rows}</div>`;
}

const code = (s) => `<code style="background: rgba(122,162,247,.12); color: ${T.accent}; padding: 1px 5px; border-radius: 4px; font-size: 12px; font-family: ${T.mono}">${esc(s)}</code>`;
const link = (s, href = "#") => `<a href="${href}" style="color: ${T.accent}; text-decoration: none">${esc(s)}</a>`;
const p = (html) => `<p style="margin: 5px 0; line-height: 1.55">${html}</p>`;

/** The README as .viewer-md > .md paints it. */
function renderedReadme() {
  const pre = (cmd) => `<pre style="background: color-mix(in srgb, ${T.text} 4%, ${T.bg}); border: 1px solid ${T.border}; border-radius: 10px; padding: 11px 13px; overflow-x: auto; margin: 8px 0; font-family: ${T.mono}; font-size: 12px; line-height: 1.5; color: ${T.text}">${esc(cmd)}</pre>`;
  return `<div style="flex: 1; min-height: 0; overflow: hidden; padding: 14px 16px; font-size: 13px">
    <h1 style="color: ${T.strong}; margin: 0 0 4px; line-height: 1.3; font-size: 16px; font-weight: 600">Hark</h1>
    ${p(link("test"))}
    ${p(`A local, voice-first cockpit for ${link("Claude Code")}. Talk to your machine like JARVIS: ask what you were working on, hear the answer out loud, and dispatch real work into your existing Claude Code sessions — by voice or text, across every project on your disk.`)}
    ${p(`Hark never calls the Anthropic API directly. It drives the ${code("claude")} CLI you already have installed and authenticated, so all usage draws from your existing subscription. No API key, no separate billing, no telemetry.`)}
    <blockquote style="border-left: 2px solid ${T.border}; margin: 8px 0; padding-left: 10px; color: ${T.dim}; line-height: 1.55">Status: a working desktop app in active development (macOS, Apple Silicon and Intel; Linux is on the backlog). Expect sharp edges; the voice loop, windows, boards, cost ledger and the persistent assistant chat all work today.</blockquote>
    <h2 style="color: ${T.strong}; margin: 12px 0 4px; line-height: 1.3; font-size: 15px; font-weight: 600">Install (from a release)</h2>
    ${p(`Binaries ship from the PUBLIC releases-only repo (${link("harkharness/hark")}) — this source repo stays private and is never exposed through them. Anyone can install with:`)}
    ${pre("curl -fsSL https://harkharness.web.app/install.sh | bash")}
    ${p(`That downloads the latest public release for your architecture, installs the ${code("hark")} CLI into ${code("~/.local/bin")}, drops ${code("hark.app")} into ${code("/Applications")} and clears the quarantine bit (the app is not code-signed yet).`)}
    ${p(`After installing: ${code("hark setup")} downloads the whisper speech model (~466MB), and the first mic use asks for microphone permission.`)}
    ${p(`Releases are cut with one command, never by hand:`)}
    ${pre("scripts/release.sh 0.2.4")}
    ${p(`It refuses a dirty tree, a branch that is not ${code("main")}, or a ${code("HEAD")} that disagrees with ${code("origin/main")}; then it bumps ${code("Cargo.toml")} and ${code("tauri.conf.json")}, commits, tags and pushes together.`)}
  </div>`;
}

/** No tab open: the right side says what to do; the hint follows the tree. */
function emptyState({ treeOpen }) {
  const hint = treeOpen
    ? "clique num arquivo da árvore, ou num caminho no chat"
    : "clique num caminho no chat — ou reabra a árvore no ícone da esquerda";
  return `<div style="flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 6px; padding: 24px; text-align: center">
    <span style="color: ${T.dim}; opacity: .7; margin-bottom: 6px">${icon("folderOpen", 22, "currentColor", 1.6)}</span>
    <div style="color: ${T.text}; font-size: 12.5px">os arquivos abertos aparecem aqui</div>
    <div style="color: ${T.dim}; font-size: 11.5px; max-width: 260px; line-height: 1.45">${hint}</div>
  </div>`;
}

function viewer(headOpts, bodyHtml) {
  // .viewer: column, --panel, hairline on the left (it returns now that the
  // tree sits beside it; .filetab-body .viewer used to remove it).
  return `<div style="flex: 1; min-width: 0; display: flex; flex-direction: column; background: ${T.panel}; border-left: 1px solid ${T.border}">
    ${headOpts ? viewerHead(headOpts) : ""}
    ${bodyHtml}
  </div>`;
}

const TABS = (active = "README.md") => [
  { name: "CLAUDE.md", abs: "~/Projects/vox/CLAUDE.md", on: active === "CLAUDE.md" },
  { name: "README.md", abs: "~/Projects/vox/README.md", on: active === "README.md" },
];

/** .frame: --panel, hairline border, radius 12, on the window's --bg. */
function frame({ treeOpen, tabs, left, right, rail = true, width, height, margin = 10 }) {
  return `<div style="width: ${width}px; height: ${height}px; background: ${T.bg}; box-sizing: border-box; padding: ${margin}px; font-family: ${T.ui}; font-size: 13px; color: ${T.text}">
    <div style="height: 100%; display: flex; flex-direction: column; background: ${T.panel}; border: 1px solid ${T.border}; border-radius: 12px; overflow: hidden; box-sizing: border-box">
      ${frameHead({ treeOpen, tabs, rail })}
      <div style="flex: 1; min-height: 0; display: flex">
        ${treeOpen ? left : ""}
        ${right}
      </div>
    </div>
  </div>`;
}

function artboard(bodyHtml) {
  return `<!doctype html>
<html>
<head>
  <meta charset="utf-8">
  <script src="./support.js"></script>
</head>
<body>
<x-dc>
<helmet>
  <style>
    body { margin: 0; background: ${T.bg}; color: ${T.text}; font-family: ${T.ui}; font-size: 13px; }
    a { color: ${T.accent}; } a:hover { color: ${T.ok}; }
  </style>
</helmet>
${bodyHtml}
</x-dc>
</body>
</html>
`;
}

const W = 880;
const H = 680;

const boards = {
  // 1 · fonte: tree open, README in source (= today's edit mode: save + eye).
  "Main.dc.html": frame({
    treeOpen: true,
    tabs: TABS(),
    left: treeColumn(),
    right: viewer({ mode: "edit" }, sourceView(README_SRC)),
    width: W,
    height: H,
  }),
  // 2 · renderizado: markdown opens rendered, the pencil unlocks the source.
  "Renderizado.dc.html": frame({
    treeOpen: true,
    tabs: TABS(),
    left: treeColumn(),
    right: viewer({ mode: "read" }, renderedReadme()),
    width: W,
    height: H,
  }),
  // 3 · árvore recolhida: the viewer takes the whole body.
  "Recolhida.dc.html": frame({
    treeOpen: false,
    tabs: TABS(),
    left: "",
    right: viewer({ mode: "read" }, renderedReadme()),
    width: W,
    height: H,
  }),
  // 4 · recolhida + fonte com a linha alvo (README.md:21 clicado no chat).
  "LinhaAlvo.dc.html": frame({
    treeOpen: false,
    tabs: TABS(),
    left: "",
    right: viewer({ mode: "edit" }, sourceView(README_SRC, { targetLine: 21 })),
    width: W,
    height: H,
  }),
  // 5 · vazio, árvore aberta (pasta clicada na sidebar do app).
  "Vazio.dc.html": frame({
    treeOpen: true,
    tabs: [],
    left: treeColumn({ activePath: "" }),
    right: viewer(null, emptyState({ treeOpen: true })),
    width: W,
    height: H,
  }),
  // 6 · vazio, árvore recolhida.
  "VazioRecolhido.dc.html": frame({
    treeOpen: false,
    tabs: [],
    left: "",
    right: viewer(null, emptyState({ treeOpen: false })),
    width: W,
    height: H,
  }),
  // 7 · filtro: the hits replace the trees, the pick opens on the right.
  "Filtro.dc.html": frame({
    treeOpen: true,
    tabs: [{ name: "install.sh", abs: "~/Projects/vox/install.sh", on: true }],
    left: hitsColumn({
      query: "install.s",
      hits: [
        { name: "install.sh", dir: "vox/" },
        { name: "initialize.claude-agent-acp.json", dir: "vox/crates/hark-plugin-acp/fixtures/" },
      ],
    }),
    right: viewer({ rel: "install.sh", mode: "edit" }, sourceView(INSTALL_SRC, { lang: "sh" })),
    width: W,
    height: H,
  }),
  // 8 · na rail: the real proportion (42% beside the chat) — tree born collapsed.
  "NaRail.dc.html": railContext(),
};

/** The project window as it is (title bar, task sidebar, chat, rail) with the
 * unified Arquivos frame in the rail and the terminal collapsed under it. */
function railContext() {
  const winW = 1280;
  const winH = 760;
  const sideW = 236;
  const railW = 438; // defaultSize 42 of the work area

  const sideRow = (label, active, icn) =>
    `<div style="display: flex; align-items: center; gap: 7px; padding: 6px 8px; border-radius: 6px; color: ${active ? T.text : T.dim}; font-size: 12.5px; background: ${active ? T.panel : "transparent"}"><span style="color: ${icn}">◍</span><span style="flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap">${label}</span></div>`;

  const sidebar = `<div style="width: ${sideW}px; flex: 0 0 auto; background: ${T.bgDeep}; border-right: 1px solid ${T.border}; box-shadow: inset -12px 0 16px -14px rgba(0,0,0,.9); padding: 10px 8px; display: flex; flex-direction: column; gap: 6px; box-sizing: border-box">
    <div style="display: flex; align-items: center; gap: 8px; border: 1px solid ${T.border}; border-radius: 8px; padding: 7px 10px; color: ${T.text}; font-size: 12.5px">${icon("kanban", 13, "currentColor", 1.8)} Board</div>
    <div style="display: flex; align-items: center; gap: 8px; border: 1px solid ${T.border}; border-radius: 8px; padding: 7px 10px; color: ${T.text}; font-size: 12.5px">${icon("message", 13, "currentColor", 1.8)} Chat do hark</div>
    <div style="margin-top: 10px; display: flex; align-items: center; justify-content: space-between; color: ${T.dim}; font-size: 10.5px; letter-spacing: 1px; padding: 0 4px"><span>VOX</span><span style="display: flex; gap: 8px; align-items: center">${icon("plus", 12)}<span title="arquivos do projeto" style="color: ${T.accent}; display: inline-flex">${icon("folderTree", 12)}</span></span></div>
    ${sideRow("Arquivos numa janela só", true, T.ok)}
    ${sideRow("Historiador das sessões ACP", false, T.warn)}
    ${sideRow("Catálogo com versão publicada", false, T.dim)}
    <div style="margin-top: 10px; display: flex; align-items: center; justify-content: space-between; color: ${T.dim}; font-size: 10.5px; letter-spacing: 1px; padding: 0 4px"><span>WORKSPACE-FABRICA</span><span style="display: flex; gap: 8px; align-items: center">${icon("plus", 12)}${icon("folderTree", 12)}</span></div>
    ${sideRow("Alertas do cluster migra…", false, T.dim)}
  </div>`;

  const chat = `<div style="flex: 1; min-width: 0; display: flex; flex-direction: column; background: ${T.bg}">
    <div style="flex: 1; padding: 18px 22px; display: flex; flex-direction: column; gap: 14px; min-height: 0">
      <div style="align-self: center; color: ${T.dim}; font-size: 12px">contexto de "Arquivos numa janela só" carregado; mensagens continuam esta task</div>
      <div style="align-self: flex-end; max-width: 78%; display: flex; flex-direction: column; gap: 3px">
        <span style="color: ${T.dim}; font-size: 11px; text-align: right">você → Arquivos numa</span>
        <div style="background: #1a2230; border-radius: 14px 14px 4px 14px; padding: 9px 13px">onde o README explica de onde vêm os binários?</div>
      </div>
      <div style="display: flex; flex-direction: column; gap: 6px">
        <span style="color: ${T.dim}; font-size: 11px">claude → Arquivos numa</span>
        <div style="line-height: 1.55">Na seção de instalação: <span style="font-family: ${T.mono}; background: rgba(122,162,247,.12); color: ${T.accent}; border-radius: 4px; padding: 1px 5px; font-size: 12px; border-bottom: 1px dotted rgba(122,162,247,.55)">README.md:21</span> — o repo público só recebe binários; a fonte nunca sai daqui.</div>
        <span style="color: ${T.dim}; font-size: 11px; font-family: ${T.mono}">sonnet-5 · $0.0121</span>
      </div>
    </div>
    <div style="padding: 0 22px 16px; display: flex; flex-direction: column; gap: 9px">
      <div style="border: 1px solid ${T.borderStrong}; border-radius: 12px; padding: 13px 15px; color: ${T.dim}"><span style="color: ${T.accent}">→</span> Arquivos numa janela só (Enter manda pra esta task · "hark, …" fala com o hark)</div>
      <div style="display: flex; align-items: center; gap: 10px">
        <div style="border: 1px solid ${T.borderStrong}; border-radius: 8px; padding: 6px 12px; color: ${T.text}; font-size: 12.5px">Manual ⌃</div>
        <div style="flex: 1"></div>
        <div style="display: flex; align-items: center; gap: 14px; color: ${T.dim}">${icon("terminal", 16, "currentColor", 1.9)}${icon("mic", 17, "currentColor", 1.7)}${icon("send", 18, T.accent, 1.8)}</div>
      </div>
    </div>
  </div>`;

  // The rail: Arquivos expanded (tree collapsed at this width), Terminal shrunk to its bar.
  const filesFrame = `<div style="flex: 1; min-height: 0; display: flex; flex-direction: column; background: ${T.panel}; border: 1px solid ${T.border}; border-radius: 12px; overflow: hidden; margin: 0 0 4px 4px">
    ${frameHead({ treeOpen: false, tabs: TABS() })}
    <div style="flex: 1; min-height: 0; display: flex">${viewer({ mode: "read" }, renderedReadme())}</div>
  </div>`;
  const termBar = `<div style="flex: 0 0 auto; display: flex; flex-direction: column; background: ${T.panel}; border: 1px solid ${T.border}; border-radius: 12px; overflow: hidden; margin: 0 0 4px 4px">
    <div style="display: flex; align-items: center; gap: 8px; padding: 6px 8px 6px 12px"><span style="color: ${T.dim}; font-size: 12.5px; font-weight: 600">Terminal</span><span style="margin-left: auto; display: flex; gap: 2px">${ctl("chevronDown", "expandir painel").replace("chevronDown", "chevronDown")}${ctl("x", "fechar")}</span></div>
  </div>`;
  const rail = `<div style="width: ${railW}px; flex: 0 0 auto; display: flex; flex-direction: column; height: 100%; padding: 4px 4px 0 0; box-sizing: border-box">${filesFrame}${termBar}</div>`;

  return `<div style="width: ${winW}px; height: ${winH}px; display: flex; flex-direction: column; background: ${T.bgDeep}; overflow: hidden; font-family: ${T.ui}; font-size: 13px; color: ${T.text}">
    <div style="display: flex; align-items: center; gap: 8px; padding: 10px 14px; background: ${T.bg}; flex: 0 0 auto">
      <span style="width: 11px; height: 11px; border-radius: 50%; background: ${T.err}"></span>
      <span style="width: 11px; height: 11px; border-radius: 50%; background: ${T.warn}"></span>
      <span style="width: 11px; height: 11px; border-radius: 50%; background: ${T.ok}"></span>
      <span style="margin-left: 8px; color: ${T.dim}; display: inline-flex">${icon("panelLeft", 14)}</span>
      <span style="margin-left: 6px; color: ${T.dim}; font-size: 12px">Hark — vox</span>
    </div>
    <div style="flex: 1; display: flex; min-height: 0">
      ${sidebar}
      ${chat}
      ${rail}
    </div>
  </div>`;
}

for (const [name, html] of Object.entries(boards)) {
  writeFileSync(new URL(`./${name}`, import.meta.url), artboard(html));
}
console.log(`wrote ${Object.keys(boards).length} artboards`);
