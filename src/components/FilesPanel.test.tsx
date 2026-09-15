// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render } from "@testing-library/react";
import FilesPanel from "./FilesPanel";
import { t } from "../lib/i18n";
import type { Project } from "../types";

vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string, args: { query?: string }) => {
    if (cmd !== "project_files") return undefined;
    // The filter is name-fuzzy: the same command answers the tree (empty
    // query) and the hits.
    return args.query ? ["install.sh", "crates/hark-plugin-acp/fixtures/initialize.json"] : ["README.md"];
  }),
}));

afterEach(cleanup);

const vox: Project = { name: "vox", path: "/p/vox" };

describe("the filter in the tree column", () => {
  it("shows a hit as its file name and, dimmed, the folder it lives in", async () => {
    const { getByPlaceholderText, findByText, getByText } = render(
      <FilesPanel projects={[vox]} onOpen={() => {}} />,
    );
    fireEvent.change(getByPlaceholderText(t("filespanel_ph")), { target: { value: "in" } });
    expect(await findByText("initialize.json")).toBeTruthy();
    expect(getByText("vox/")).toBeTruthy();
    expect(getByText("vox/crates/hark-plugin-acp/fixtures/")).toBeTruthy();
  });

  it("arrows move the selection and Enter opens it, like the palette", async () => {
    const onOpen = vi.fn();
    const { getByPlaceholderText, findByText } = render(<FilesPanel projects={[vox]} onOpen={onOpen} />);
    const input = getByPlaceholderText(t("filespanel_ph"));
    fireEvent.change(input, { target: { value: "in" } });
    await findByText("initialize.json");
    fireEvent.keyDown(input, { key: "ArrowDown" });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onOpen).toHaveBeenCalledWith({
      abs: "/p/vox/crates/hark-plugin-acp/fixtures/initialize.json",
      rel: "crates/hark-plugin-acp/fixtures/initialize.json",
      project: vox,
    });
  });

  it("the × clears the filter and brings the trees back", async () => {
    const { getByPlaceholderText, findByText, getByTitle, queryByText } = render(
      <FilesPanel projects={[vox]} onOpen={() => {}} />,
    );
    const input = getByPlaceholderText(t("filespanel_ph")) as HTMLInputElement;
    fireEvent.change(input, { target: { value: "in" } });
    await findByText("install.sh");
    fireEvent.click(getByTitle(t("filespanel_clear")));
    expect(input.value).toBe("");
    expect(queryByText("install.sh")).toBeNull();
    expect(await findByText("README.md")).toBeTruthy();
  });
});
