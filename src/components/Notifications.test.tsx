// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render } from "@testing-library/react";
import Notifications from "./Notifications";
import { t } from "../lib/i18n";
import type { Notification } from "../types";

afterEach(cleanup);

const redesign: Notification = { id: "turn:chat:Redesign:1", kind: "turn_ended", target: { kind: "chat", title: "Redesign da home", session_id: "s2" }, project: "/p/landing", text: "terminou o turno · deck v3 pronto", at: "2026-11-04T11:03:00-03:00" };
const billing: Notification = { id: "perm:chat:Billing:1", kind: "permission_asked", target: { kind: "chat", title: "Billing v2", session_id: null }, project: "/p/hark", text: "pediu permissão: Bash", at: "2026-11-04T09:14:00-03:00", read_at: "2026-11-04T09:20:00-03:00" };
const base = { rows: [redesign, billing], onGo: () => {}, onFocus: () => {}, onReadAll: () => {}, onHistory: () => {}, onSeen: () => {} };

describe("the bell's panel", () => {
  it("lists the unread first, the read under their own heading", () => {
    const { getByText } = render(<Notifications {...base} />);
    expect(getByText("Redesign da home")).toBeTruthy();
    expect(getByText(t("notif_read_heading", { n: 10 }))).toBeTruthy();
    expect(getByText(t("notif_unread_n", { n: 1 }))).toBeTruthy();
  });

  it("'ir' hands the row over, 'focar' only exists for a chat, and the footer reads all", () => {
    const onGo = vi.fn();
    const onFocus = vi.fn();
    const onReadAll = vi.fn();
    const { getAllByText, getByText } = render(<Notifications {...base} onGo={onGo} onFocus={onFocus} onReadAll={onReadAll} />);
    fireEvent.click(getAllByText(t("demand_go"))[0]);
    expect(onGo).toHaveBeenCalledWith(redesign);
    fireEvent.click(getAllByText(t("demand_focus"))[0]);
    expect(onFocus).toHaveBeenCalledWith(redesign);
    fireEvent.click(getByText(t("notif_read_all")));
    expect(onReadAll).toHaveBeenCalled();
  });

  it("with nothing behind the bell, it says so and still offers the history", () => {
    const onHistory = vi.fn();
    const { getByText } = render(<Notifications {...base} rows={[]} onHistory={onHistory} />);
    expect(getByText(t("notif_empty"))).toBeTruthy();
    fireEvent.click(getByText(t("notif_history")));
    expect(onHistory).toHaveBeenCalled();
  });
});
