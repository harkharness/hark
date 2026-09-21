// Generates the artboards of the "Plugins e Configurações" canvas.
// Every token below is lifted from src/styles.css; the mockups draw the
// app's own components, not an approximation of them. Run: node build.mjs
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
  ui: "-apple-system, system-ui, sans-serif",
  mono: "ui-monospace, Menlo, monospace",
};

const esc = (s) => String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
const svg = (body, size, color, extra = "") =>
  `<svg width="${size}" height="${size}" viewBox="0 0 24 24" fill="none" stroke="${color}" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" style="flex:0 0 auto;${extra}">${body}</svg>`;

const ICON = {
  sliders: `<line x1="4" y1="21" x2="4" y2="14"/><line x1="4" y1="10" x2="4" y2="3"/><line x1="12" y1="21" x2="12" y2="12"/><line x1="12" y1="8" x2="12" y2="3"/><line x1="20" y1="21" x2="20" y2="16"/><line x1="20" y1="12" x2="20" y2="3"/><line x1="1" y1="14" x2="7" y2="14"/><line x1="9" y1="8" x2="15" y2="8"/><line x1="17" y1="16" x2="23" y2="16"/>`,
  plug: `<path d="M12 22v-5"/><path d="M9 8V2"/><path d="M15 8V2"/><path d="M18 8v5a4 4 0 0 1-4 4h-4a4 4 0 0 1-4-4V8z"/>`,
  mic: `<path d="M12 2a3 3 0 0 0-3 3v7a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3Z"/><path d="M19 10v2a7 7 0 0 1-14 0v-2"/><line x1="12" y1="19" x2="12" y2="22"/>`,
  bot: `<rect x="3" y="11" width="18" height="10" rx="2"/><circle cx="12" cy="5" r="2"/><path d="M12 7v4"/><line x1="8" y1="16" x2="8" y2="16"/><line x1="16" y1="16" x2="16" y2="16"/>`,
  wrench: `<path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z"/>`,
  fileCode: `<path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><path d="M14 2v6h6"/><path d="m10 13-2 2 2 2"/><path d="m14 17 2-2-2-2"/>`,
  check: `<path d="M20 6 9 17l-5-5"/>`,
  minus: `<path d="M5 12h14"/>`,
  plus: `<path d="M12 5v14"/><path d="M5 12h14"/>`,
  refresh: `<path d="M21 12a9 9 0 1 1-2.6-6.4"/><path d="M21 3v6h-6"/>`,
  key: `<circle cx="7.5" cy="15.5" r="4.5"/><path d="m10.7 12.3 8.3-8.3"/><path d="m17 6 2 2"/><path d="m14 9 2 2"/>`,
  arrowUp: `<circle cx="12" cy="12" r="9"/><path d="m8 12 4-4 4 4"/><path d="M12 16V8"/>`,
  chevron: `<path d="m9 18 6-6-6-6"/>`,
  copy: `<rect x="9" y="9" width="12" height="12" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>`,
  gear: `<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.6a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9c.14.35.38.65.7.86.32.2.69.32 1.07.33H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/>`,
};

// ---------------------------------------------------------------------
// Vendor marks. Drawn HERE, by us: a hue and a geometric glyph per
// vendor, never a bundled brand asset. An id we do not know falls back
// to a monogram over a hue hashed from the id, so a twin the user wrote
// in config.toml is as recognizable as a built-in.
// ---------------------------------------------------------------------
const VENDOR = {
  anthropic: { hue: "#c96442", glyph: (c) => `<path d="M7.2 17 11 6.6h2L16.8 17" stroke="${c}" stroke-width="2.1" fill="none" stroke-linecap="round"/><path d="M9.3 13.6h5.4" stroke="${c}" stroke-width="2.1" stroke-linecap="round"/>` },
  google:    { hue: "#5b8dee", glyph: (c) => `<path d="M12 4.5c.9 4 2.6 5.7 6.6 6.6-4 .9-5.7 2.6-6.6 6.6-.9-4-2.6-5.7-6.6-6.6 4-.9 5.7-2.6 6.6-6.6Z" fill="${c}" stroke="none"/>` },
  openai:    { hue: "#4fb79a", glyph: (c) => `<circle cx="12" cy="12" r="6.4" stroke="${c}" stroke-width="1.9" fill="none"/><path d="M12 5.6v12.8M6.5 8.8l11 6.4M6.5 15.2l11-6.4" stroke="${c}" stroke-width="1.5"/>` },
  acp:       { hue: "#7aa2f7", glyph: (c) => `<path d="M12 19v-4M9.5 8.5V4M14.5 8.5V4M7.5 8.5h9v4a4.5 4.5 0 0 1-9 0z" stroke="${c}" stroke-width="1.8" fill="none" stroke-linecap="round" stroke-linejoin="round"/>` },
};
const HUES = ["#7aa2f7", "#bb9af7", "#7dcfff", "#9ece6a", "#e0af68", "#f7768e"];
const hueFor = (id) => HUES[[...id].reduce((a, c) => a + c.charCodeAt(0), 0) % HUES.length];

