// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import FolderInput from "./FolderInput";
import { t } from "../lib/i18n";
import type { Completion, FolderHit } from "../lib/folderPick";

afterEach(cleanup);

const hit = (shown: string, more: Partial<FolderHit> = {}): FolderHit => ({
  name: shown.split("/").pop() ?? shown,
  path: shown.replace("~", "/home/u"),
  shown,
  git: false,
  known: false,
  ...more,
});

/** A tiny disk: what each typed text is answered with. */
const DISK: Record<string, Completion> = {
  "": { hits: [hit("~/work/beta", { git: true })], exact: false },
  al: { hits: [hit("~/work/alpha", { git: true }), hit("~/work/alpha-old")], exact: false },
  "~/work/alpha": { hits: [hit("~/work/alpha"), hit("~/work/alpha-old")], exact: true },
  "~/work/alpha/": { hits: [hit("~/work/alpha/src")], exact: true },
  lab: { hits: [hit("~/work/lab", { known: true })], exact: false },
};

const disk = vi.fn(async (typed: string) => DISK[typed] ?? { hits: [], exact: false });

function mount(over: Partial<React.ComponentProps<typeof FolderInput>> = {}) {
  const onPick = vi.fn();
  const onClose = vi.fn();
  const view = render(<FolderInput onPick={onPick} onClose={onClose} complete={disk} {...over} />);
  const input = view.getByRole("combobox") as HTMLInputElement;
  const type = (value: string) => fireEvent.change(input, { target: { value } });
  return { ...view, input, type, onPick, onClose };
}

describe("the add-a-project field offers folders while a path is typed", () => {
  it("lists what the typed text matches and Enter takes the first", async () => {
    const { type, input, findByText, onPick, onClose } = mount();
    type("al");
    await findByText("alpha-old");
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onPick).toHaveBeenCalledWith("/home/u/work/alpha");
    expect(onClose).toHaveBeenCalled();
  });

  it("shows each folder's path under its name", async () => {
    const { type, findByText } = mount();
    type("al");
    expect(await findByText("~/work/alpha-old")).toBeTruthy();
  });

  it("the arrows move the highlight and Enter takes where it rests", async () => {
    const { type, input, findByText, onPick } = mount();
    type("al");
    await findByText("alpha-old");
    fireEvent.keyDown(input, { key: "ArrowDown" });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onPick).toHaveBeenCalledWith("/home/u/work/alpha-old");
  });

  it("a whole folder typed is taken as written, not swapped for a suggestion", async () => {
    const { type, input, findByText, onPick } = mount();
    type("~/work/alpha");
    await findByText("alpha-old");
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onPick).toHaveBeenCalledWith("~/work/alpha");
  });

  it("Tab goes into the folder: the field takes its path and lists what it holds", async () => {
    const { type, input, findByText, onPick } = mount();
    type("al");
    await findByText("alpha-old");
    fireEvent.keyDown(input, { key: "Tab" });
    expect(input.value).toBe("~/work/alpha/");
    expect(await findByText("src")).toBeTruthy();
    expect(onPick).not.toHaveBeenCalled();
  });

  it("a click on a folder takes it", async () => {
    const { type, findByText, onPick } = mount();
    type("al");
    fireEvent.click(await findByText("alpha-old"));
    expect(onPick).toHaveBeenCalledWith("/home/u/work/alpha-old");
  });

  it("the arrow at the end of a row goes into the folder instead of taking it", async () => {
    const { type, input, findByText, getAllByTitle, onPick } = mount();
    type("al");
    await findByText("alpha-old");
    fireEvent.click(getAllByTitle(t("folder_into"))[1]);
    expect(input.value).toBe("~/work/alpha-old/");
    expect(onPick).not.toHaveBeenCalled();
  });

  it("an empty field offers folders but Enter adds nothing", async () => {
    const { input, findByText, onPick, onClose } = mount();
    await findByText("beta");
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onPick).not.toHaveBeenCalled();
    expect(onClose).toHaveBeenCalled();
  });

  it("a folder that is a project already says so", async () => {
    const { type, findByText } = mount();
    type("lab");
    expect(await findByText(t("folder_known"))).toBeTruthy();
  });

  it("text nothing matches says so, and Enter still tries what was typed", async () => {
    const { type, input, findByText, onPick } = mount();
    type("/srv/unknown");
    expect(await findByText(t("folder_none"))).toBeTruthy();
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onPick).toHaveBeenCalledWith("/srv/unknown");
  });

  it("Escape closes without adding", async () => {
    const { type, input, onPick, onClose } = mount();
    type("al");
    fireEvent.keyDown(input, { key: "Escape" });
    expect(onClose).toHaveBeenCalled();
    expect(onPick).not.toHaveBeenCalled();
  });

  it("a slow answer to older text never replaces the answer to the newer", async () => {
    let release: (c: Completion) => void = () => {};
    const slow = vi.fn((typed: string) =>
      typed === "al" ? new Promise<Completion>((r) => (release = r)) : Promise.resolve(DISK[typed] ?? { hits: [], exact: false }),
    );
    const { type, findByText, queryByText } = mount({ complete: slow });
    type("al");
    await waitFor(() => expect(slow).toHaveBeenCalledWith("al"));
    type("lab");
    await findByText("lab");
    release(DISK.al);
    await new Promise((r) => setTimeout(r, 20));
    expect(queryByText("alpha-old")).toBeNull();
  });

  it("a backend that fails leaves a plain field that still takes a path", async () => {
    const broken = vi.fn(async () => {
      throw new Error("no shell");
    });
    const { type, input, onPick } = mount({ complete: broken });
    type("/p/new");
    await waitFor(() => expect(broken).toHaveBeenCalled());
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onPick).toHaveBeenCalledWith("/p/new");
  });
});
