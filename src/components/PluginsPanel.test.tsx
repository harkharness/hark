// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render } from "@testing-library/react";
import PluginsPanel from "./PluginsPanel";
import { t } from "../lib/i18n";

const version = {
  installed: "2.1.236",
  current: null,
  freshness: { state: "unknown" },
  update: null,
  // Fresh, so the panel does not reach for the registry on mount.
  checked_at: new Date().toISOString(),
};

const claude = {
  id: "claude",
  name: "Claude Code",
  plugin: "claude",
  cmd: "claude",
  vendor: "Anthropic",
  status: "available",
  detected: true,
  enabled: true,
  detail: "/x/bin/claude",
  install: "",
  selected: true,
  memory_file: "CLAUDE.md",
  login_hint: "claude /login",
  args: [],
  env_keys: [],
  models: { light: "haiku", standard: "sonnet" },
  version,
  capabilities: {
    resume: true,
    permissions: true,
    structured_output: true,
    cost_reporting: true,
    history: true,
    live_list: true,
    slash_commands: true,
    memory_file: "CLAUDE.md",
    shell_tools: [],
    fork: true,
    directive_mode: true,
    directive_model: true,
    directive_effort: true,
  },
};

const twin = {
  ...claude,
  id: "claude-twin",
  name: "Claude via gateway",
  selected: false,
  login_hint: "chave do gateway em env",
  // Keys only: the values are the user's secrets and never leave the backend.
  env_keys: ["ANTHROPIC_BASE_URL", "ANTHROPIC_AUTH_TOKEN"],
  models: { light: "haiku-x", standard: "sonnet-x" },
  capabilities: { ...claude.capabilities, cost_reporting: false, history: false },
};

const invoke = vi.fn(
  async (cmd: string, _args?: unknown): Promise<unknown> =>
    cmd === "agent_plugins" ? [claude, twin] : undefined,
);
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (cmd: string, args?: unknown) => invoke(cmd, args),
}));

beforeEach(() => invoke.mockClear());
afterEach(cleanup);

const called = (cmd: string) => invoke.mock.calls.filter((c) => c[0] === cmd).length;

describe("choosing which agent drives the sessions", () => {
  it("inspects the agent a card is clicked on, without adopting it", async () => {
    // The card used to BE the switch, so reading about an agent meant
    // switching to it. Now only the button in the detail switches.
    const { findByText, getByText } = render(<PluginsPanel onEditConfig={() => {}} />);
    fireEvent.click(await findByText("Claude via gateway"));
    expect(called("agent_plugin_select")).toBe(0);
    expect(getByText("chave do gateway em env")).toBeTruthy();
  });

  it("adopts it when the detail's button is pressed", async () => {
    const { findByText, getByText } = render(<PluginsPanel onEditConfig={() => {}} />);
    fireEvent.click(await findByText("Claude via gateway"));
    fireEvent.click(getByText(t("pl_use")));
    expect(invoke.mock.calls.some((c) => c[0] === "agent_plugin_select")).toBe(true);
  });

  it("opens on the agent that is driving, so the answer is on screen first", async () => {
    const { findAllByText, container } = render(<PluginsPanel onEditConfig={() => {}} />);
    // Twice on screen once it works: the card and the detail it opened on.
    await findAllByText("Claude Code");
    expect(container.querySelector(".agent-detail-name")?.textContent).toBe("Claude Code");
  });
});