/** state: "on" | "idle" | "off" | "missing" */
function mark(agent, { size = 34, state = "idle" } = {}) {
  const v = VENDOR[agent.vendor];
  const hue = v ? v.hue : hueFor(agent.id);
  const ring =
    state === "on" ? T.ok : state === "missing" ? T.warn : state === "off" ? "transparent" : T.border;
  const inner = v
    ? `<svg width="${Math.round(size * 0.62)}" height="${Math.round(size * 0.62)}" viewBox="0 0 24 24">${v.glyph(hue)}</svg>`
    : `<span style="font:600 ${Math.round(size * 0.4)}px/1 ${T.ui};color:${hue}">${esc(agent.id[0].toUpperCase())}</span>`;
  return `<span style="flex:0 0 auto;width:${size}px;height:${size}px;border-radius:${Math.round(size * 0.29)}px;
    background:color-mix(in srgb, ${hue} 14%, ${T.bgDeep});
    border:${state === "off" ? "1px dashed " + T.border : "1.5px solid " + ring};
    display:flex;align-items:center;justify-content:center;opacity:${state === "off" ? 0.5 : 1}">${inner}</span>`;
}

// --------------------------------- primitives
const row = (gap = 8, extra = "") =>
  `display:flex;align-items:center;gap:${gap}px;${extra}`;
const monoTxt = (s, c = T.dim, size = 11) =>
  `<span style="font:400 ${size}px/1.4 ${T.mono};color:${c}">${esc(s)}</span>`;
const dimTxt = (s, size = 11.5) =>
  `<span style="font:400 ${size}px/1.5 ${T.ui};color:${T.dim}">${esc(s)}</span>`;

function pill(text, tone = "dim") {
  const c = { dim: T.dim, ok: T.ok, accent: T.accent, warn: T.warn }[tone];
  const b = tone === "dim" ? T.border : `color-mix(in srgb, ${c} 45%, transparent)`;
  return `<span style="${row(4)};border:1px solid ${b};border-radius:999px;padding:2px 9px;
    font:400 10.5px/1.4 ${T.ui};color:${c};flex:0 0 auto">${text}</span>`;
}

function btn(label, { tone = "plain", icon = "" } = {}) {
  const c = tone === "accent" ? T.accent : T.text;
  const bg = tone === "accent" ? `color-mix(in srgb, ${T.accent} 16%, transparent)` : T.panel2;
  return `<span style="${row(6)};background:${bg};border-radius:8px;padding:6px 12px;
    font:500 12px/1 ${T.ui};color:${c}">${icon ? svg(ICON[icon], 12, c) : ""}${esc(label)}</span>`;
}

// --------------------------------- agents (invented catalog)
const AGENTS = {
  claude: {
    id: "claude", name: "Claude Code", vendor: "anthropic", cmd: "claude", version: "2.1.236",
    bin: "~/.local/bin/claude", standard: "sonnet", state: "on",
    models: [["light", "haiku"], ["standard", "sonnet"], ["heavy", "opus"], ["max", "fable"]],
    caps: { permissions: 1, cost: 1, history: 1, resume: 1, slash: 1, live: 1 },
    env: [], args: [], login: "claude /login",
  },
  gw: {
    id: "claude-gw", name: "Claude via gateway", vendor: "anthropic", cmd: "claude", version: "2.1.236",
    bin: "~/.local/bin/claude", standard: "claude-sonnet-5", state: "idle",
    models: [["light", "claude-haiku-4-5"], ["standard", "claude-sonnet-5"], ["heavy", "claude-sonnet-5"], ["max", "auto-routing-plan"]],
    caps: { permissions: 1, cost: 1, history: 1, resume: 1, slash: 1, live: 1 },
    env: ["BASE_URL", "AUTH_TOKEN"], args: [], login: "chave do gateway em env",
  },
  gemini: {
    id: "gemini", name: "Gemini CLI", vendor: "google", cmd: "gemini", version: "0.59.0",
    bin: "~/.nvm/versions/node/v26.7.0/bin/gemini", standard: "gemini-2.5-flash", state: "idle",
    behind: "0.60.0", update: "npm install -g @google/gemini-cli@0.60.0",
    models: [["light", "gemini-2.5-flash-lite"], ["standard", "gemini-2.5-flash"], ["heavy", "gemini-2.5-pro"], ["max", "gemini-2.5-pro"]],
    caps: { permissions: 1, cost: 0, history: 0, resume: 1, slash: 1, live: 0 },
    env: [], args: ["--experimental-acp"], login: "gemini",
  },
  codex: {
    id: "codex", name: "Codex", vendor: "openai", cmd: "codex-acp", version: "0.12.1",
    bin: "~/.local/bin/codex-acp", standard: "gpt-5-codex", state: "off",
    models: [["standard", "gpt-5-codex"], ["heavy", "gpt-5-codex"]],
    caps: { permissions: 1, cost: 0, history: 0, resume: 0, slash: 0, live: 0 },
    env: [], args: ["--acp"], login: "codex login",
  },
  local: {
    id: "qwen-local", name: "Qwen local", vendor: null, cmd: "qwen-acp", version: null,
    bin: null, standard: "qwen3-coder", state: "missing",
    install: "npm install -g qwen-acp",
    models: [["standard", "qwen3-coder"]],
    caps: null, env: [], args: ["--acp"], login: null,
  },
};

