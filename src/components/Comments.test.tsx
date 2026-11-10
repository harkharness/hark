// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import type { PageComment } from "../types";

const calls = vi.hoisted(() => ({
  add: vi.fn(async (_s: string, _i: string, text: string, anchor: unknown) => ({ id: "c9", at: "2026-11-10T20:00:00Z", text, anchor, resolved: false })),
  send: vi.fn(async () => "Na página hark://artifacts/tulipa/custos, em célula B4: tira o legado"),
  resolve: vi.fn(async () => undefined),
  remove: vi.fn(async () => undefined),
}));
vi.mock("../lib/ipc", () => ({
  artifactCommentAdd: calls.add,
  artifactCommentSend: calls.send,
  artifactCommentResolve: calls.resolve,
  artifactCommentRemove: calls.remove,
}));

import Comments from "./Comments";
import { t } from "../lib/i18n";

afterEach(cleanup);

const comments: PageComment[] = [
  { id: "c1", at: "2026-11-10T18:40:00Z", text: "@hark tira o legado", anchor: { kind: "cell", tab: 0, row: 2, col: 1, tab_name: "Mensal" }, resolved: false, sent: { at: "2026-11-10T18:40:30Z", task: "Billing" }, answered_at: "2026-11-10T18:42:00Z" },
  { id: "c2", at: "2026-11-10T19:02:00Z", text: "conferir outubro", anchor: { kind: "page" }, resolved: true },
];
const dests = [{ kind: "hark" as const }, { kind: "origin" as const, title: "Billing", sessionId: "s-1" }, { kind: "new" as const, project: "tulipa" }];

describe("the comments column", () => {
  it("lists the comments with their anchors and the answered badge", () => {
    const { getByText } = render(<Comments space="tulipa" id="custos" comments={comments} anchor={{ kind: "page" }} destinations={dests} onChanged={() => {}} onSend={() => {}} />);
    expect(getByText("@hark tira o legado")).toBeTruthy();
    expect(getByText(t("cm_answered"))).toBeTruthy();
    expect(getByText(new RegExp(t("cm_anchor_cell", { c: "B4", t: "Mensal" })))).toBeTruthy();
    expect(getByText(t("cm_reopen"))).toBeTruthy();
  });

  it("a mention checks 'send to hark', the origin chat is the default, and the request leaves", async () => {
    const onSend = vi.fn();
    const onChanged = vi.fn();
    const { getByPlaceholderText, getByLabelText, getByText, getByDisplayValue } = render(
      <Comments space="tulipa" id="custos" comments={[]} anchor={{ kind: "cell", tab: 0, row: 2, col: 1, tab_name: "Mensal" }} destinations={dests} onChanged={onChanged} onSend={onSend} />,
    );
    const box = getByLabelText(t("cm_send")) as HTMLInputElement;
    expect(box.checked).toBe(false);
    fireEvent.change(getByPlaceholderText(t("cm_ph")), { target: { value: "@hark tira o legado" } });
    expect(box.checked).toBe(true);
    expect(getByDisplayValue(t("cm_dest_origin", { t: "Billing" }))).toBeTruthy();
    fireEvent.click(getByText(t("cm_submit")));
    await waitFor(() => expect(calls.add).toHaveBeenCalledWith("tulipa", "custos", "@hark tira o legado", { kind: "cell", tab: 0, row: 2, col: 1, tab_name: "Mensal" }));
    await waitFor(() => expect(calls.send).toHaveBeenCalledWith("tulipa", "custos", "c9", "s-1", "Billing"));
    await waitFor(() => expect(onSend).toHaveBeenCalledWith(dests[1], expect.stringContaining("hark://artifacts/tulipa/custos")));
    expect(onChanged).toHaveBeenCalled();
  });

  it("a plain comment is kept and goes nowhere", async () => {
    const onSend = vi.fn();
    const { getByPlaceholderText, getByText } = render(<Comments space="tulipa" id="custos" comments={[]} anchor={{ kind: "page" }} destinations={dests} onChanged={() => {}} onSend={onSend} />);
    fireEvent.change(getByPlaceholderText(t("cm_ph")), { target: { value: "conferir outubro" } });
    fireEvent.click(getByText(t("cm_submit")));
    await waitFor(() => expect(calls.add).toHaveBeenCalledWith("tulipa", "custos", "conferir outubro", { kind: "page" }));
    expect(onSend).not.toHaveBeenCalled();
  });
});
