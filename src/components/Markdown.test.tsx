// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render } from "@testing-library/react";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => undefined) }));
import Markdown from "./Markdown";

afterEach(cleanup);

describe("Markdown: every fenced block can be copied on its own", () => {
  it("gives a plain fence (no language) a copy button that copies only that block", async () => {
    const writeText = vi.fn(async () => undefined);
    Object.assign(navigator, { clipboard: { writeText } });
    const md = "Prompt para colar no outro chat:\n\n```\nAtualização de documentação\n\n## Objetivo\n```\n\ne mais prosa depois.";
    const { getAllByTitle } = render(<Markdown>{md}</Markdown>);
    const [copy] = getAllByTitle(/copiar|copy/);
    fireEvent.click(copy);
    expect(writeText).toHaveBeenCalledWith("Atualização de documentação\n\n## Objetivo");
  });
  it("a shell fence keeps run and insert beside copy; a plain one has copy only", () => {
    const onRun = vi.fn();
    const { container } = render(<Markdown onRun={onRun}>{"```bash\nls -la\n```\n\n```json\n{\"a\": 1}\n```"}</Markdown>);
    const blocks = container.querySelectorAll(".codeblock");
    expect(blocks.length).toBe(2);
    expect(blocks[0].querySelectorAll("button").length).toBe(3);
    expect(blocks[1].querySelectorAll("button").length).toBe(1);
    expect(blocks[0].classList.contains("shell")).toBe(true);
    expect(blocks[1].classList.contains("shell")).toBe(false);
  });
});