const CAP_LABEL = {
  permissions: "permissões", cost: "custo por turno", history: "histórico indexado",
  resume: "retomar sessão", slash: "slash commands", live: "sessões vivas",
};

// ---------------------------------------------------------------------
// The card. Identity and ONE line of meta — enough to choose, never
// enough to read. Everything else moved into the detail panel.
// Clicking a card INSPECTS it; only "usar este" changes the driver.
// ---------------------------------------------------------------------
function card(a, { selected = false, width = 0 } = {}) {
  const st = a.state;
  const bd = selected
    ? `1px solid color-mix(in srgb, ${T.accent} 55%, transparent)`
    : `1px solid ${T.border}`;
  const bg = selected ? T.panel2 : T.bg;
  const meta =
    st === "missing"
      ? `<span style="font:400 11px/1.4 ${T.ui};color:${T.warn}">não encontrado</span>`
      : a.behind
        ? `<span style="${row(4)};font:400 11px/1.4 ${T.ui};color:${T.warn}">${svg(ICON.arrowUp, 11, T.warn)}atualização ${esc(a.behind)}</span>`
        : st === "off"
          ? `<span style="font:400 11px/1.4 ${T.ui};color:${T.dim}">desligado · ligar</span>`
          : dimTxt(`${a.standard} · ${a.caps ? Object.values(a.caps).filter(Boolean).length : 0} capacidades`);
  const badge =
    st === "on"
      ? `<span style="${row(3)};color:${T.ok};font:500 10.5px/1 ${T.ui}">${svg(ICON.check, 11, T.ok)}em uso</span>`
      : "";
  return `<div style="${width ? `width:${width}px;` : ""}border:${bd};border-radius:12px;background:${bg};
    padding:11px 12px;display:flex;flex-direction:column;gap:9px;min-width:0">
    <div style="${row(9)}">
      ${mark(a, { size: 34, state: st })}
      <div style="min-width:0;flex:1">
        <div style="${row(6, "justify-content:space-between")}">
          <span style="font:600 13px/1.3 ${T.ui};color:${st === "off" ? T.dim : T.strong};
            white-space:nowrap;overflow:hidden;text-overflow:ellipsis">${esc(a.name)}</span>
          ${badge}
        </div>
        ${monoTxt(a.version ? `${a.id} · v${a.version}` : a.id, T.dim, 10.5)}
      </div>
    </div>
    <div style="${row(6, "min-height:16px")}">${meta}</div>
  </div>`;
}

function addCard({ width = 0 } = {}) {
  return `<div style="${width ? `width:${width}px;` : ""}border:1px dashed ${T.borderStrong};border-radius:12px;
    padding:11px 12px;display:flex;align-items:center;gap:9px;color:${T.dim};min-height:70px">
    <span style="flex:0 0 auto;width:34px;height:34px;border-radius:10px;border:1px dashed ${T.borderStrong};
      display:flex;align-items:center;justify-content:center">${svg(ICON.plus, 15, T.dim)}</span>
    <div><div style="font:600 13px/1.3 ${T.ui};color:${T.text}">novo agente</div>
    ${dimTxt("uma entrada em config.toml", 10.5)}</div>
  </div>`;
}

// --------------------------------- the detail panel
function detailRow(label, value, { top = false } = {}) {
  return `<div style="display:flex;gap:12px;padding:7px 0;border-top:${top ? `1px solid ${T.border}` : "none"}">
    <div style="flex:0 0 78px;font:400 11px/1.5 ${T.ui};color:${T.dim};min-width:0">${String(label).startsWith("<") ? label : esc(label)}</div>
    <div style="flex:1;min-width:0">${value}</div>
  </div>`;
}

