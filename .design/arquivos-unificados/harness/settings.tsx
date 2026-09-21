// Dev harness for Settings › Plugins and › config.toml, served by `vite`
// (no Tauri): the real components over a fake backend at the Tauri
// boundary. Saving a text that contains "quebra" is refused, the way the
// real validator refuses a file that would not load.
// Open: http://localhost:1420/.design/arquivos-unificados/harness/settings.html
import { createRoot } from "react-dom/client";
import "/src/styles.css";
import Settings from "/src/components/Settings";

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

const version = (installed: string) => ({
  installed,
  current: null,
  freshness: { state: "unknown" },
  update: null,
  checked_at: new Date().toISOString(),
});

const plugins = [
  {
    id: "claude",
    name: "Claude Code",
    plugin: "claude",
    cmd: "claude",
    vendor: "Anthropic",
    status: "available",
    detected: true,
    enabled: true,
    detail: "/Users/dev/.local/bin/claude",
    install: "",
    selected: true,
    memory_file: "CLAUDE.md",
    login_hint: null,
    args: [],
    env_keys: [],
    models: { light: "haiku", standard: "sonnet", heavy: "opus", max: "fable" },
    version: version("2.1.236"),
    capabilities: { permissions: true, cost_reporting: true, history: true, resume: true, slash_commands: true, live_list: true },
  },
  {
    id: "claude-gateway",
    name: "Claude Code (gateway)",
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
    login_hint: "chave do gateway em env (Virtual Keys)",
    args: [],
    env_keys: ["ANTHROPIC_BASE_URL", "ANTHROPIC_AUTH_TOKEN"],
    models: { light: "claude-haiku-4-5", standard: "claude-sonnet-5", heavy: "claude-sonnet-5", max: "auto-routing-plan" },
    version: version("2.1.236"),
    capabilities: { permissions: true, cost_reporting: true, history: true, resume: true, slash_commands: true, live_list: true },
  },
  {
    id: "gemini",
    name: "Gemini CLI",
    plugin: "acp",
    cmd: "gemini",
    vendor: "Google",
    status: "available",
    detected: true,
    enabled: true,
    detail: "",
    install: "npm install -g @google/gemini-cli",
    selected: false,
    memory_file: "GEMINI.md",
    login_hint: "gemini",
    args: ["--experimental-acp"],
    env_keys: [],
    models: { light: "gemini-2.5-flash-lite", standard: "gemini-2.5-flash", heavy: "gemini-2.5-pro", max: "gemini-2.5-pro" },
    version: version("0.59.0"),
    capabilities: null,
  },
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

createRoot(document.getElementById("root")!).render(<Settings open onClose={() => {}} />);
