// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render } from "@testing-library/react";
import FilesWindow from "./FilesWindow";
import { t } from "../lib/i18n";
import type { OpenFile, Project } from "../types";

vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === "project_files") return ["README.md", "src/lib.rs"];
    if (cmd === "file_read") return { content: "# Hark\n", truncated: false };
    return undefined;
  }),
}));

afterEach(cleanup);

const vox: Project = { name: "vox", path: "/p/vox" };
const noop = () => {};

describe("the files window with nothing open", () => {
  it("shows the tree beside an empty state that points at the tree", () => {
    const { getByText, getByPlaceholderText } = render(
      <FilesWindow projects={[vox]} files={[]} active={0} treeOpen onOpen={noop} onDirty={noop} />,
    );
    expect(getByText(t("files_empty_title"))).toBeTruthy();
    expect(getByText(t("files_empty_tree"))).toBeTruthy();
    expect(getByPlaceholderText(t("filespanel_ph"))).toBeTruthy();
  });

  it("with the tree collapsed, hides it and tells how to get it back", () => {
    const { getByText, queryByPlaceholderText } = render(
      <FilesWindow projects={[vox]} files={[]} active={0} treeOpen={false} onOpen={noop} onDirty={noop} />,
    );
    expect(getByText(t("files_empty_closed"))).toBeTruthy();
    expect(queryByPlaceholderText(t("filespanel_ph"))).toBeNull();
  });
});

describe("the tree and the tabs are one window", () => {
  it("a file picked in the tree is opened as a tab", async () => {
    const onOpen = vi.fn();
    const { findByText } = render(
      <FilesWindow projects={[vox]} files={[]} active={0} treeOpen onOpen={onOpen} onDirty={noop} />,
    );
    fireEvent.click(await findByText("README.md"));
    expect(onOpen).toHaveBeenCalledWith({ abs: "/p/vox/README.md", rel: "README.md", project: vox });
  });

  it("the open file is revealed and marked in the tree", async () => {
    const open: OpenFile = { abs: "/p/vox/src/lib.rs", rel: "src/lib.rs", project: vox };
    const { findByText } = render(
      <FilesWindow projects={[vox]} files={[open]} active={0} treeOpen onOpen={noop} onDirty={noop} />,
    );
    // `src` is closed by default: the open file forces it open and paints
    // its own row like the active tab.
    const row = await findByText("lib.rs");
    expect(row.className).toContain("tree-row");
    expect(row.className).toContain("on");
  });
});