function section(title) {
  return `<div style="font:600 10.5px/1 ${T.ui};color:${T.dim};letter-spacing:.06em;
    text-transform:uppercase;margin:14px 0 4px">${esc(title)}</div>`;
}

function capsGrid(caps) {
  if (!caps)
    return dimTxt("o Hark ainda não negociou capacidades com este agente");
  return `<div style="display:grid;grid-template-columns:1fr 1fr;gap:4px 12px">
    ${Object.entries(CAP_LABEL)
      .map(([k, label]) => {
        const has = caps[k];
        const c = has ? T.text : T.dim;
        return `<span style="${row(6)};font:400 11.5px/1.5 ${T.ui};color:${c};opacity:${has ? 1 : 0.55}">
          ${svg(has ? ICON.check : ICON.minus, 12, has ? T.ok : T.dim)}${esc(label)}</span>`;
      })
      .join("")}
  </div>`;
}

function modelTable(models) {
  return `<div style="display:flex;flex-direction:column;gap:3px">
    ${models
      .map(
        ([tier, id]) =>
          `<div style="${row(10)};font-size:11.5px">
            <span style="flex:0 0 62px;font:400 11px/1.5 ${T.ui};color:${T.dim}">${esc(tier)}</span>
            ${monoTxt(id, T.text, 11.5)}
          </div>`,
      )
      .join("")}
  </div>`;
}

function codeBlock(text, tone = T.text) {
  return `<div style="${row(8, "justify-content:space-between")};background:${T.bgDeep};
    border:1px solid ${T.border};border-radius:8px;padding:7px 10px">
    ${monoTxt(text, tone, 11)}${svg(ICON.copy, 12, T.dim)}</div>`;
}

function detail(a, { width = 380, fill = false } = {}) {
  const st = a.state;
  const action =
    st === "on"
      ? `<span style="${row(5)};color:${T.ok};font:500 12px/1 ${T.ui}">${svg(ICON.check, 13, T.ok)}em uso</span>`
      : st === "missing"
        ? pill("instale para usar", "warn")
        : st === "off"
          ? btn("ligar")
          : btn("usar este", { tone: "accent" });
  const box = fill ? `flex:1 1 auto;min-width:0` : `width:${width}px;flex:0 0 ${width}px`;
  return `<aside style="${box};border:1px solid ${T.border};border-radius:14px;
    background:${T.panel};padding:14px 16px;display:flex;flex-direction:column;overflow:hidden">
    <div style="${row(11)}">
      ${mark(a, { size: 42, state: st })}
      <div style="flex:1;min-width:0">
        <div style="font:600 14.5px/1.3 ${T.ui};color:${T.strong}">${esc(a.name)}</div>
        ${monoTxt(a.version ? `${a.cmd} · v${a.version}` : a.cmd, T.dim, 11)}
      </div>
      ${action}
    </div>

    ${section("identidade")}
    ${detailRow("tabela", monoTxt(`[agents.${a.id}]`, T.accent, 11))}
    ${detailRow(
      "binário",
      a.bin ? monoTxt(a.bin, T.text, 11) : `<span style="font:400 11.5px/1.5 ${T.ui};color:${T.warn}">não está no PATH</span>`,
      { top: true },
    )}
    ${a.args.length ? detailRow("comando", monoTxt(`${a.cmd} ${a.args.join(" ")}`, T.text, 11), { top: true }) : ""}
    ${a.install ? `<div style="margin-top:8px">${codeBlock(a.install, T.text)}</div>` : ""}
    ${a.behind ? `<div style="margin-top:8px;display:flex;flex-direction:column;gap:6px">
      <span style="${row(5)};font:400 11.5px/1.5 ${T.ui};color:${T.warn}">${svg(ICON.arrowUp, 12, T.warn)}instalado ${esc(a.version)} · publicado ${esc(a.behind)}</span>
      ${codeBlock(a.update)}</div>` : ""}

    ${section("modelos por tier")}
    ${modelTable(a.models)}

    ${a.env.length ? section("ambiente") + a.env
      .map((k, i) =>
        detailRow(monoTxt(k, T.dim, 11), `<span style="${row(8)}">${monoTxt("•••••••••", T.dim, 11)}${dimTxt("no arquivo", 10.5)}</span>`, { top: i > 0 }),
      )
      .join("") : ""}

    ${section("capacidades")}
    ${capsGrid(a.caps)}

    ${a.login ? section("login") + `<div style="${row(6)};margin-top:2px">${svg(ICON.key, 12, T.dim)}${monoTxt(a.login, T.text, 11)}</div>` : ""}

    <div style="margin-top:auto;padding-top:14px">
      <span style="${row(5)};font:400 11.5px/1 ${T.ui};color:${T.accent}">
        ${svg(ICON.fileCode, 12, T.accent)}editar [agents.${esc(a.id)}] no config.toml${svg(ICON.chevron, 11, T.accent)}</span>
    </div>
  </aside>`;
}

