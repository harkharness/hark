// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import type { SheetView as View } from "../types";

const calls = vi.hoisted(() => ({
  accept: vi.fn(async () => undefined),
  restore: vi.fn(async () => undefined),
  setCell: vi.fn(async () => undefined),
  edit: vi.fn(async () => undefined),
  clipboard: vi.fn(async () => undefined),
}));

const view: View = {
  computed: {
    title: "Custos de infra",
    tabs: [
      {
        name: "Mensal",
        columns: ["mês", "infra"],
        formats: [null, "money"],
        widths: [140, null],
        cells: [
          [{ raw: "set/26", text: "set/26", formula: false }, { raw: "412", text: "412", formula: false, number: 412 }],
          [{ raw: "out/26", text: "out/26", formula: false }, { raw: "=B2*1.03", text: "424.36", formula: true, number: 424.36 }],
          [{ raw: "erro", text: "erro", formula: false }, { raw: "=FOO(1)", text: "#NOME?", formula: true, error: "#NOME?" }],
        ],
      },
      { name: "Por serviço", columns: ["serviço"], cells: [[{ raw: "api", text: "api", formula: false }]] },
    ],
  },
  changed: [{ tab: 0, row: 1, col: 1 }],
  versions: 2,
  raw: "{}",
};

vi.mock("../lib/ipc", () => ({
  sheetView: async () => view,
  sheetSetCell: calls.setCell,
  sheetEdit: calls.edit,
  artifactAccept: calls.accept,
  artifactRestore: calls.restore,
  artifactVersions: async () => [{ at: "2026-11-10T10:00:00Z", bytes: 400 }, { at: "2026-11-10T10:05:00Z", bytes: 420 }],
}));

import SheetView from "./SheetView";
import { t } from "../lib/i18n";

Object.assign(navigator, { clipboard: { writeText: calls.clipboard } });
afterEach(cleanup);

describe("the sheet viewer", () => {
  it("shows the computed values, marks the agent's changes and names a cell's error", async () => {
    const { findByText, getByText, getByTitle } = render(<SheetView space="tulipa" id="custos" />);
    expect(await findByText("424.36")).toBeTruthy();
    // The changed cell wears the mark; the banner counts it.
    expect(getByText("424.36").closest("td")!.className).toContain("changed");
    expect(getByText(t("sheet_changed_n", { n: 1 }))).toBeTruthy();
    // An error cell shows its name and says why on hover.
    expect(getByTitle("#NOME?")).toBeTruthy();
    // Both tabs are there; the second opens on click.
    fireEvent.click(getByText("Por serviço"));
    expect(getByText("api")).toBeTruthy();
  });

  it("accepting takes the changes and undoing goes back to the baseline", async () => {
    const { findByText, getByText } = render(<SheetView space="tulipa" id="custos" />);
    await findByText("424.36");
    fireEvent.click(getByText(t("sheet_accept")));
    await waitFor(() => expect(calls.accept).toHaveBeenCalledWith("tulipa", "custos"));
    fireEvent.click(getByText(t("sheet_undo")));
    await waitFor(() => expect(calls.restore).toHaveBeenCalledWith("tulipa", "custos", "2026-11-10T10:05:00Z"));
  });

  it("a selected cell shows its formula in the bar, and typing into it writes the file", async () => {
    const { findByText, getByText, getByDisplayValue } = render(<SheetView space="tulipa" id="custos" />);
    await findByText("424.36");
    fireEvent.click(getByText("424.36"));
    expect(getByText("=B2*1.03")).toBeTruthy();
    fireEvent.doubleClick(getByText("424.36"));
    const input = getByDisplayValue("=B2*1.03") as HTMLInputElement;
    fireEvent.change(input, { target: { value: "440" } });
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => expect(calls.setCell).toHaveBeenCalledWith("tulipa", "custos", { tab: 0, row: 1, col: 1 }, "440"));
  });

  it("copies the open tab as csv with values, not formulas", async () => {
    const { findByText, getByText } = render(<SheetView space="tulipa" id="custos" />);
    await findByText("424.36");
    fireEvent.click(getByText(t("sheet_copy_csv")));
    await waitFor(() => expect(calls.clipboard).toHaveBeenCalledWith("mês,infra\nset/26,412\nout/26,424.36\nerro,#NOME?\n"));
  });

  it("a column's format shapes its numbers and its width is the file's", async () => {
    const { findByText, container } = render(<SheetView space="tulipa" id="custos" />);
    expect(await findByText((412).toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 }))).toBeTruthy();
    const first = container.querySelector("th.hd") as HTMLElement;
    expect(first.style.width).toBe("140px");
  });

  it("the column menu inserts, removes and formats; the row menu inserts and removes", async () => {
    const { findByText, getByText, getAllByText } = render(<SheetView space="tulipa" id="custos" />);
    await findByText("set/26");
    fireEvent.click(getByText("B"));
    fireEvent.click(getByText(t("sheet_insert_right")));
    await waitFor(() => expect(calls.edit).toHaveBeenCalledWith("tulipa", "custos", { op: "insert_col", tab: 0, at: 2 }));
    fireEvent.click(getByText("B"));
    fireEvent.click(getByText(t("sheet_fmt_pct")));
    await waitFor(() => expect(calls.edit).toHaveBeenCalledWith("tulipa", "custos", { op: "set_format", tab: 0, col: 1, format: "pct" }));
    fireEvent.click(getByText("A"));
    fireEvent.click(getByText(t("sheet_remove_col")));
    await waitFor(() => expect(calls.edit).toHaveBeenCalledWith("tulipa", "custos", { op: "remove_col", tab: 0, at: 0 }));
    // Row 3 (data row 1): the header "3" opens the row menu.
    fireEvent.click(getAllByText("3")[0]);
    fireEvent.click(getByText(t("sheet_insert_above")));
    await waitFor(() => expect(calls.edit).toHaveBeenCalledWith("tulipa", "custos", { op: "insert_row", tab: 0, at: 1 }));
  });

  it("tabs are renamed on a double click and added with the plus", async () => {
    const { findByText, getByText, getByDisplayValue, getByPlaceholderText } = render(<SheetView space="tulipa" id="custos" />);
    await findByText("set/26");
    fireEvent.doubleClick(getByText("Mensal"));
    const input = getByDisplayValue("Mensal") as HTMLInputElement;
    fireEvent.change(input, { target: { value: "2026" } });
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => expect(calls.edit).toHaveBeenCalledWith("tulipa", "custos", { op: "rename_tab", tab: 0, name: "2026" }));
    fireEvent.click(getByText(t("sheet_add_tab")));
    const name = getByPlaceholderText(t("sheet_tab_name")) as HTMLInputElement;
    fireEvent.change(name, { target: { value: "Notas" } });
    fireEvent.keyDown(name, { key: "Enter" });
    await waitFor(() => expect(calls.edit).toHaveBeenCalledWith("tulipa", "custos", { op: "add_tab", name: "Notas" }));
  });
});
