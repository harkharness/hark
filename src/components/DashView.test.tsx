// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import type { DashView as View } from "../types";

const calls = vi.hoisted(() => ({
  view: vi.fn(),
  certify: vi.fn(async () => undefined),
  accept: vi.fn(async () => undefined),
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
  artifactAccept: calls.accept,
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

  it("a broken file shows the error and the raw text", async () => {
    calls.view.mockResolvedValue({ ...view, error: "não é json: x", panels: [] });
    const { findByText } = render(<DashView space="tulipa" id="uso" />);
    expect(await findByText(t("dash_file_err", { e: "não é json: x" }))).toBeTruthy();
  });
});
