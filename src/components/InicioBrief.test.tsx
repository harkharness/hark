// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render } from "@testing-library/react";
import InicioBrief from "./InicioBrief";
import InicioGlance from "./InicioGlance";
import { t } from "../lib/i18n";
import type { Demand } from "../types";

afterEach(cleanup);

const billing: Demand = { id: "task:Billing v2", source: "board", project: "/p/hark", title: "Billing v2", why: "parou numa permissão", urgency: "needs_you", link: null, at: "2026-11-04T09:50:00-03:00" };
const deck: Demand = { id: "page:landing/deck", source: "artifact", project: "landing", title: "Home redesign v3", why: "pronto para você revisar", urgency: "review", link: "hark://artifacts/landing/deck", at: "2026-11-03T17:40:00-03:00" };

describe("the brief that opens Hoje when the day has no chat yet", () => {
  it("greets by name and counts in one line", () => {
    const { getByText } = render(<InicioBrief userName="Ana" demands={[billing, deck]} />);
    expect(getByText(/, Ana$/)).toBeTruthy();
    expect(getByText(t("brief_sub", { you: 1, review: 1 }))).toBeTruthy();
  });

  it("with nothing on the list it says so", () => {
    const { getByText } = render(<InicioBrief userName="Ana" demands={[]} />);
    expect(getByText(t("brief_nothing"))).toBeTruthy();
  });
});

describe("the glance under the composer", () => {
  const base = { demands: [billing, deck], onGo: () => {}, onAll: () => {}, onArtifacts: () => {}, onBoard: () => {}, onAsk: () => {} };

  it("three boxes, three rows each at most, and a way to the full list", () => {
    const onAll = vi.fn();
    const onGo = vi.fn();
    const { getByText } = render(<InicioGlance {...base} onAll={onAll} onGo={onGo} />);
    expect(getByText(t("brief_box_you"))).toBeTruthy();
    expect(getByText("Billing v2")).toBeTruthy();
    expect(getByText("Home redesign v3")).toBeTruthy();
    fireEvent.click(getByText("Billing v2"));
    expect(onGo).toHaveBeenCalledWith(billing);
    fireEvent.click(getByText(t("brief_see_n", { n: 1 })));
    expect(onAll).toHaveBeenCalled();
  });

  it("a hint fills the composer; with nothing on the list only the hints stay", () => {
    const onAsk = vi.fn();
    const { getByText, queryByText } = render(<InicioGlance {...base} demands={[]} onAsk={onAsk} />);
    fireEvent.click(getByText(t("brief_hint_remind")));
    expect(onAsk).toHaveBeenCalledWith(t("brief_hint_remind_text"));
    expect(queryByText(t("brief_box_you"))).toBeNull();
  });
});
