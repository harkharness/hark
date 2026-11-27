// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import SpaceFolder from "./SpaceFolder";
import { t } from "../lib/i18n";
import type { ArtifactSpace } from "../types";

afterEach(cleanup);

const space = (more: Partial<ArtifactSpace> = {}): ArtifactSpace => ({
  name: "tulipafin",
  project: "/Users/ana/code/tulipafin",
  pages: 4,
  folder: "/Users/ana/.hark/artifacts/tulipafin",
  in_project: false,
  left_behind: 0,
  ...more,
});

const inRepo = space({ folder: "/Users/ana/code/tulipafin/docs/hark", in_project: true });

describe("where a space keeps its pages, and moving them to the repository", () => {
  it("says the folder the pages are in, home as ~", () => {
    const { getByText } = render(<SpaceFolder space={space()} onMove={vi.fn()} />);
    expect(getByText("~/.hark/artifacts/tulipafin")).toBeTruthy();
  });

  it("a project's pages on this machine offer to go to the repository", () => {
    const { getByText } = render(<SpaceFolder space={space()} onMove={vi.fn()} />);
    expect(getByText(t("space_move"))).toBeTruthy();
  });

  it("the personal space has no repository to go to", () => {
    const { queryByText } = render(<SpaceFolder space={space({ name: "pessoal", project: null })} onMove={vi.fn()} />);
    expect(queryByText(t("space_move"))).toBeNull();
  });

  it("asks before writing into the repository, naming the folder it will make", () => {
    const onMove = vi.fn(async () => 4);
    const { getByText } = render(<SpaceFolder space={space()} onMove={onMove} />);
    fireEvent.click(getByText(t("space_move")));
    expect(onMove).not.toHaveBeenCalled();
    expect(getByText(t("space_move_ask", { dir: "~/code/tulipafin/docs/hark" }))).toBeTruthy();
  });

  it("moves only once the person confirms, and says how many pages went", async () => {
    const onMove = vi.fn(async () => 4);
    const { getByText, findByText } = render(<SpaceFolder space={space()} onMove={onMove} />);
    fireEvent.click(getByText(t("space_move")));
    fireEvent.click(getByText(t("space_move_yes")));
    await waitFor(() => expect(onMove).toHaveBeenCalledWith("tulipafin"));
    expect(await findByText(t("space_moved", { n: 4 }))).toBeTruthy();
  });

  it("cancelling leaves everything where it was", () => {
    const onMove = vi.fn(async () => 4);
    const { getByText, queryByText } = render(<SpaceFolder space={space()} onMove={onMove} />);
    fireEvent.click(getByText(t("space_move")));
    fireEvent.click(getByText(t("space_move_no")));
    expect(onMove).not.toHaveBeenCalled();
    expect(queryByText(t("space_move_yes"))).toBeNull();
    expect(getByText(t("space_move"))).toBeTruthy();
  });

  it("a move that is refused shows why and can be tried again", async () => {
    const onMove = vi.fn(async () => {
      throw new Error("home.md: the project already has another page there");
    });
    const { getByText, findByText } = render(<SpaceFolder space={space()} onMove={onMove} />);
    fireEvent.click(getByText(t("space_move")));
    fireEvent.click(getByText(t("space_move_yes")));
    expect(await findByText(/home\.md/)).toBeTruthy();
    expect(getByText(t("space_move"))).toBeTruthy();
  });

  it("pages that live in the repository say so, and offer nothing to move", () => {
    const { getByText, queryByText } = render(<SpaceFolder space={inRepo} onMove={vi.fn()} />);
    expect(getByText("~/code/tulipafin/docs/hark")).toBeTruthy();
    expect(getByText(t("space_in_repo"))).toBeTruthy();
    expect(queryByText(t("space_move"))).toBeNull();
  });

  it("pages left on this machine after the folder came with a pull can be brought over", async () => {
    const onMove = vi.fn(async () => 2);
    const { getByText } = render(<SpaceFolder space={{ ...inRepo, left_behind: 2 }} onMove={onMove} />);
    fireEvent.click(getByText(t("space_bring", { n: 2 })));
    fireEvent.click(getByText(t("space_move_yes")));
    await waitFor(() => expect(onMove).toHaveBeenCalledWith("tulipafin"));
  });
});
