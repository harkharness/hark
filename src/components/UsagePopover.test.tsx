// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render } from "@testing-library/react";
import { t } from "../lib/i18n";

vi.mock("../lib/ipc", () => ({ subscriptionLimits: async () => null }));

import UsagePopover, { limitLabel, resetLabel } from "./UsagePopover";

afterEach(cleanup);

describe("the usage popover of the cost pill", () => {
  it("shows one bar per window with its percentage and reset, and a way to the costs page", () => {
    const onAll = vi.fn();
    const soon = new Date(Date.now() + 3600_000).toISOString();
    const { getByText, container } = render(
      <UsagePopover
        limits={{ context_used: null, context_tokens: null, context_window: null, model: null, session_id: null, limits: [{ key: "five_hour", used: 0.74, resets_at: soon }, { key: "seven_day", used: 0.3 }, { key: "seven_day_opus", used: 0.53 }] }}
        onAll={onAll}
        onClose={() => {}}
      />,
    );
    expect(getByText(t("usage_5h"))).toBeTruthy();
    expect(getByText("74%")).toBeTruthy();
    expect(getByText(t("usage_week"))).toBeTruthy();
    expect(getByText(t("usage_week_model", { m: "opus" }))).toBeTruthy();
    expect(container.querySelectorAll(".usage-bar i").length).toBe(3);
    fireEvent.click(getByText(t("usage_all")));
    expect(onAll).toHaveBeenCalled();
  });
  it("says where the bridge is when there is no limit data", () => {
    const { getByText } = render(<UsagePopover limits={null} onAll={() => {}} onClose={() => {}} />);
    expect(getByText(t("usage_none"))).toBeTruthy();
  });
  it("names the windows and the reset moment", () => {
    expect(limitLabel("five_hour")).toBe(t("usage_5h"));
    expect(limitLabel("seven_day_fable")).toBe(t("usage_week_model", { m: "fable" }));
    const now = new Date("2026-11-12T10:00:00");
    expect(resetLabel(new Date("2026-11-12T13:20:00").toISOString(), now)).toContain("13:20");
    expect(resetLabel(undefined, now)).toBeUndefined();
  });
});
