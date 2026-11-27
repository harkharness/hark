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
  "": { hits: [hit("~/work/beta", { git: true })], folder: null },
  al: { hits: [hit("~/work/alpha", { git: true }), hit("~/work/alpha-old")], folder: null },
  "~/work/alpha": { hits: [hit("~/work/alpha"), hit("~/work/alpha-old")], folder: hit("~/work/alpha") },
  "~/work/alpha/": { hits: [hit("~/work/alpha/src")], folder: hit("~/work/alpha") },
  "~/work/alpha-old/": { hits: [], folder: hit("~/work/alpha-old") },
  "dotfiles/": { hits: [hit("~/work/dotfiles/bin")], folder: hit("~/work/dotfiles") },
  lab: { hits: [hit("~/work/lab", { known: true })], folder: null },
};

const disk = vi.fn(async (typed: string) => DISK[typed] ?? { hits: [], folder: null });

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

  it("shows each folder's path beside its name", async () => {
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

  it("a whole folder typed is the one taken, not swapped for a suggestion", async () => {
    const { type, input, findByText, onPick } = mount();
    type("~/work/alpha");
    await findByText("alpha-old");
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onPick).toHaveBeenCalledWith("/home/u/work/alpha");
  });

  it("a name with a separator after it is the folder that name was found as", async () => {
    const { type, input, findByText, onPick } = mount();
    type("dotfiles/");
    // What it holds is listed, and Enter takes the folder itself.
    await findByText("bin");
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onPick).toHaveBeenCalledWith("/home/u/work/dotfiles");
  });

  it("says which folder Enter adds once the text is one", async () => {
    const { type, findByText } = mount();
    type("dotfiles/");
    expect(await findByText(t("folder_keys_add", { path: "~/work/dotfiles" }))).toBeTruthy();
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

  it("a click fills the field with the folder and lists what it holds; only Enter adds", async () => {
    const { type, input, findByText, onPick, onClose } = mount();
    type("al");
    fireEvent.click(await findByText("alpha"));
    expect(input.value).toBe("~/work/alpha/");
    expect(await findByText("src")).toBeTruthy();
    expect(onPick).not.toHaveBeenCalled();
    expect(onClose).not.toHaveBeenCalled();
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onPick).toHaveBeenCalledWith("/home/u/work/alpha");
  });

  it("a folder with nothing inside still says what Enter adds, not that nothing matched", async () => {
    const { type, findByText, queryByText } = mount();
    type("al");
    fireEvent.click(await findByText("alpha-old"));
    expect(await findByText(t("folder_add_only", { path: "~/work/alpha-old" }))).toBeTruthy();
    expect(queryByText(t("folder_none"))).toBeNull();
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
      typed === "al" ? new Promise<Completion>((r) => (release = r)) : Promise.resolve(DISK[typed] ?? { hits: [], folder: null }),
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

  it("the list floats over the page: nothing is added under the field, so it never moves", async () => {
    const { type, findByText, container } = mount();
    const before = container.innerHTML.length;
    type("al");
    const row = await findByText("alpha-old");
    expect(container.contains(row)).toBe(false);
    expect(document.body.querySelector(".folder-pop")?.contains(row)).toBe(true);
    // Only the field's own value changed inside the component's box.
    expect(container.querySelectorAll("*").length).toBe(2);
    expect(before).toBeGreaterThan(0);
  });

  it("the folders of the text before stay up while the next ones load: no blink per key", async () => {
    let release: (c: Completion) => void = () => {};
    const slow = vi.fn((typed: string) =>
      typed === "alp" ? new Promise<Completion>((r) => (release = r)) : Promise.resolve(DISK[typed] ?? { hits: [], folder: null }),
    );
    const { type, findByText, queryByText } = mount({ complete: slow });
    type("al");
    await findByText("alpha-old");
    type("alp");
    await waitFor(() => expect(slow).toHaveBeenCalledWith("alp"));
    expect(queryByText("alpha-old")).not.toBeNull();
    release({ hits: [hit("~/work/alpha")], folder: null });
    await waitFor(() => expect(queryByText("alpha-old")).toBeNull());
  });

  it("Enter before the folders arrive waits for them instead of taking the list before", async () => {
    let release: (c: Completion) => void = () => {};
    const slow = vi.fn((typed: string) =>
      typed === "lab" ? new Promise<Completion>((r) => (release = r)) : Promise.resolve(DISK[typed] ?? { hits: [], folder: null }),
    );
    const { type, input, findByText, onPick, onClose } = mount({ complete: slow });
    type("al");
    await findByText("alpha-old");
    type("lab");
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onPick).not.toHaveBeenCalled();
    await waitFor(() => expect(slow).toHaveBeenCalledWith("lab"));
    release(DISK.lab);
    await waitFor(() => expect(onPick).toHaveBeenCalledWith("/home/u/work/lab"));
    expect(onClose).toHaveBeenCalled();
  });

  it("a backend that fails leaves a plain field that still takes a path", async () => {
    const broken = vi.fn(async () => {
      throw new Error("no shell");
    });
    const { type, input, onPick } = mount({ complete: broken });
    type("/p/new");
    await waitFor(() => expect(broken).toHaveBeenCalled());
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => expect(onPick).toHaveBeenCalledWith("/p/new"));
  });
});
