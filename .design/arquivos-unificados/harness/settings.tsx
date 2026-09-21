// Dev harness for Settings › Plugins and › config.toml, served by `vite`
// (no Tauri): the real components over a fake backend at the Tauri
// boundary. Saving a text that contains "quebra" is refused, the way the
// real validator refuses a file that would not load.
// Open: http://localhost:1420/.design/arquivos-unificados/harness/settings.html
import { createRoot } from "react-dom/client";
import "/src/styles.css";
import Settings from "/src/components/Settings";
import PluginsPanel from "/src/components/PluginsPanel";

let CONFIG_TEXT = `# Hark — ~/.hark/config.toml
model = "sonnet"
ui_language = "pt"
hotkey = "cmd+shift+space"

[agents.gemini]
args = ["--experimental-acp"]

[agents.claude-gateway]
plugin = "claude"
cmd = "claude"
login_hint = "chave do gateway em env (Virtual Keys)"
env = { ANTHROPIC_BASE_URL = "https://gateway.example.internal", ANTHROPIC_AUTH_TOKEN = "sk-fake" }

[agents.claude-gateway.models]
light = "claude-haiku-4-5"
standard = "claude-sonnet-5"
heavy = "claude-sonnet-5"
max = "auto-routing-plan"
`;

const values = {
  claude_bin: "",
  model: "sonnet",
  projects_dir: "~/.claude/projects",
  language: "pt",
  ui_language: "pt",
  voice: "Luciana",
  whisper_model: "",
  stt: "whisper",
  theme: "hark",
  prompt_budget_chars: 12000,
  hotkey: "cmd+shift+space",
  worker_budget_usd: 2,
  worker_max_turns: 0,
  worker_mode: "",
  worker_model: "",
  worker_effort: "",
  assistant_name: "Hark",
  auto_update: true,
  models: { light: "haiku", standard: "sonnet", heavy: "opus", max: "fable" },
};

const version = (installed: string, behind?: string) => ({
  installed,
  current: behind ?? null,
  freshness: behind ? { state: "behind", installed, current: behind } : { state: "unknown" },
  update: behind ? `npm install -g @google/gemini-cli@${behind}` : null,
  checked_at: new Date().toISOString(),
});

const caps = (over: Record<string, boolean> = {}) => ({
  resume: true, permissions: true, structured_output: true, cost_reporting: true,
  history: true, live_list: true, slash_commands: true, memory_file: "CLAUDE.md",
  shell_tools: [], fork: true, directive_mode: true, directive_model: true,
  directive_effort: true, ...over,
});

const agent = (over: Record<string, unknown>) => ({
  plugin: "claude",
  cmd: "claude",
  vendor: "Anthropic",
  status: "available",
  detected: true,
  enabled: true,
  detail: "/Users/dev/.local/bin/claude",
  install: "",
  selected: false,
  memory_file: "CLAUDE.md",
  login_hint: null,
  args: [],
  env_keys: [],
  models: { light: "haiku", standard: "sonnet", heavy: "opus", max: "fable" },
  version: version("2.1.236"),
  capabilities: caps(),
  ...over,
});

const plugins = [
  agent({ id: "claude", name: "Claude Code", selected: true, login_hint: "claude /login" }),
  agent({
    id: "claude-gateway",
    name: "Claude via gateway",
    login_hint: "chave do gateway em env (Virtual Keys)",
    env_keys: ["ANTHROPIC_BASE_URL", "ANTHROPIC_AUTH_TOKEN"],
    models: {
      light: "claude-haiku-4-5",
      standard: "claude-sonnet-5",
      heavy: "claude-sonnet-5",
      max: "auto-routing-plan",
    },
  }),
  agent({
    id: "gemini",
    name: "Gemini CLI",
    plugin: "acp",
    cmd: "gemini",
    vendor: "Google",
    detail: "",
    install: "npm install -g @google/gemini-cli",
    login_hint: "gemini",
    args: ["--experimental-acp"],
    models: {
      light: "gemini-2.5-flash-lite",
      standard: "gemini-2.5-flash",
      heavy: "gemini-2.5-pro",
      max: "gemini-2.5-pro",
    },
    version: version("0.59.0", "0.60.0"),
    capabilities: caps({ cost_reporting: false, history: false, live_list: false }),
  }),
  agent({
    id: "codex",
    name: "Codex",
    plugin: "acp",
    cmd: "codex-acp",
    vendor: "OpenAI",
    enabled: false,
    detail: "/Users/dev/.local/bin/codex-acp",
    install: "npm install -g @agentclientprotocol/codex-acp",
    login_hint: "codex login",
    args: ["--acp"],
    models: { standard: "gpt-5-codex", heavy: "gpt-5-codex" },
    version: version("0.12.1"),
    capabilities: null,
  }),
  agent({
    id: "qwen-local",
    name: "Qwen local",
    plugin: "acp",
    cmd: "qwen-acp",
    vendor: "",
    detected: false,
    detail: "",
    install: "npm install -g qwen-acp",
    login_hint: null,
    args: ["--acp"],
    models: { standard: "qwen3-coder" },
    version: version(""),
    capabilities: null,
  }),
];

(window as unknown as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__ = {
  invoke: async (cmd: string, args: Record<string, unknown>) => {
    switch (cmd) {
      case "config_read":
        return {
          values,
          path: "/Users/dev/.hark/config.toml",
          claude_bin_resolved: "/Users/dev/.local/bin/claude",
          whisper_model_resolved: "/Users/dev/.hark/models/ggml-small.bin",
          data_dir: "/Users/dev/.hark",
        };
      case "config_write":
        console.log("config_write", args);
        return null;
      case "config_raw_read":
        return { text: CONFIG_TEXT, path: "/Users/dev/.hark/config.toml" };
      case "config_raw_write": {
        const text = String(args.text);
        if (text.includes("quebra")) throw "linha 3: invalid string, expected `\"`";
        CONFIG_TEXT = text;
        return null;
      }
      case "agent_plugins":
        // The real one probes every binary's version: seconds, not ms.
        await new Promise((resolve) => setTimeout(resolve, 1800));
        return plugins;
      case "agent_registry_refresh":
        return plugins;
      case "tts_voices":
        return [["Luciana", "pt_BR"]];
      default:
        return null;
    }
  },
  transformCallback: () => 0,
  metadata: {},
};

// The mother window's shell around the tab body, so the view has the
// flex parent it gets in the app.
// ?compact shows the catalog the way the first-run wizard gets it:
// narrow, no detail panel, and the click picks the agent.
const compact = new URLSearchParams(location.search).has("compact");

createRoot(document.getElementById("root")!).render(
  compact ? (
    <div className="ob-card" style={{ maxWidth: 560, margin: "40px auto" }}>
      <PluginsPanel compact onReady={() => {}} />
    </div>
  ) : (
  <div className="mother mother-wide">
    <nav className="tabs mother-tabs">
      <button>voz</button>
      <button>board</button>
      <button>custos</button>
      <button className="active">ajustes</button>
    </nav>
    <Settings />
  </div>
  ),
);
