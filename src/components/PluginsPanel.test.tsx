// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render } from "@testing-library/react";
import PluginsPanel from "./PluginsPanel";
import { t } from "../lib/i18n";

const twin = {
  id: "claude-twin",
  name: "Claude via gateway",
  plugin: "claude",
  cmd: "claude",
  vendor: "Anthropic",
  status: "available",
  detected: true,
  enabled: true,
  detail: "/x/bin/claude",
  install: "",
  selected: false,
  memory_file: "CLAUDE.md",
  login_hint: "chave do gateway em env",
  args: [],
  // Keys only: the values are the user's secrets and never leave the backend.
  env_keys: ["ANTHROPIC_BASE_URL", "ANTHROPIC_AUTH_TOKEN"],
  models: { light: "haiku-x", standard: "sonnet-x" },
  version: {
    installed: "2.1.236",
    current: null,
    freshness: { state: "unknown" },
    update: null,
    // Fresh, so the panel does not reach for the registry on mount.
    checked_at: new Date().toISOString(),
  },
  capabilities: null,
};

vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string) => (cmd === "agent_plugins" ? [twin] : undefined)),
}));

afterEach(cleanup);

describe("an agent's knobs from config, on its card", () => {
  it("shows the env KEYS and the login hint, never a value", async () => {
    const { findByText, queryByText } = render(<PluginsPanel onEditConfig={() => {}} />);
    expect(await findByText(/ANTHROPIC_AUTH_TOKEN/)).toBeTruthy();
    expect(queryByText(/sk-/)).toBeNull();
    expect(await findByText(/chave do gateway em env/)).toBeTruthy();
  });

  it("offers to edit its table in config.toml, naming the table", async () => {
    const onEditConfig = vi.fn();
    const { findByTitle } = render(<PluginsPanel onEditConfig={onEditConfig} />);
    fireEvent.click(await findByTitle(t("pl_edit_config")));
    expect(onEditConfig).toHaveBeenCalledWith("agents.claude-twin");
  });

  it("without an editor at hand (the wizard), the card offers nothing to edit", async () => {
    const { findByText, queryByTitle } = render(<PluginsPanel compact />);
    await findByText("Claude via gateway");
    expect(queryByTitle(t("pl_edit_config"))).toBeNull();
  });
});
