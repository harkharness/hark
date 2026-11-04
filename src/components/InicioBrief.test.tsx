// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render } from "@testing-library/react";
import InicioBrief from "./InicioBrief";
import { t } from "../lib/i18n";
import type { Demand } from "../types";

afterEach(cleanup);

const billing: Demand = { id: "task:Billing v2", source: "board", project: "/p/hark", title: "Billing v2", why: "parou numa permissão", urgency: "needs_you", link: null, at: "2026-11-04T09:50:00-03:00" };
const base = { userName: "Ana", demands: [billing], onGo: () => {}, onDone: () => {}, onLater: () => {}, onFocus: () => {}, onAll: () => {}, onAsk: () => {} };

describe("the brief that opens Hoje when the day has no chat yet", () => {
  it("greets by name, counts the demands and lists them", () => {
    const { getByText } = render(<InicioBrief {...base} />);
    expect(getByText(/, Ana$/)).toBeTruthy();
    expect(getByText(t("brief_demands_n", { n: 1, you: 1 }))).toBeTruthy();
    expect(getByText("Billing v2")).toBeTruthy();
  });

  it("with nothing on the list it says so, and a hint fills the composer", () => {
    const onAsk = vi.fn();
    const { getByText } = render(<InicioBrief {...base} demands={[]} onAsk={onAsk} />);
    expect(getByText(t("brief_nothing"))).toBeTruthy();
    fireEvent.click(getByText(t("brief_hint_remind")));
    expect(onAsk).toHaveBeenCalledWith(t("brief_hint_remind_text"));
  });
});
