// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render } from "@testing-library/react";
import ModelSelect, { DEFAULT_TIERS } from "./ModelSelect";
import { t } from "../lib/i18n";

afterEach(cleanup);

const GEMINI = {
  light: "gemini-2.5-flash-lite",
  standard: "gemini-2.5-flash",
  heavy: "gemini-2.5-pro",
  max: "gemini-2.5-pro",
};

describe("the model pill in a chat on another agent", () => {
  it("shows the agent's own name for the tier the window default stands for", () => {
    // The window default was picked as "haiku" (claude's light) long
    // before this gemini chat opened. The pill must not read "haiku" over
    // a gemini turn: light, on gemini, is flash-lite.
    const { getByRole } = render(
      <ModelSelect value="haiku" tiers={GEMINI} global={DEFAULT_TIERS} onSelect={() => {}} />,
    );
    expect(getByRole("button").textContent?.toLowerCase()).toContain("gemini-2.5-flash-lite");
  });

  it("marks that tier as the chosen row and picks by tier, not by name", () => {
    const onSelect = vi.fn();
    const { getByRole, getAllByRole } = render(
      <ModelSelect value="haiku" tiers={GEMINI} global={DEFAULT_TIERS} onSelect={onSelect} />,
    );
    fireEvent.click(getByRole("button"));
    const rows = getAllByRole("button").filter((b) => b.className.includes("mm-row"));
    const light = rows.find((r) => r.textContent?.toLowerCase().includes("gemini-2.5-flash-lite"));
    expect(light?.className).toContain(" on");
    const heavy = rows.find((r) => r.textContent?.toLowerCase().includes("gemini-2.5-pro"));
    fireEvent.click(heavy!);
    // A tier travels; the driver says it in each agent's own words.
    expect(onSelect).toHaveBeenCalledWith("heavy");
  });

  it("still prefers the model that actually produced the last turn", () => {
    const { getByRole } = render(
      <ModelSelect value="light" liveModel="gemini-2.5-pro" tiers={GEMINI} global={DEFAULT_TIERS} onSelect={() => {}} />,
    );
    expect(getByRole("button").textContent?.toLowerCase()).toContain("gemini-2.5-pro");
  });

  it("a legacy pick that is a tier key shows the table's name for it", () => {
    const { getByRole } = render(<ModelSelect value="standard" tiers={DEFAULT_TIERS} onSelect={() => {}} />);
    expect(getByRole("button").textContent).toContain("Sonnet");
  });
});

describe("the effort rides in the model control", () => {
  it("shows the effort beside the model, opens its submenu and picks a level", () => {
    const onEffort = vi.fn();
    const { getByRole, getAllByRole, getByText } = render(
      <ModelSelect value="standard" tiers={DEFAULT_TIERS} onSelect={() => {}} effort="high" onSelectEffort={onEffort} />,
    );
    expect(getByRole("button").textContent).toContain("Sonnet");
    expect(getByRole("button").textContent).toContain(t("effort_high"));
    fireEvent.click(getByRole("button"));
    const sub = getAllByRole("button").find((b) => b.className.includes("mm-row sub") || b.className.includes(" sub"));
    expect(sub?.textContent).toContain(t("menu_effort"));
    fireEvent.click(sub!);
    expect(getByText(t("effort_recommended"))).toBeTruthy();
    const max = getAllByRole("button").find((b) => b.className.includes("mm-row") && b.textContent?.includes(t("effort_max")));
    fireEvent.click(max!);
    expect(onEffort).toHaveBeenCalledWith("max");
  });
});