// ---------------------------------------------------------------------
// The settings shell. Six flat items become three named groups, so
// "Workers" stops sitting between "Voz" and "Avançado" by accident.
// ---------------------------------------------------------------------
const NAV = [
  ["você", [["Geral", "sliders"], ["Voz", "mic"]]],
  ["agentes", [["Plugins", "plug"], ["Workers", "bot"]]],
  ["máquina", [["Avançado", "wrench"], ["config.toml", "fileCode"]]],
];

function nav(active, { width = 200, grouped = true } = {}) {
  const item = ([name, icon]) => {
    const on = name === active;
    const c = on ? T.accent : T.text;
    return `<div style="${row(9)};padding:7px 10px;border-radius:7px;font:400 12.5px/1 ${T.ui};color:${c};
      background:${on ? "rgba(122,162,247,.12)" : "transparent"}">${svg(ICON[icon], 14, c)}${esc(name)}</div>`;
  };
  const body = grouped
    ? NAV.map(
        ([group, items]) =>
          `<div style="font:500 10px/1 ${T.ui};color:${T.dim};letter-spacing:.07em;text-transform:uppercase;
            margin:12px 10px 5px">${esc(group)}</div>${items.map(item).join("")}`,
      ).join("")
    : NAV.flatMap(([, items]) => items).map(item).join("");
  return `<nav style="flex:0 0 ${width}px;background:${T.bgDeep};border-right:1px solid ${T.border};
    padding:14px 10px;display:flex;flex-direction:column;gap:2px">
    ${grouped ? "" : `<div style="font:500 11px/1 ${T.ui};color:${T.dim};margin:0 8px 10px">Configurações</div>`}
    ${body}
    <div style="margin-top:auto;padding:8px;font:400 10px/1.4 ${T.mono};color:${T.dim};
      overflow:hidden;text-overflow:ellipsis;white-space:nowrap">~/.hark/config.toml</div>
  </nav>`;
}

function sectionHead(title, { sub = "", right = "" } = {}) {
  return `<div style="display:flex;align-items:flex-start;justify-content:space-between;gap:16px;margin-bottom:12px">
    <div style="min-width:0">
      <div style="font:600 15px/1.3 ${T.ui};color:${T.strong}">${esc(title)}</div>
      ${sub ? `<div style="font:400 12px/1.5 ${T.ui};color:${T.dim};margin-top:3px;max-width:560px">${esc(sub)}</div>` : ""}
    </div>
    ${right}
  </div>`;
}

const registryLine = `<span style="${row(8)};flex:0 0 auto">
  ${dimTxt("catálogo lido às 14:02", 11)}
  <span style="${row(5)};background:${T.panel2};border-radius:8px;padding:5px 10px;font:400 11.5px/1 ${T.ui};color:${T.text}">
  ${svg(ICON.refresh, 11, T.dim)}rechecar</span></span>`;

function grid(cards, { cols = 3, gap = 10 } = {}) {
  return `<div style="display:grid;grid-template-columns:repeat(${cols},minmax(0,1fr));gap:${gap}px;align-content:start">
    ${cards.join("")}</div>`;
}

/** The mother window's tab strip, with Ajustes as a tab. */
function motherTabs(active, { gear = false } = {}) {
  const tab = (name) => {
    const on = name === active;
    return `<div style="padding:5px 13px;border-radius:8px;font:500 12px/1 ${T.ui};
      color:${on ? T.strong : T.dim};background:${on ? T.panel2 : "transparent"}">${esc(name)}</div>`;
  };
  return `<div style="display:flex;justify-content:center;padding:9px 0">
    <div style="${row(2)};background:${T.panel};border:1px solid ${T.border};border-radius:11px;padding:3px">
      ${tab("voz")}${tab("board")}${tab("custos")}${gear ? `<span style="padding:5px 11px;display:flex">${svg(ICON.gear, 14, T.dim)}</span>` : tab("ajustes")}
    </div>
  </div>`;
}

function shell({ tabs = "ajustes", body, width, height }) {
  return `<div style="width:${width}px;height:${height}px;background:${T.bg};display:flex;flex-direction:column;
    overflow:hidden;font-family:${T.ui}">
    ${motherTabs(tabs)}
    <div style="flex:1;min-height:0;display:flex;margin:0 14px 14px;border:1px solid ${T.border};
      border-radius:14px;background:${T.panel};overflow:hidden">${body}</div>
  </div>`;
}

