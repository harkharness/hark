// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render } from "@testing-library/react";
import Demands from "./Demands";
import { t } from "../lib/i18n";
import type { Demand } from "../types";

afterEach(cleanup);

const billing: Demand = { id: "task:Billing v2", source: "board", project: "/p/hark", title: "Billing v2", why: "parou numa permissão", urgency: "needs_you", link: null, at: "2026-11-04T09:50:00-03:00" };
const deck: Demand = { id: "page:landing/deck", source: "artifact", project: "landing", title: "Home redesign v3", why: "pronto para você revisar", urgency: "review", link: "hark://artifacts/landing/deck", at: "2026-11-03T17:40:00-03:00" };
const contador: Demand = { id: "req:r1", source: "request", project: null, title: "responder o contador", why: "você pediu ontem, 18:10", urgency: "reminder", link: null, at: "2026-11-03T18:10:00-03:00" };
const base = { demands: [billing, deck, contador], onGo: () => {}, onDone: () => {}, onLater: () => {} };

describe("Demandas: the list of the day", () => {
  it("shows each demand with its why and its source", () => {
    const { getByText } = render(<Demands {...base} />);
    expect(getByText("Billing v2")).toBeTruthy();
    expect(getByText("parou numa permissão")).toBeTruthy();
    expect(getByText(t("demand_src_artifact"))).toBeTruthy();
    expect(getByText(t("demand_src_request"))).toBeTruthy();
  });

  it("'ir' hands the demand over; 'feito' only exists for a request", () => {
    const onGo = vi.fn();
    const onDone = vi.fn();
    const { getAllByText, getByText } = render(<Demands {...base} onGo={onGo} onDone={onDone} />);
    fireEvent.click(getAllByText(t("demand_go"))[0]);
    expect(onGo).toHaveBeenCalledWith(billing);
    expect(getAllByText(t("demand_done"))).toHaveLength(1);
    fireEvent.click(getByText(t("demand_done")));
    expect(onDone).toHaveBeenCalledWith(contador);
  });

  it("'focar' exists for a board demand only, and hands it over", () => {
    const onFocus = vi.fn();
    const { getAllByText } = render(<Demands {...base} onFocus={onFocus} />);
    const buttons = getAllByText(t("demand_focus"));
    expect(buttons).toHaveLength(1);
    fireEvent.click(buttons[0]);
    expect(onFocus).toHaveBeenCalledWith(billing);
  });

  it("with nothing on the list, it says so", () => {
    const { getByText } = render(<Demands {...base} demands={[]} />);
    expect(getByText(t("demands_empty"))).toBeTruthy();
  });
});