describe("an agent's knobs from config, in the detail", () => {
  it("shows the env KEYS and the login hint, never a value", async () => {
    const { findByText, queryByText } = render(<PluginsPanel onEditConfig={() => {}} />);
    fireEvent.click(await findByText("Claude via gateway"));
    expect(await findByText(/ANTHROPIC_AUTH_TOKEN/)).toBeTruthy();
    expect(queryByText(/sk-/)).toBeNull();
  });

  it("counts on the card only the capabilities the detail lists", async () => {
    // The sheet also carries structured_output, fork and the directive
    // flags; counting those said "11 capacidades" for an agent with six.
    const { findByText } = render(<PluginsPanel onEditConfig={() => {}} />);
    expect(await findByText(new RegExp(t("pl_card_caps", { n: 6 })))).toBeTruthy();
  });

  it("marks a capability the agent does NOT have, instead of leaving it out", async () => {
    // A missing pill read as "we don't know"; an agent with no cost
    // reporting is a fact the user needs before adopting it.
    const { findByText, container } = render(<PluginsPanel onEditConfig={() => {}} />);
    fireEvent.click(await findByText("Claude via gateway"));
    const off = [...container.querySelectorAll(".agent-cap.off")].map((n) => n.textContent);
    expect(off).toContain(t("pl_cap_cost"));
    expect(off).toContain(t("pl_cap_hist"));
    expect([...container.querySelectorAll(".agent-cap.on")].map((n) => n.textContent)).toContain(
      t("pl_cap_perm"),
    );
  });

  it("offers to edit its table in config.toml, naming the table", async () => {
    const onEditConfig = vi.fn();
    const { findByText, getByTitle } = render(<PluginsPanel onEditConfig={onEditConfig} />);
    fireEvent.click(await findByText("Claude via gateway"));
    fireEvent.click(getByTitle(t("pl_edit_config")));
    expect(onEditConfig).toHaveBeenCalledWith("agents.claude-twin");
  });
});

describe("adding an agent", () => {
  it("is the last card, and writes the three keys an entry needs", async () => {
    const onEditConfig = vi.fn();
    const { findByText, getByPlaceholderText, getByText } = render(
      <PluginsPanel onEditConfig={onEditConfig} />,
    );
    fireEvent.click(await findByText(t("pl_add_agent")));
    fireEvent.change(getByPlaceholderText(t("pl_add_id_ph")), { target: { value: "qwen" } });
    fireEvent.change(getByPlaceholderText(t("pl_add_cmd_ph")), { target: { value: "qwen-acp" } });
    fireEvent.click(getByText(t("pl_add_go")));
    await vi.waitFor(() => expect(onEditConfig).toHaveBeenCalledWith("agents.qwen"));
    const write = invoke.mock.calls.find((c) => c[0] === "config_write");
    expect(write?.[1]).toMatchObject({
      patch: { "agents.qwen.plugin": "claude", "agents.qwen.cmd": "qwen-acp", "agents.qwen.enabled": true },
    });
  });
});

const absent = {
  ...twin,
  id: "qwen-local",
  name: "Qwen local",
  vendor: "",
  detected: false,
  detail: "",
  install: "npm install -g qwen-acp",
  capabilities: null,
};

describe("in the first-run wizard, where the only job is to pick", () => {
  it("still hands over the install line for an agent that is not here", async () => {
    // A fresh machine is exactly where this matters: with no detail
    // panel, a card saying "not found" and nothing else is a dead end.
    invoke.mockImplementationOnce(async () => [absent]);
    const { findByText } = render(<PluginsPanel compact />);
    expect(await findByText("npm install -g qwen-acp")).toBeTruthy();
  });

  it("switches an agent on from the card, where the detail would have", async () => {
    invoke.mockImplementationOnce(async () => [{ ...twin, enabled: false }]);
    const { findByText } = render(<PluginsPanel compact />);
    fireEvent.click(await findByText(t("pl_enable")));
    expect(invoke.mock.calls.some((c) => c[0] === "agent_plugin_enable")).toBe(true);
  });

  it("keeps the click as the choice and shows no detail panel", async () => {
    const { findByText, container } = render(<PluginsPanel compact />);
    fireEvent.click(await findByText("Claude via gateway"));
    expect(called("agent_plugin_select")).toBe(1);
    expect(container.querySelector(".agent-detail")).toBeNull();
  });
});

describe("while the catalog is still being read", () => {
  it("shows placeholders where the cards will be, and no form or footer yet", async () => {
    // The backend probes every binary's version before it answers, which
    // takes seconds: an empty list in the meantime read as "no agents".
    invoke.mockImplementationOnce(() => new Promise<never>(() => {}));
    const { getByText, queryByText, container } = render(<PluginsPanel onEditConfig={() => {}} />);
    expect(getByText(t("pl_loading"))).toBeTruthy();
    expect(container.querySelectorAll(".plugin-skeleton").length).toBeGreaterThan(0);
    expect(queryByText(t("pl_add_agent"))).toBeNull();
    expect(queryByText(t("pl_check_updates"))).toBeNull();
  });
});
