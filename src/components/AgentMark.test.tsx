// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render } from "@testing-library/react";
import AgentMark from "./AgentMark";

afterEach(cleanup);

describe("the mark that identifies an agent", () => {
  it("draws the vendor's glyph, with no letter in it", () => {
    const { container } = render(<AgentMark vendor="Anthropic" id="claude" state="on" />);
    expect(container.querySelector("svg")).toBeTruthy();
    expect(container.textContent).toBe("");
  });

  it("falls back to the id's letter when the agent has no vendor of ours", () => {
    const { container } = render(<AgentMark vendor="" id="qwen-local" state="idle" />);
    expect(container.querySelector("svg")).toBeNull();
    expect(container.textContent).toBe("Q");
  });

  it("gives a twin its own color, so two claude entries are not the same square", () => {
    const { container: a } = render(<AgentMark vendor="" id="claude-gw" state="idle" />);
    const { container: b } = render(<AgentMark vendor="" id="qwen-local" state="idle" />);
    const hue = (c: HTMLElement) => (c.firstElementChild as HTMLElement).style.getPropertyValue("--mark");
    expect(hue(a)).not.toBe(hue(b));
  });

  it("carries the state, so the catalog needs no badge for it", () => {
    const { container } = render(<AgentMark vendor="Google" id="gemini" state="missing" />);
    expect(container.firstElementChild?.getAttribute("data-state")).toBe("missing");
  });
});
