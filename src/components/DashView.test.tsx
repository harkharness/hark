// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import type { DashView as View } from "../types";

const calls = vi.hoisted(() => ({
  view: vi.fn(),
  certify: vi.fn(async () => undefined),
  edit: vi.fn(async () => undefined),
  accept: vi.fn(async () => undefined),
  story: vi.fn(async () => "A retenção subiu 23 pontos."),
  create: vi.fn(async () => ({ id: "historia-uso-do-app", title: "x", path: "x.md", kind: "doc", updated_at: "" })),
  write: vi.fn(async () => undefined),
}));

const view: View = {
  title: "Uso do app",
  refresh: "hourly",
  sources: [
    { id: "db", kind: "sqlite", path: "data/app.db" },
    { id: "st", kind: "command", command: "./scripts/stats.sh" },
  ],
  queries: [
    { id: "por_dia", source: "db", sql: "SELECT dia, n FROM lancamentos", columns: [], certified: true, changed: false, preview: { columns: ["dia", "n"], rows: [["01", 3], ["02", 5]] } },
    { id: "ativos", source: "st", sql: null, columns: [], certified: false, changed: true, preview: { columns: ["n"], rows: [[1284]] } },
    { id: "quebrada", source: "db", sql: "SELECT x FROM nada", columns: [], certified: false, changed: true, preview: null, error: "no such table: nada" },
  ],
  panels: [
    { title: "usuários ativos", query: "ativos", chart: "number", certified: false, number: 1284, delta: 0.062 },
    { title: "por dia", query: "por_dia", chart: "bar", certified: true, labels: ["01", "02"], values: [3, 5] },
    { title: "retenção", query: "por_dia", chart: "line", certified: true, labels: ["set", "out"], values: [38, 41] },
    { title: "tabela", query: "por_dia", chart: "table", certified: true, table: { columns: ["dia", "n"], rows: [["01", 3]] } },
    { title: "quebrado", query: "quebrada", chart: "number", certified: false, error: "no such table: nada" },
  ],
  cache_at: "2026-11-10T09:40:00Z",
  stale: false,
  changed: ["ativos", "quebrada"],
  versions: 3,
  raw: "{}",
};

vi.mock("../lib/ipc", () => ({
  dashView: calls.view,
  dashCertify: calls.certify,
  dashEdit: calls.edit,
  artifactAccept: calls.accept,
  dashStory: calls.story,
  artifactCreate: calls.create,
  artifactWrite: calls.write,
}));

import DashView from "./DashView";
import { t } from "../lib/i18n";

afterEach(() => {
  cleanup();
  calls.view.mockReset();
});

