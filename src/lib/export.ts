// What leaves the app as a file: a chat as markdown, a page as one html
// document. Pure: the caller writes the file.
import type { TranscriptEntry } from "../types";

const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

/** A tool's input as the reader wants it: the command of a shell call,
 *  the path of a file call, else the raw payload. */
function toolLine(input: string): string {
  try {
    const parsed = JSON.parse(input) as Record<string, unknown>;
    const pick = parsed.command ?? parsed.file_path ?? parsed.path ?? parsed.pattern ?? parsed.query;
    if (typeof pick === "string") return pick;
  } catch {
    /* not json: the payload is the line */
  }
  return input;
}

/** The chat as a document: prose as prose, every tool call and its
 *  result folded, compactions left out. */
export function transcriptMarkdown(title: string, entries: TranscriptEntry[]): string {
  const out: string[] = [`# ${title}`, ""];
  for (const en of entries) {
    switch (en.role) {
      case "user":
        out.push(`**Você:** ${en.text}`, "");
        break;
      case "assistant":
        out.push(en.text, "");
        break;
      case "tool_use":
        out.push(`<details><summary>${esc(en.tool ?? "tool")}</summary>`, "", "```", toolLine(en.text), "```", "");
        break;
      case "tool_result":
        out.push("```", en.text, "```", "", "</details>", "");
        break;
      case "compaction":
        break;
    }
  }
  return out.join("\n");
}

/** One html file that opens anywhere: the rendered body with the app's
 *  reading styles inlined, no script, no external asset. */
export function pageHtml(title: string, body: string): string {
  return `<!doctype html>
<html lang="pt-BR">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${esc(title)}</title>
<style>
  body { max-width: 760px; margin: 40px auto; padding: 0 20px; font: 15px/1.6 -apple-system, "Segoe UI", sans-serif; color: #1f2430; background: #fff; }
  h1 { font-size: 26px; letter-spacing: -.01em; } h2 { font-size: 19px; margin-top: 28px; }
  pre { background: #f4f5f8; padding: 12px; border-radius: 8px; overflow: auto; font-size: 13px; }
  code { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: .92em; }
  table { border-collapse: collapse; } td, th { border: 1px solid #ddd; padding: 4px 8px; }
  a { color: #2f5fd1; }
  @media (prefers-color-scheme: dark) { body { background: #12151c; color: #d6dae3; } pre { background: #1b1f29; } td, th { border-color: #333; } a { color: #7aa2f7; } }
</style>
</head>
<body>
${body}
</body>
</html>
`;
}
