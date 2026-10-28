import { describe, expect, it } from "vitest";
import { pageHtml, transcriptMarkdown } from "./export";
import type { TranscriptEntry } from "../types";

const e = (role: TranscriptEntry["role"], text: string, tool?: string): TranscriptEntry => ({ ts: "2026-10-28T10:00:00Z", role, text, tool, is_error: false });

describe("transcriptMarkdown: a chat as a document, tools folded", () => {
  it("heads with the title, prose as prose, tool calls inside details", () => {
    const md = transcriptMarkdown("Billing endpoint v2", [
      e("user", "roda os testes"),
      e("tool_use", '{"command":"cargo test"}', "Bash"),
      e("tool_result", "ok 12 passed"),
      e("assistant", "Passaram os 12."),
    ]);
    expect(md.startsWith("# Billing endpoint v2\n")).toBe(true);
    expect(md).toContain("**Você:** roda os testes");
    expect(md).toContain("<details><summary>Bash</summary>");
    expect(md).toContain("cargo test");
    expect(md).toContain("ok 12 passed");
    expect(md).toContain("Passaram os 12.");
    expect(md.indexOf("roda os testes")).toBeLessThan(md.indexOf("Passaram"));
  });
  it("skips compactions and fences a result that looks like code", () => {
    const md = transcriptMarkdown("t", [e("compaction", "…"), e("tool_result", "line1\nline2")]);
    expect(md).not.toContain("…");
    expect(md).toContain("```\nline1\nline2\n```");
  });
});

describe("pageHtml: one file that opens anywhere", () => {
  it("wraps the body in a document with the title and its own styles", () => {
    const html = pageHtml("ADR 004", "<h1>ADR 004</h1><p>x</p>");
    expect(html.startsWith("<!doctype html>")).toBe(true);
    expect(html).toContain("<title>ADR 004</title>");
    expect(html).toContain("<style>");
    expect(html).toContain("<h1>ADR 004</h1><p>x</p>");
    expect(pageHtml("<b>", "")).toContain("<title>&lt;b&gt;</title>");
  });
});
