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

  it("without a repository the changes section says so", () => {
    const { getByText } = render(<SessionPanel {...base} changes={{ workspace: undefined, tick: 0 }} />);
    expect(getByText(t("sp_no_repo"))).toBeTruthy();
  });
});
