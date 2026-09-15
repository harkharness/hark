// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render } from "@testing-library/react";
import ModeSelect from "./ModeSelect";

afterEach(cleanup);

describe("the bypass row in a chat on an ACP agent", () => {
  it("stays in the menu, disabled, with the reason as its hover, and cannot be picked", () => {
    const onSelect = vi.fn();
    const why = "bypass não atravessa o Gemini CLI";
    const { getByRole, getAllByRole } = render(
      <ModeSelect value="manual" onSelect={onSelect} bypassOff={why} />,
    );
    fireEvent.click(getByRole("button"));
    const rows = getAllByRole("button").filter((b) => b.className.includes("mm-row"));
    const bypass = rows.find((r) => r.className.includes(" off")) as HTMLButtonElement;
    expect(bypass).toBeDefined();
    expect(bypass.disabled).toBe(true);
    expect(bypass.getAttribute("title")).toBe(why);
    expect(rows.filter((r) => r.className.includes(" off"))).toHaveLength(1);
    fireEvent.click(bypass);
    expect(onSelect).not.toHaveBeenCalled();
    // Every other row still picks.
    const plan = rows.find((r) => !r.className.includes(" off") && r.textContent?.includes("plan"));
    if (plan) {
      fireEvent.click(plan);
      expect(onSelect).toHaveBeenCalled();
    }
  });

  it("is an ordinary row on claude", () => {
    const { getByRole, getAllByRole } = render(<ModeSelect value="manual" onSelect={() => {}} />);
    fireEvent.click(getByRole("button"));
    const rows = getAllByRole("button").filter((b) => b.className.includes("mm-row"));
    expect(rows.every((r) => !(r as HTMLButtonElement).disabled)).toBe(true);
  });
});
