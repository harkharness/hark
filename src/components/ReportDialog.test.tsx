// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, waitFor } from "@testing-library/react";

const calls = vi.hoisted(() => ({
  issueUrl: vi.fn(),
  open: vi.fn(async () => undefined),
  clipboard: vi.fn(async () => undefined),
}));

vi.mock("../lib/ipc", () => ({
  reportBundle: async () => ({
    markdown: "Hark 0.7.2 · macOS 15.6 · Apple Silicon\n<details>log</details>",
    version: "0.7.2",
    os: "macOS 15.6",
    arch: "Apple Silicon",
    agents: "claude 2.1.281",
    log_lines: 3,
  }),
  reportGhStatus: async () => false,
  reportIssueUrl: calls.issueUrl,
  reportSave: async () => "/Users/ana/Library/Application Support/hark/reports/r.md",
  reportGhCreate: async () => "https://github.com/harkharness/hark/issues/9",
  openExternal: calls.open,
}));

import ReportDialog from "./ReportDialog";
import { t } from "../lib/i18n";

Object.assign(navigator, { clipboard: { writeText: calls.clipboard } });

afterEach(() => {
  cleanup();
  calls.issueUrl.mockReset();
  calls.open.mockClear();
  calls.clipboard.mockClear();
});

function fill(container: HTMLElement) {
  const [title] = Array.from(container.querySelectorAll("input[type=text], input:not([type])"));
  const [what] = Array.from(container.querySelectorAll("textarea"));
  fireEvent.change(title, { target: { value: "Composer doubles text" } });
  fireEvent.change(what, { target: { value: "Typed a long draft, saw two copies" } });
}

describe("the bug report dialog", () => {
  it("warns that the issue is public only once logs are included", async () => {
    const { container, getByText } = render(<ReportDialog open onClose={() => {}} />);
    await waitFor(() => expect(container.textContent).toContain("0.7.2"));
    expect(container.querySelector(".report-warn")).toBeNull();
    fireEvent.click(container.querySelector("input[type=checkbox]")!);
    expect(container.querySelector(".report-warn")?.textContent).toContain(t("report_public_warning"));
    // What will be sent is on screen before anything is sent.
    expect(getByText(t("report_preview"))).toBeTruthy();
    expect(container.querySelector(".report-preview pre")?.textContent).toContain("Hark 0.7.2");
  });

  it("opens the prefilled form, and carries long logs on the clipboard", async () => {
    calls.issueUrl.mockResolvedValue({
      url: "https://github.com/harkharness/hark/issues/new?template=bug_report.yml&title=x",
      logs_in_url: false,
      logs: "LOGS",
    });
    const onClose = vi.fn();
    const { container, getByText } = render(<ReportDialog open onClose={onClose} />);
    await waitFor(() => expect(container.textContent).toContain("0.7.2"));
    fill(container);
    fireEvent.click(container.querySelector("input[type=checkbox]")!);
    fireEvent.click(getByText(t("report_open_github")));
    await waitFor(() => expect(calls.open).toHaveBeenCalled());
    expect(calls.issueUrl).toHaveBeenCalledWith(
      "Composer doubles text",
      "Typed a long draft, saw two copies",
      "",
      true,
    );
    expect(calls.clipboard).toHaveBeenCalledWith("LOGS");
    expect(calls.open).toHaveBeenCalledWith(
      "https://github.com/harkharness/hark/issues/new?template=bug_report.yml&title=x",
    );
    // The dialog stays to say where the logs went.
    expect(container.textContent).toContain(t("report_pasted_hint"));
    expect(onClose).not.toHaveBeenCalled();
  });

  it("without logs, nothing touches the clipboard and the dialog closes", async () => {
    calls.issueUrl.mockResolvedValue({ url: "https://github.com/x", logs_in_url: false, logs: null });
    const onClose = vi.fn();
    const { container, getByText } = render(<ReportDialog open onClose={onClose} />);
    fill(container);
    fireEvent.click(getByText(t("report_open_github")));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(calls.issueUrl).toHaveBeenCalledWith(expect.any(String), expect.any(String), "", false);
    expect(calls.clipboard).not.toHaveBeenCalled();
  });

  it("needs a title and a description before anything can be filed", () => {
    const { getByText } = render(<ReportDialog open onClose={() => {}} />);
    expect((getByText(t("report_open_github")) as HTMLButtonElement).disabled).toBe(true);
  });
});
