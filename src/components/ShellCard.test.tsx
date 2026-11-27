// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render } from "@testing-library/react";
import ShellCard from "./ShellCard";
import { t } from "../lib/i18n";
import type { ShellMsg } from "../lib/bang";

afterEach(cleanup);

const ran = (more: Partial<ShellMsg> = {}): ShellMsg => ({
  who: "shell",
  id: "r1",
  command: "git status",
  cwd: "/p/app",
  running: false,
  output: "On branch main\n",
  code: 0,
  ...more,
});

describe("a command run from the composer, in the chat", () => {
  it("shows the command and what it printed, as printed", () => {
    const { getByText, container } = render(<ShellCard m={ran({ output: "a  b\n  c\n" })} />);
    expect(getByText("git status")).toBeTruthy();
    // Columns and indentation are the output: nothing collapses them.
    expect(container.querySelector("pre")?.textContent).toBe("a  b\n  c\n");
  });

  it("shows the folder it ran in", () => {
    const { getByText } = render(<ShellCard m={ran()} />);
    expect(getByText("/p/app")).toBeTruthy();
  });

  it("a deep folder shows its end, where the project's name is, and keeps the whole path at hand", () => {
    const { getByText } = render(<ShellCard m={ran({ cwd: "/Users/ana/code/webhook-api" })} />);
    const dir = getByText("…/code/webhook-api");
    expect(dir.getAttribute("title")).toBe("/Users/ana/code/webhook-api");
  });

  it("prints output as text: markup in it is never built into the page", () => {
    const { container } = render(<ShellCard m={ran({ output: '<img src=x onerror="boom()">\n[open](file:///etc)\n' })} />);
    expect(container.querySelector("img")).toBeNull();
    expect(container.querySelector("a")).toBeNull();
    expect(container.querySelector("pre")?.textContent).toContain("<img src=x");
  });

  it("while it runs says so and offers to stop it", () => {
    const onStop = vi.fn();
    const { getByText } = render(<ShellCard m={ran({ running: true, output: undefined, code: undefined })} onStop={onStop} />);
    expect(getByText(t("shell_running"))).toBeTruthy();
    fireEvent.click(getByText(t("shell_stop")));
    expect(onStop).toHaveBeenCalledWith("r1");
  });

  it("once it ended there is nothing to stop", () => {
    const { queryByText } = render(<ShellCard m={ran()} onStop={vi.fn()} />);
    expect(queryByText(t("shell_stop"))).toBeNull();
    expect(queryByText(t("shell_running"))).toBeNull();
  });

  it("a command that went well says nothing about its exit", () => {
    const { queryByText } = render(<ShellCard m={ran()} />);
    expect(queryByText(t("shell_exit", { n: 0 }))).toBeNull();
  });

  it("a command that failed says with which code", () => {
    const { getByText, container } = render(<ShellCard m={ran({ code: 2, output: "1 failed\n" })} />);
    expect(getByText(t("shell_exit", { n: 2 }))).toBeTruthy();
    expect(container.querySelector(".shell-card.failed")).toBeTruthy();
  });

  it("a command with nothing to print says so instead of showing an empty box", () => {
    const { getByText, container } = render(<ShellCard m={ran({ output: "" })} />);
    expect(getByText(t("shell_no_output"))).toBeTruthy();
    expect(container.querySelector("pre")).toBeNull();
  });

  it("a stopped command says it was stopped, not that it failed", () => {
    const { getByText, queryByText } = render(<ShellCard m={ran({ code: null, stopped: true })} />);
    expect(getByText(t("shell_stopped"))).toBeTruthy();
    expect(queryByText(t("shell_killed"))).toBeNull();
  });

  it("a command ended for running too long says why and where to run it", () => {
    const { getByText } = render(<ShellCard m={ran({ code: null, timedOut: true })} />);
    expect(getByText(t("shell_timeout"))).toBeTruthy();
  });

  it("says when the middle of a long output was left out", () => {
    const { getByText } = render(<ShellCard m={ran({ omitted: 4200 })} />);
    expect(getByText(t("shell_cut", { n: 4200 }))).toBeTruthy();
  });

  it("a command that never started shows the reason", () => {
    const { getByText } = render(<ShellCard m={ran({ output: undefined, code: undefined, error: "no such shell" })} />);
    expect(getByText(t("shell_not_started", { e: "no such shell" }))).toBeTruthy();
  });

  it("tells the output goes to the agent with the next message, until it has", () => {
    const waiting = render(<ShellCard m={ran()} toAgent />);
    expect(waiting.getByText(t("shell_to_agent"))).toBeTruthy();
    cleanup();
    const told = render(<ShellCard m={ran({ told: true })} toAgent />);
    expect(told.queryByText(t("shell_to_agent"))).toBeNull();
  });

  it("outside a chat there is no agent to tell", () => {
    const { queryByText } = render(<ShellCard m={ran()} />);
    expect(queryByText(t("shell_to_agent"))).toBeNull();
  });
});
