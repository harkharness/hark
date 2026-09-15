// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, waitFor } from "@testing-library/react";
import FileViewer from "./FileViewer";
import type { Project } from "../types";

const files: Record<string, { content: string; truncated: boolean }> = {};
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string, args: { path: string }) => {
    if (cmd === "file_read") return files[args.path];
    return undefined;
  }),
}));

afterEach(cleanup);

const vox: Project = { name: "vox", path: "/p/vox" };
const thirty = Array.from({ length: 30 }, (_, i) => `line ${i + 1}`).join("\n");

describe("the gutter", () => {
  it("numbers every line of a code file while editing, and marks the line the chat pointed at", async () => {
    files["/p/vox/src/lib.rs"] = { content: thirty, truncated: false };
    const { container } = render(
      <FileViewer file={{ abs: "/p/vox/src/lib.rs", rel: "src/lib.rs", line: 21, project: vox }} />,
    );
    await waitFor(() => expect(container.querySelector("textarea")).toBeTruthy());
    const gutter = container.querySelector(".viewer-gutter");
    expect(gutter?.textContent).toContain("30");
    expect(gutter?.textContent).not.toContain("31");
    const mark = container.querySelector(".viewer-line-mark");
    expect(mark?.getAttribute("data-line")).toBe("21");
  });

  it("is there in the read-only view too, when a file is too big to edit", async () => {
    files["/p/vox/big.log"] = { content: thirty, truncated: true };
    const { container, findByText } = render(
      <FileViewer file={{ abs: "/p/vox/big.log", rel: "big.log", project: vox }} />,
    );
    await findByText("truncado");
    expect(container.querySelector("textarea")).toBeNull();
    expect(container.querySelector(".viewer-gutter")?.textContent).toContain("30");
    // Nothing pointed at a line: no mark.
    expect(container.querySelector(".viewer-line-mark")).toBeNull();
  });

  it("stays out of rendered markdown, which has no lines to count", async () => {
    files["/p/vox/README.md"] = { content: "# Hark\n\ntext\n", truncated: false };
    const { container, findByText } = render(
      <FileViewer file={{ abs: "/p/vox/README.md", rel: "README.md", project: vox }} />,
    );
    await findByText("Hark");
    expect(container.querySelector(".viewer-gutter")).toBeNull();
  });
});
