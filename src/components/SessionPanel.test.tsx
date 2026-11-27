// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import { t } from "../lib/i18n";

vi.mock("../lib/ipc", () => ({
  sessionContextWeight: async () => null,
  spendSummary: async () => [],
  subscriptionLimits: async () => null,
}));
vi.mock("./DiffPanel", async () => {
  const { useEffect } = await import("react");
  return {
    default: ({ onTotals }: { onTotals?: (s: { added: number; removed: number; files: number }) => void }) => {
      useEffect(() => {
        onTotals?.({ added: 21, removed: 4, files: 2 });
        // eslint-disable-next-line react-hooks/exhaustive-deps
      }, []);
      return <div data-testid="diff" />;
    },
  };
});

import SessionPanel from "./SessionPanel";

afterEach(cleanup);
beforeEach(() => window.localStorage.clear());

const base = {
  scope: { kind: "chat", project: { name: "tulipafin", path: "/p/tulipafin" } } as never,
  projectsCount: 3,
  taskTitle: "Billing v2",
  costs: {},
  outputs: [],
  onOpenPath: () => {},
};

describe("the session bar", () => {
  it("folds a section and remembers it; the diff lives in 'changes' with its totals in the head", async () => {
    const onCommit = vi.fn();
    const { getByText, getByTestId, queryByTestId, queryByText } = render(<SessionPanel {...base} changes={{ workspace: "/p/tulipafin", tick: 0 }} onCommit={onCommit} />);
    expect(getByTestId("diff")).toBeTruthy();
    await waitFor(() => expect(getByText("+21")).toBeTruthy());
    fireEvent.click(getByText(t("sp_commit")));
    expect(onCommit).toHaveBeenCalled();
    fireEvent.click(getByText(t("sec_changes")));
    expect(queryByTestId("diff")).toBeNull();
    expect(getByText("+21")).toBeTruthy();
    expect(JSON.parse(window.localStorage.getItem("hark.session.sections")!)).toEqual({ changes: false });
    // Context opens on a click and closes again.
    expect(queryByText(t("ctx_none"), { selector: "p" })).toBeNull();
    fireEvent.click(getByText(t("sec_ctx")));
    expect(getByText(t("ctx_none"), { selector: "p" })).toBeTruthy();
  });

  it("connectors show their state, and background tasks their count until they finish", () => {
    const onPlugins = vi.fn();
    const { getByText, getByTitle, container } = render(
      <SessionPanel
        {...base}
        connectors={[{ name: "github", status: "connected" }, { name: "figma", status: "failed" }]}
        background={[{ id: "b1", description: "Run the spike", command: "cargo run", startedAt: Date.now() - 16000 }, { id: "b2", description: "List pods", command: "kubectl", startedAt: 0, doneAt: 1, status: "completed", summary: "ok" }]}
        onPlugins={onPlugins}
      />,
    );
    fireEvent.click(getByText(t("sec_connectors")));
    expect(getByText("github")).toBeTruthy();
    expect(container.querySelectorAll(".sp-dot.err").length).toBe(1);
    expect(getByText(t("sp_conn_reconnect_n", { n: 1 }), { exact: false })).toBeTruthy();
    fireEvent.click(getByText(t("sp_conn_plugins")));
    expect(onPlugins).toHaveBeenCalled();
    expect(getByTitle("cargo run")).toBeTruthy();
    expect(getByText(t("sp_bg_done_n", { n: 1 }))).toBeTruthy();
  });

  it("the project is its name and where it lives, home as ~, never the whole path across the bar", () => {
    const scope = { kind: "chat", project: { name: "tulipafin", path: "/Users/ana/code/tulipafin" } } as never;
    const { getByText, queryByText } = render(<SessionPanel {...base} scope={scope} />);
    expect(getByText("tulipafin")).toBeTruthy();
    expect(getByText("~/code")).toBeTruthy();
    expect(queryByText("/Users/ana/code/tulipafin")).toBeNull();
    // The whole path is still one hover away.
    expect(getByText("tulipafin").closest(".sp-row")?.getAttribute("title")).toBe("/Users/ana/code/tulipafin");
  });

  it("the branch and the agent are rows of their own, each with what it is in front", () => {
    const repo = { root: "/p/tulipafin", branch: "billing-v2", upstream: null, ahead: 2, behind: 0, dirty: 3, added: 0, removed: 0 };
    const { getByText } = render(<SessionPanel {...base} repo={repo} agentName="Claude Code" mode="auto" model="opus" />);
    expect(getByText("billing-v2").closest(".sp-row")?.querySelector("svg")).toBeTruthy();
    expect(getByText("Claude Code").closest(".sp-row")?.querySelector("svg")).toBeTruthy();
  });

  it("background tasks come before the changes and the outputs: they are what is looked at most", () => {
    const { container } = render(<SessionPanel {...base} />);
    const order = [...container.querySelectorAll("[data-section]")].map((el) => el.getAttribute("data-section"));
    expect(order.indexOf("bg")).toBeGreaterThan(-1);
    expect(order.indexOf("bg")).toBeLessThan(order.indexOf("changes"));
    expect(order.indexOf("bg")).toBeLessThan(order.indexOf("outputs"));
    expect(order[0]).toBe("env");
    // What changed in the repository reads before the list of files written.
    expect(order.indexOf("changes")).toBeLessThan(order.indexOf("outputs"));
  });

  it("a background task says how long it has been running, in the unit a person would use", () => {
    const { getByText } = render(
      <SessionPanel
        {...base}
        background={[
          { id: "b1", description: "Watch the CI run", command: "gh run watch", startedAt: Date.now() - (4 * 86_400_000 + 16 * 3_600_000) },
          { id: "b2", description: "Dev server", command: "npm run dev", startedAt: Date.now() - 5 * 60_000 },
        ]}
      />,
    );
    expect(getByText("Watch the CI run").closest(".sp-row")?.textContent).toContain("4 d 16 h");
    expect(getByText("Dev server").closest(".sp-row")?.textContent).toContain("5 min");
  });

  it("without a repository the changes section says so", () => {
    const { getByText } = render(<SessionPanel {...base} changes={{ workspace: undefined, tick: 0 }} />);
    expect(getByText(t("sp_no_repo"))).toBeTruthy();
  });

  it("an output that is a page shows its title and its kind; a markdown of the workspace is a doc", () => {
    const { getByText, container } = render(
      <SessionPanel
        {...base}
        outputs={[
          { path: "/Users/ana/.hark/artifacts/tulipa/custos.sheet.json", kind: "written", touches: 1, page: { space: "tulipa", id: "custos", title: "Custos por mês", kind: "sheet" } },
          { path: "/p/tulipafin/README.md", kind: "edited", touches: 1 },
        ]}
      />,
    );
    expect(getByText("Custos por mês")).toBeTruthy();
    expect(container.querySelector(".sp-out .art-kind.sheet")).toBeTruthy();
    expect(getByText("README.md")).toBeTruthy();
    expect(container.querySelectorAll(".sp-out .sp-out-doc").length).toBe(1);
  });
});
