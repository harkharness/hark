import Markdown from "./Markdown";

/** Shorten absolute paths to something readable in a narrow pane. */
function shortPath(path: string): string {
  return path.replace(/^\/Users\/[^/]+\//, "~/");
}

function fence(lang: string, body: string): string {
  return "```" + lang + "\n" + body.trimEnd() + "\n```";
}

/**
 * A tool call rendered the way a human reads it: shell commands as real
 * multi-line shell blocks, edits as diffs, everything else as indented JSON.
 * Never the escaped one-liner JSON the CLI emits.
 */
export default function ToolCall({ name, input }: { name: string; input: string }) {
  let parsed: Record<string, unknown>;
  try {
    parsed = JSON.parse(input);
  } catch {
    return (
      <div className="toolcall">
        <span className="toolname">{name}</span>
        <pre className="raw">{input}</pre>
      </div>
    );
  }

  const str = (key: string): string | undefined =>
    typeof parsed[key] === "string" ? (parsed[key] as string) : undefined;

  const body = (() => {
    switch (name) {
      case "Bash": {
        const command = str("command") ?? "";
        return (
          <>
            {str("description") && <div className="caption">{str("description")}</div>}
            <Markdown>{fence("bash", command)}</Markdown>
          </>
        );
      }
      case "Read":
      case "Write": {
        const path = str("file_path") ?? str("path") ?? "";
        const content = str("content");
        return (
          <>
            <div className="path">{shortPath(path)}</div>
            {content && <Markdown>{fence("", content.slice(0, 1500))}</Markdown>}
          </>
        );
      }
      case "Edit": {
        const path = str("file_path") ?? "";
        const before = (str("old_string") ?? "")
          .split("\n")
          .map((l) => `-${l}`)
          .join("\n");
        const after = (str("new_string") ?? "")
          .split("\n")
          .map((l) => `+${l}`)
          .join("\n");
        return (
          <>
            <div className="path">{shortPath(path)}</div>
            <Markdown>{fence("diff", `${before}\n${after}`)}</Markdown>
          </>
        );
      }
      case "Grep":
      case "Glob": {
        const pattern = str("pattern") ?? "";
        const where = str("path") ?? str("glob");
        return (
          <div className="path">
            <code>{pattern}</code>
            {where ? ` em ${shortPath(where)}` : ""}
          </div>
        );
      }
      default:
        return <Markdown>{fence("json", JSON.stringify(parsed, null, 2))}</Markdown>;
    }
  })();

  return (
    <div className="toolcall">
      <span className="toolname">{name}</span>
      {body}
    </div>
  );
}

/** Tool output: first lines visible, the rest behind a disclosure. */
export function ToolOutput({ content, isError }: { content: string; isError: boolean }) {
  const lines = content.split("\n");
  const head = lines.slice(0, 8).join("\n");
  const hidden = lines.length - 8;
  return (
    <div className={`tooloutput ${isError ? "error" : ""}`}>
      <pre>{head}</pre>
      {hidden > 0 && (
        <details>
          <summary>+{hidden} linhas</summary>
          <pre>{lines.slice(8).join("\n")}</pre>
        </details>
      )}
    </div>
  );
}