describe("the dashboard viewer", () => {
  it("draws every panel from the cache and says when the numbers were taken", async () => {
    calls.view.mockResolvedValue(view);
    const { findByText, getByText, getAllByText, container } = render(<DashView space="tulipa" id="uso" />);
    expect(await findByText("usuários ativos")).toBeTruthy();
    expect(getByText((1284).toLocaleString())).toBeTruthy();
    expect(getByText(`▲ ${(6.2).toLocaleString(undefined, { maximumFractionDigits: 1 })}%`)).toBeTruthy();
    expect(container.querySelectorAll(".dash-bar").length).toBe(2);
    expect(container.querySelector(".dash-line path")).toBeTruthy();
    expect(getByText("no such table: nada")).toBeTruthy();
    expect(getAllByText(t("dash_certified")).length).toBeGreaterThan(0);
    expect(getByText(t("dash_changed_n", { n: 2 }))).toBeTruthy();
    expect(calls.view).toHaveBeenCalledWith("tulipa", "uso", false);
  });

  it("refreshing runs the sources, commands included", async () => {
    calls.view.mockResolvedValue(view);
    const { findByText, getByText } = render(<DashView space="tulipa" id="uso" />);
    await findByText("usuários ativos");
    fireEvent.click(getByText(t("dash_refresh")));
    await waitFor(() => expect(calls.view).toHaveBeenCalledWith("tulipa", "uso", true));
  });

  it("the sources and queries screen certifies a query and shows its preview", async () => {
    calls.view.mockResolvedValue(view);
    const { findByText, getByText, getAllByText } = render(<DashView space="tulipa" id="uso" />);
    await findByText("usuários ativos");
    fireEvent.click(getByText(t("dash_sources_tab")));
    expect(getByText("./scripts/stats.sh")).toBeTruthy();
    fireEvent.click(getByText("ativos"));
    expect(getAllByText((1284).toLocaleString()).length).toBeGreaterThan(0);
    fireEvent.click(getByText(t("dash_certify")));
    await waitFor(() => expect(calls.certify).toHaveBeenCalledWith("tulipa", "uso", "ativos", true));
  });

  it("the data story is written when asked, read, and becomes a child document", async () => {
    calls.view.mockResolvedValue(view);
    const onDoc = vi.fn();
    const { findByText, getByText } = render(<DashView space="tulipa" id="uso" onDoc={onDoc} />);
    await findByText("usuários ativos");
    expect(calls.story).not.toHaveBeenCalled();
    fireEvent.click(getByText(t("dash_story_write")));
    expect(await findByText("A retenção subiu 23 pontos.")).toBeTruthy();
    fireEvent.click(getByText(t("dash_story_to_doc")));
    await waitFor(() => expect(calls.create).toHaveBeenCalledWith("tulipa", "uso", t("dash_story_doc_title", { t: "Uso do app" }), "doc"));
    await waitFor(() => expect(calls.write).toHaveBeenCalled());
    expect((calls.write.mock.calls[0] as unknown[])[2]).toContain("A retenção subiu 23 pontos.");
    await waitFor(() => expect(onDoc).toHaveBeenCalledWith("historia-uso-do-app"));
  });

  it("a broken file shows the error and the raw text", async () => {
    calls.view.mockResolvedValue({ ...view, error: "não é json: x", panels: [] });
    const { findByText } = render(<DashView space="tulipa" id="uso" />);
    expect(await findByText(t("dash_file_err", { e: "não é json: x" }))).toBeTruthy();
  });

  it("a query is edited in a form and saved as the person's", async () => {
    calls.view.mockResolvedValue(view);
    const { findByText, getByText, getByDisplayValue } = render(<DashView space="tulipa" id="uso" />);
    await findByText("usuários ativos");
    fireEvent.click(getByText(t("dash_sources_tab")));
    fireEvent.click(getByText(t("dash_edit_query")));
    const sql = getByDisplayValue("SELECT dia, n FROM lancamentos") as HTMLTextAreaElement;
    fireEvent.change(sql, { target: { value: "SELECT dia, n FROM lancamentos WHERE n > 0" } });
    fireEvent.click(getByText(t("board_save")));
    await waitFor(() =>
      expect(calls.edit).toHaveBeenCalledWith("tulipa", "uso", { op: "query", id: "por_dia", source: "db", sql: "SELECT dia, n FROM lancamentos WHERE n > 0", columns: [], limit: null }),
    );
  });

  it("a panel is edited through its gear and a new source through the sources screen", async () => {
    calls.view.mockResolvedValue(view);
    const { findByText, getByText, getAllByTitle, getByDisplayValue, getByPlaceholderText } = render(<DashView space="tulipa" id="uso" />);
    await findByText("usuários ativos");
    fireEvent.click(getAllByTitle(t("dash_edit_panel"))[1]);
    const title = getByDisplayValue("por dia") as HTMLInputElement;
    fireEvent.change(title, { target: { value: "lançamentos por dia" } });
    fireEvent.click(getByText(t("board_save")));
    await waitFor(() =>
      expect(calls.edit).toHaveBeenCalledWith("tulipa", "uso", { op: "panel", index: 1, title: "lançamentos por dia", query: "por_dia", chart: "bar", x: null, y: null, value: null, previous: null }),
    );
    fireEvent.click(getByText(t("dash_sources_tab")));
    fireEvent.click(getByText(t("dash_new_source")));
    fireEvent.change(getByPlaceholderText(t("dash_query_id")), { target: { value: "custos" } });
    fireEvent.change(getByPlaceholderText(t("dash_path")), { target: { value: "data/custos.csv" } });
    fireEvent.click(getByText(t("board_save")));
    await waitFor(() => expect(calls.edit).toHaveBeenCalledWith("tulipa", "uso", { op: "add_source", id: "custos", kind: "csv", path: "data/custos.csv", command: null }));
  });
});
