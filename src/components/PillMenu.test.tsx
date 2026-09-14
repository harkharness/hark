// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, fireEvent, render } from "@testing-library/react";
import { PillPopover } from "./PillMenu";

afterEach(cleanup);

describe("a pill for a feature the plugin lacks", () => {
  it("stays in place, disabled, and its hover says why", () => {
    // Observed: in a Gemini chat the mode/model/effort pills took the click
    // and nothing happened — the ACP plugin drops directives. A control
    // that silently does nothing is worse than one that says it cannot.
    const why = "sem suporte no plugin Gemini CLI: modo, modelo e esforço";
    const { getByRole, queryByText } = render(
      <PillPopover label="auto" title="escopo" disabled={why}>
        {() => <div>menu aberto</div>}
      </PillPopover>,
    );
    const pill = getByRole("button") as HTMLButtonElement;
    expect(pill.disabled).toBe(true);
    expect(pill.getAttribute("title")).toBe(why);
    expect(pill.className).toContain("unsupported");
    fireEvent.click(pill);
    expect(queryByText("menu aberto")).toBeNull();
  });

  it("without a reason it is the ordinary pill", () => {
    const { getByRole, queryByText } = render(
      <PillPopover label="auto" title="escopo">
        {() => <div>menu aberto</div>}
      </PillPopover>,
    );
    const pill = getByRole("button") as HTMLButtonElement;
    expect(pill.disabled).toBe(false);
    expect(pill.getAttribute("title")).toBe("escopo");
    fireEvent.click(pill);
    expect(queryByText("menu aberto")).not.toBeNull();
  });
});