// --------------------------------- screens
function plugSection({ selected = "claude", cols = 3, detailWidth = 380, stacked = false }) {
  const list = [AGENTS.claude, AGENTS.gw, AGENTS.gemini, AGENTS.codex, AGENTS.local];
  const cards = list.map((a) => card(a, { selected: a.id === selected })).concat(addCard());
  const sel = list.find((a) => a.id === selected) ?? list[0];
  const head = sectionHead("Plugins", {
    sub: "O Hark fala com o agente por um contrato aberto. Escolha qual backend dirige as sessões novas.",
    right: registryLine,
  });
  if (stacked)
    return `<div style="flex:1;min-width:0;padding:18px 22px;display:flex;flex-direction:column;overflow:hidden">
      ${head}${grid(cards, { cols })}
      <div style="margin-top:12px;display:flex;min-height:0">${detail(sel, { fill: true })}</div></div>`;
  return `<div style="flex:1;min-width:0;padding:18px 22px;display:flex;gap:16px;overflow:hidden">
    <div style="flex:1;min-width:0;display:flex;flex-direction:column">${head}${grid(cards, { cols })}</div>
    ${detail(sel, { width: detailWidth })}</div>`;
}

function formRow(label, hint, control) {
  return `<div style="display:flex;align-items:center;justify-content:space-between;gap:18px;
    padding:10px 0;border-bottom:1px solid ${T.border}">
    <div style="min-width:0"><div style="font:600 12.5px/1.4 ${T.ui};color:${T.text}">${esc(label)}</div>
    ${dimTxt(hint)}</div>${control}</div>`;
}
const input = (v, w = 220) =>
  `<span style="flex:0 0 ${w}px;background:${T.panel2};border-radius:8px;padding:7px 10px;
    font:400 12.5px/1 ${T.ui};color:${T.text}">${esc(v)}</span>`;

// --------------------------------- today, for comparison
function todayCard(a, { on = false } = {}) {
  const pills = [
    ...a.models.map(([tier, id]) => pill(`${tier} · ${id}`)),
    ...(a.caps ? Object.entries(CAP_LABEL).filter(([k]) => a.caps[k]).map(([, l]) => pill(l)) : []),
  ];
  return `<div style="border:1px solid ${on ? `color-mix(in srgb, ${T.ok} 45%, transparent)` : T.border};
    border-radius:12px;background:${on ? `color-mix(in srgb, ${T.ok} 5%, ${T.bg})` : T.bg};
    padding:13px 15px;display:flex;flex-direction:column;gap:8px">
    <div style="${row(10, "justify-content:space-between")}">
      <span style="${row(9, "align-items:baseline")}">
        <span style="font:600 14px/1.3 ${T.ui};color:${T.strong}">${esc(a.name)}</span>
        ${monoTxt(`${a.cmd} v${a.version}`, T.dim, 11)}</span>
      ${on ? pill(`${svg(ICON.check, 11, T.ok)}em uso`, "ok") : pill("usar este", "accent")}
    </div>
    ${a.bin ? monoTxt(a.bin, T.dim, 11) : ""}
    ${a.behind ? `<div style="display:flex;flex-direction:column;gap:8px">
      <span style="${row(5)};font:400 11.5px/1.5 ${T.ui};color:${T.warn}">${svg(ICON.arrowUp, 12, T.warn)}instalado ${a.version} · publicado ${a.behind} no registro ACP — atualize:</span>
      ${codeBlock(a.update)}</div>` : ""}
    <div style="display:flex;flex-wrap:wrap;gap:5px">${pills.join("")}</div>
    ${a.args.length ? `<span style="${row(8)}">${monoTxt(`${a.cmd} ${a.args.join(" ")}`)}${svg(ICON.key, 11, T.dim)}${dimTxt(a.login, 11.5)}</span>` : ""}
    <span style="${row(5)};font:400 11.5px/1 ${T.ui};color:${T.accent}">${svg(ICON.fileCode, 12, T.accent)}config.toml ›</span>
  </div>`;
}

function today({ width = 1440, height = 820 }) {
  const modalW = 760, modalH = 560;
  return `<div style="width:${width}px;height:${height}px;background:${T.bg};position:relative;
    overflow:hidden;font-family:${T.ui}">
    ${motherTabs("voz", { gear: true })}
    <div style="position:absolute;inset:0;background:rgba(0,0,0,.6);display:flex;align-items:center;justify-content:center">
      <div style="width:${modalW}px;height:${modalH}px;background:${T.panel};border:1px solid ${T.borderStrong};
        border-radius:12px;display:flex;overflow:hidden">
        ${nav("Plugins", { width: 190, grouped: false })}
        <div style="flex:1;min-width:0;padding:18px 22px;overflow:hidden;display:flex;flex-direction:column;gap:10px">
          <span style="${row(7)};color:${T.dim};font:400 12.5px/1.5 ${T.ui}">${svg(ICON.plug, 14, T.dim)}O Hark fala com o agente por um contrato aberto. Escolha qual backend dirige as sessões.</span>
          ${todayCard(AGENTS.claude, { on: true })}
          ${todayCard(AGENTS.gemini)}
        </div>
      </div>
    </div>
    <div style="position:absolute;left:${(width - modalW) / 2 + modalW + 24}px;top:${(height - modalH) / 2}px;
      width:250px;font:400 12px/1.6 ${T.ui};color:${T.dim}">
      <b style="color:${T.warn}">hoje</b><br>760×560 fixos: sobra tela de todo lado e mesmo assim só cabem
      dois cards. Cada card empilha 10 pílulas do mesmo peso — tier, capacidade, env, login — e a única
      decisão da tela (quem dirige) tem o mesmo tamanho do caminho do binário.</div>
  </div>`;
}

