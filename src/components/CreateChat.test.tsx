// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render } from "@testing-library/react";
import CreateChat from "./CreateChat";
import { t } from "../lib/i18n";
import type { Project } from "../types";

afterEach(cleanup);

const vox: Project = { name: "vox", path: "/p/vox" };
const lab: Project = { name: "lab", path: "/p/lab" };
const base = { projects: [vox, lab], onPick: () => {}, onAddProject: () => {}, onImport: () => {} };

describe("Criar: the project comes first", () => {
  it("lists every project and picking one starts the chat there", () => {
    const onPick = vi.fn();
    const { getByText } = render(<CreateChat {...base} onPick={onPick} />);
    fireEvent.click(getByText("lab"));
    expect(onPick).toHaveBeenCalledWith(lab);
  });

  it("'Escolher pasta…' asks for a path and registers it on Enter", () => {
    const onAddProject = vi.fn();
    const { getByText, getByPlaceholderText } = render(<CreateChat {...base} onAddProject={onAddProject} />);
    fireEvent.click(getByText(t("create_pick_folder")));
    const input = getByPlaceholderText("~/Projects/…");
    fireEvent.change(input, { target: { value: "/p/new" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onAddProject).toHaveBeenCalledWith("/p/new");
  });

  it("importing the CLI sessions is a door of its own", () => {
    const onImport = vi.fn();
    const { getByText } = render(<CreateChat {...base} onImport={onImport} />);
    fireEvent.click(getByText(t("create_import")));
    expect(onImport).toHaveBeenCalled();
  });

  it("with no project registered, it says so and offers the folder", () => {
    const { getByText } = render(<CreateChat {...base} projects={[]} />);
    expect(getByText(t("create_no_projects"))).toBeTruthy();
    expect(getByText(t("create_pick_folder"))).toBeTruthy();
  });
});
