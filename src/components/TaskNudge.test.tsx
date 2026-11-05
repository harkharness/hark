// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render } from "@testing-library/react";
import TaskNudge from "./TaskNudge";
import { t } from "../lib/i18n";

afterEach(cleanup);

describe("the offer of a task", () => {
  it("names the gap and hands over either verb", () => {
    const onRegister = vi.fn();
    const onLater = vi.fn();
    const { getByText } = render(<TaskNudge onRegister={onRegister} onLater={onLater} />);
    expect(getByText(t("nudge_no_task"))).toBeTruthy();
    fireEvent.click(getByText(t("nudge_register")));
    expect(onRegister).toHaveBeenCalled();
    fireEvent.click(getByText(t("nudge_later")));
    expect(onLater).toHaveBeenCalled();
  });
});