// --------------------------------- artboards
const A = (title, inner) => `<!doctype html>
<html lang="pt-BR"><head><meta charset="utf-8"><title>${esc(title)}</title>
<style>*{box-sizing:border-box;margin:0;padding:0}body{background:${T.bg};color:${T.text}}</style>
</head><body>${inner}</body></html>`;

const boards = [];
const board = (file, title, w, h, inner, note) => {
  writeFileSync(new URL(file, import.meta.url), A(title, inner));
  boards.push({ file, title, w, h, note, inner });
};

board("Main.dc.html", "Ajustes · Plugins", 1440, 820,
  shell({ width: 1440, height: 820, body: nav("Plugins") + plugSection({ selected: "claude" }) }),
  "Ajustes vira a quarta aba da mãe, com a largura da janela. A grade responde ao espaço (3 colunas aqui, 5 numa tela grande) e o detalhe do agente selecionado fica ao lado. Clicar num card INSPECIONA; só o botão “usar este” troca quem dirige.");

board("Detalhe.dc.html", "O painel de detalhe", 1440, 820,
  shell({ width: 1440, height: 820, body: nav("Plugins") + plugSection({ selected: "gemini" }) }),
  "Gemini selecionado: o aviso de versão, os argumentos do ACP e as capacidades que ele NÃO tem (custo por turno, histórico, sessões vivas) aparecem como ausência marcada, não como pílula faltando.");

board("Gateway.dc.html", "Gêmeo de gateway", 1440, 820,
  shell({ width: 1440, height: 820, body: nav("Plugins") + plugSection({ selected: "claude-gw" }) }),
  "Dois agentes com o mesmo binário. O que os separa é o id (a tabela em config.toml), por isso o id é a segunda linha do card. As chaves de ambiente aparecem pelo NOME; o valor nunca sai do arquivo.");

board("Cards.dc.html", "Os cinco estados do card", 900, 300,
  `<div style="width:900px;height:300px;background:${T.bg};padding:22px;font-family:${T.ui}">
    ${grid([
      card(AGENTS.claude), card(AGENTS.gw, { selected: true }), card(AGENTS.gemini),
      card(AGENTS.codex), card(AGENTS.local), addCard(),
    ], { cols: 3 })}
    <div style="margin-top:16px;font:400 11.5px/1.6 ${T.ui};color:${T.dim}">
      em uso (anel verde) · selecionado para inspeção (borda azul) · desatualizado (âmbar) ·
      desligado (marca tracejada, 50%) · ausente do PATH · novo agente</div>
  </div>`,
  "O card carrega identidade e UMA linha de meta: o modelo do tier padrão e a contagem de capacidades. Todo o resto desceu para o detalhe.");

board("Marcas.dc.html", "As marcas", 900, 260,
  `<div style="width:900px;height:260px;background:${T.bg};padding:24px;font-family:${T.ui}">
    <div style="display:flex;gap:26px;align-items:flex-start">
      ${[["anthropic", AGENTS.claude], ["google", AGENTS.gemini], ["openai", AGENTS.codex],
         ["acp", { id: "acp", vendor: "acp" }], ["monograma", AGENTS.local], ["monograma", { id: "zeta", vendor: null }]]
        .map(([label, a]) => `<div style="display:flex;flex-direction:column;align-items:center;gap:8px">
          ${mark(a, { size: 46 })}<span style="font:400 11px/1.4 ${T.ui};color:${T.dim}">${esc(label)}</span></div>`).join("")}
    </div>
    <div style="display:flex;gap:26px;align-items:center;margin-top:24px">
      ${["on", "idle", "off", "missing"].map((s) => `<div style="display:flex;flex-direction:column;align-items:center;gap:8px">
        ${mark(AGENTS.claude, { size: 40, state: s })}
        <span style="font:400 11px/1.4 ${T.ui};color:${T.dim}">${{ on: "em uso", idle: "disponível", off: "desligado", missing: "ausente" }[s]}</span></div>`).join("")}
    </div>
    <div style="margin-top:20px;font:400 11.5px/1.6 ${T.ui};color:${T.dim};max-width:700px">
      Glifos desenhados por nós, com o matiz do fornecedor — nada de SVG de marca de terceiro no repo.
      Um id desconhecido cai no monograma sobre um matiz derivado do próprio id, então o gêmeo que você
      escreveu no config.toml é tão reconhecível quanto um built-in.</div>
  </div>`,
  "A marca é o que faz a grade ser varrida com o olho em vez de lida. O anel dela também carrega o estado, o que dispensa uma pílula por card.");

