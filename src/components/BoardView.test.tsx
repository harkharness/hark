// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import type { BoardView as View } from "../types";

const calls = vi.hoisted(() => ({
  edit: vi.fn(async () => undefined),
  accept: vi.fn(async () => undefined),
  selection: vi.fn(async () => "- [Quick-add] Atalho (Ana)\n"),
}));

const view: View = {
  board: {
    title: "Plano",
    groups: [{ id: "g1", title: "Quick-add", x: 24, y: 24, w: 338, h: 200 }],
    notes: [
      { id: "n1", text: "Atalho no celular", x: 48, y: 60, color: "yellow", owner: "Ana", date: "sex", group: "g1" },
      { id: "n2", text: "Entra na 0.11?", x: 420, y: 300, color: "pink" },
    ],
    timeline: [{ date: "2026-11-12", title: "beta", owner: "Ana" }],
  },
  changed: ["n2"],
  versions: 2,
  raw: "{}",
};

vi.mock("../lib/ipc", () => ({
  boardView: async () => view,
  boardEdit: calls.edit,
  boardSelection: calls.selection,
  artifactAccept: calls.accept,
}));

import BoardView, { within } from "./BoardView";
import { t } from "../lib/i18n";

afterEach(cleanup);

describe("the whiteboard viewer", () => {
  it("draws the groups, the notes with their marks, and the timeline", async () => {
    const { findByText, getByText, container } = render(<BoardView space="tulipa" id="plano" />);
    expect(await findByText("Atalho no celular")).toBeTruthy();
    expect(getByText("Quick-add")).toBeTruthy();
    expect(getByText("Entra na 0.11?").closest(".board-note")!.className).toContain("changed");
    expect(getByText("Entra na 0.11?").closest(".board-note")!.className).toContain("pink");
    expect(getByText(t("board_changed_n", { n: 1 }))).toBeTruthy();
    expect(getByText("beta")).toBeTruthy();
    expect(container.querySelectorAll(".board-note").length).toBe(2);
  });

  it("a double click edits a note's text and writes it on blur", async () => {
    const { findByText, getByDisplayValue } = render(<BoardView space="tulipa" id="plano" />);
    const note = await findByText("Atalho no celular");
    fireEvent.doubleClick(note);
    const area = getByDisplayValue("Atalho no celular") as HTMLTextAreaElement;
    fireEvent.change(area, { target: { value: "Atalho no celular, um toque" } });
    fireEvent.blur(area);
    await waitFor(() => expect(calls.edit).toHaveBeenCalledWith("tulipa", "plano", { op: "text", id: "n1", text: "Atalho no celular, um toque" }));
  });

  it("a selected note can be asked about: the excerpt goes to the chat", async () => {
    const onAsk = vi.fn();
    const { findByText, getAllByText } = render(<BoardView space="tulipa" id="plano" onAsk={onAsk} />);
    const note = await findByText("Atalho no celular");
    fireEvent.click(note);
    fireEvent.click(getAllByText(t("board_ask"))[0]);
    await waitFor(() => expect(calls.selection).toHaveBeenCalledWith("tulipa", "plano", ["n1"]));
    await waitFor(() => expect(onAsk).toHaveBeenCalledWith("- [Quick-add] Atalho (Ana)"));
  });

  it("accepting takes the agent's notes", async () => {
    const { findByText, getByText } = render(<BoardView space="tulipa" id="plano" />);
    await findByText("Atalho no celular");
    fireEvent.click(getByText(t("sheet_accept")));
    await waitFor(() => expect(calls.accept).toHaveBeenCalledWith("tulipa", "plano"));
  });

  it("making a document or a deck goes to a chat with the instruction", async () => {
    const onAsk = vi.fn();
    const { findByText, getByText } = render(<BoardView space="tulipa" id="plano" onAsk={onAsk} />);
    await findByText("Atalho no celular");
    fireEvent.click(getByText(t("board_to_doc")));
    expect(onAsk).toHaveBeenCalledWith(t("board_to_doc_text"));
    fireEvent.click(getByText(t("board_to_deck")));
    expect(onAsk).toHaveBeenCalledWith(t("board_to_deck_text"));
  });

  it("the lasso catches the notes inside it", () => {
    expect(within(view.board.notes, 0, 0, 400, 250)).toEqual(["n1"]);
    expect(within(view.board.notes, 0, 0, 1000, 1000)).toEqual(["n1", "n2"]);
    expect(within(view.board.notes, 0, 0, 10, 10)).toEqual([]);
  });
});
