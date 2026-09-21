// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render } from "@testing-library/react";
import Settings from "./Settings";
import { t } from "../lib/i18n";

const snapshot = {
  values: {
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
    models: {},
  },
  path: "/Users/dev/.hark/config.toml",
  claude_bin_resolved: "/Users/dev/.local/bin/claude",
  whisper_model_resolved: "",
  data_dir: "/Users/dev/.hark",
};

vi.mock("@tauri-apps/api/app", () => ({ getVersion: async () => "0.6.0" }));
vi.mock("@tauri-apps/plugin-updater", () => ({ check: async () => null }));
vi.mock("@tauri-apps/plugin-process", () => ({ relaunch: async () => {} }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === "config_read") return snapshot;
    if (cmd === "tts_voices") return [["Luciana", "pt_BR"]];
    if (cmd === "agent_plugins") return [];
    return undefined;
  }),
}));

afterEach(cleanup);

describe("settings as a view of the mother window", () => {
  it("is not a dialog over the app: no backdrop, no fixed box", async () => {
    // It used to be a modal frozen at 760x560 while the window grew
    // around it, with the config.toml editor inside that box.
    const { findAllByText, container } = render(<Settings />);
    await findAllByText(t("set_general"));
    expect(container.querySelector(".modal-backdrop")).toBeNull();
    expect(container.querySelector(".setview")).toBeTruthy();
  });

  it("groups the six sections under the three things they configure", async () => {
    const { findAllByText, getByText } = render(<Settings />);
    await findAllByText(t("set_general"));
    for (const group of ["set_group_you", "set_group_agents", "set_group_machine"] as const)
      expect(getByText(t(group))).toBeTruthy();
  });

  it("keeps the form sections in a readable measure instead of stretching", async () => {
    const { findAllByText, container } = render(<Settings />);
    await findAllByText(t("set_general"));
    expect(container.querySelector(".set-form")).toBeTruthy();
  });

  it("moves to another section when its nav item is clicked", async () => {
    const { findAllByText, getByText, container } = render(<Settings />);
    await findAllByText(t("set_general"));
    fireEvent.click(getByText(t("set_workers")));
    expect(getByText(t("set_budget"))).toBeTruthy();
    expect(container.querySelector(".set-form")).toBeTruthy();
  });
});