board("Estreito.dc.html", "Na largura do modal de hoje", 780, 880,
  `<div style="width:780px;height:880px;background:${T.bg};display:flex;font-family:${T.ui};
    border:1px solid ${T.border};border-radius:14px;overflow:hidden">
    ${nav("Plugins", { width: 180 })}${plugSection({ selected: "claude", cols: 2, stacked: true })}</div>`,
  "Mesma tela em 780px: duas colunas e o detalhe empilhado embaixo. A regra é uma só — detalhe ao lado acima de ~1040px de conteúdo, embaixo abaixo disso. Serve para a janela estreita e também para quem quiser manter modal.");

board("Geral.dc.html", "Ajustes · Geral", 1440, 820,
  shell({ width: 1440, height: 820, body: nav("Geral") +
    `<div style="flex:1;min-width:0;padding:18px 22px;overflow:hidden">
      <div style="max-width:720px">
        ${sectionHead("Geral", { sub: "Como o Hark se apresenta nesta máquina." })}
        ${formRow("Nome do assistente", "como você chama o Hark ao falar", input("Hark"))}
        ${formRow("Persona", "instruções permanentes do assistente", btn("abrir CLAUDE.md"))}
        ${formRow("Modelo", "o padrão das respostas de voz", input("sonnet"))}
        ${formRow("Tema", "cores do app e do realce de código", input("Hark"))}
        ${formRow("Idioma da interface", "rótulos e mensagens", input("português"))}
        ${formRow("Atalho global", "abre o HUD de voz de qualquer lugar", input("cmd+shift+space"))}
      </div>
    </div>` }),
  "As seções de formulário ganham teto de medida (720px) em vez de esticar com a janela: uma linha de formulário de 1200px é ilegível. Só Plugins e config.toml usam a largura inteira.");

board("Hoje.dc.html", "0 · hoje", 1440, 820, today({ width: 1440, height: 820 }),
  "O ponto de partida: modal de 760×560 fixos e sopa de pílulas.");
boards.unshift(boards.pop());

// ---------------------------------------------------------------------
// The contact sheet: every artboard inlined (no iframes), so the page
// opens straight from disk without a server. Gitignored; the artboards
// are the tracked source.
// ---------------------------------------------------------------------
const sheet = `<!doctype html><html lang="pt-BR"><head><meta charset="utf-8">
<title>Hark — plugins e configurações</title>
<style>*{box-sizing:border-box;margin:0;padding:0}
body{background:#05070a;color:${T.text};font-family:${T.ui};padding:48px}
h1{font-size:20px;color:${T.strong};font-weight:600}
.lead{color:${T.dim};font-size:13px;line-height:1.7;max-width:780px;margin:10px 0 40px}
.b{margin-bottom:54px}
.bh{display:flex;align-items:baseline;gap:10px;margin-bottom:4px}
.bt{font-size:14px;font-weight:600;color:${T.strong}}
.bd{font-size:11px;color:${T.dim};font-family:${T.mono}}
.bn{color:${T.dim};font-size:12px;line-height:1.6;max-width:780px;margin-bottom:12px}
.frame{border:1px solid ${T.borderStrong};border-radius:16px;overflow:hidden;
  box-shadow:0 18px 60px rgba(0,0,0,.5)}
</style></head><body>
<h1>Plugins e configurações</h1>
<div class="lead">Oito pranchas. A primeira é o que existe hoje; as outras são a proposta —
Ajustes como aba da janela mãe, Plugins em grade de cards com marca por fornecedor e um painel de
detalhe ao lado. Esforço e passos em esforco.md.</div>
${boards
  .map(
    (b) => `<div class="b">
  <div class="bh"><span class="bt">${esc(b.title)}</span><span class="bd">${b.w}×${b.h} · ${esc(b.file)}</span></div>
  <div class="bn">${esc(b.note)}</div>
  <div class="frame" style="width:${b.w}px;height:${b.h}px">${b.inner}</div>
</div>`,
  )
  .join("")}
</body></html>`;

writeFileSync(new URL("canvas.html", import.meta.url), sheet);
console.log(`${boards.length} artboards + canvas.html`);
