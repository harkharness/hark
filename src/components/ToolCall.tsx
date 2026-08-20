import Markdown from "./Markdown";
import { t } from "../lib/i18n";
import * as ipc from "../lib/ipc";

/** Shorten absolute paths to something readable in a narrow pane. */
function shortPath(path: string): string {
  return path.replace(/^\/Users\/[^/]+\//, "~/");
}

function fence(lang: string, body: string): string {
  return "```" + lang + "\n" + body.trimEnd() + "\n```";
}

/** Formats the editor can't render — the OS opens these. */
const OS_ONLY = /\.(html?|pdf|png|jpe?g|gif|svg|webp)$/i;

/**
 * A tool call the way Claude Code shows it: ONE quiet summary line
 * (name + hint), body folded behind it — the chat stays readable and the
 * payload is one click away. Permission cards pass `defaultOpen`: what
 * you are authorizing must be visible.
 */
export default function ToolCall({
  name,
  input,
  onOpenPath,
  defaultOpen = false,
}: {
  name: string;
  input: string;
  /** When set, file paths become clickable and open the local viewer. */
  onOpenPath?: (path: string) => void;
  /** Start expanded (permission cards). */
  defaultOpen?: boolean;
}) {
  let parsed: Record<string, unknown>;
  try {
    parsed = JSON.parse(input);
  } catch {
    parsed = {};
  }
  const failedParse = Object.keys(parsed).length === 0 && input.trim() !== "{}";

  const str = (key: string): string | undefined =>
    typeof parsed[key] === "string" ? (parsed[key] as string) : undefined;

  const openFile = (path: string) => {
    if (onOpenPath && !OS_ONLY.test(path)) onOpenPath(path);
    else ipc.openExternal(path).catch(() => {});
  };

  const pathLine = (path: string) =>
    path ? (
      <button className="path clickable" title={t("tc_open_editor")} onClick={() => openFile(path)}>
        {shortPath(path)}
      </button>
    ) : null;

  /** summary hint + optional folded body, per tool. */
  const { label, hint, body } = (() => {
    switch (name) {
      case "Bash": {
        const command = str("command") ?? "";
        return {
          label: name,
          hint: str("description") ?? command.split("\n")[0].slice(0, 90),
          body: <Markdown>{fence("bash", command)}</Markdown>,
        };
      }
      case "Read":
      case "Write": {
        const path = str("file_path") ?? str("path") ?? "";
        const content = str("content");
        return {
          label: name,
          hint: shortPath(path),
          body: (
            <>
              {pathLine(path)}
              {content && <Markdown>{fence("", content.slice(0, 1500))}</Markdown>}
            </>
          ),
        };
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
        return {
          label: name,
          hint: shortPath(path),
          body: (
            <>
              {pathLine(path)}
              <Markdown>{fence("diff", `${before}\n${after}`)}</Markdown>
            </>
          ),
        };
      }
      case "Grep":
      case "Glob": {
        const pattern = str("pattern") ?? "";
        const where = str("path") ?? str("glob");
        return {
          label: name,
          hint: `${pattern}${where ? ` · ${shortPath(where)}` : ""}`,
          body: null,
        };
      }
      case "SendUserFile": {
        // The deliverable card, not raw JSON: caption + clickable files.
        const files = Array.isArray(parsed["files"])
          ? (parsed["files"] as unknown[]).filter((f): f is string => typeof f === "string")
          : [];
        const caption = str("caption");
        return {
          label: t("tc_send_file"),
          hint: caption ?? t("tc_files", { n: files.length }),
          body: (
            <div className="tool-files">
              {caption && <div className="caption">{caption}</div>}
              {files.map((f) => (
                <button
                  key={f}
                  className="tool-file"
                  title={OS_ONLY.test(f) ? t("tc_open_os") : t("tc_open_editor")}
                  onClick={() => openFile(f)}
                >
                  📄 {f.split("/").filter(Boolean).pop()}
                  <span className="tool-file-dir">{shortPath(f)}</span>
                </button>
              ))}
            </div>
          ),
        };
      }
      default: {
        const raw = failedParse ? input : JSON.stringify(parsed, null, 2);
        const first = failedParse ? input.slice(0, 90) : Object.keys(parsed).join(", ");
        return {
          label: name,
          hint: first,
          body: <Markdown>{fence("json", raw)}</Markdown>,
        };
      }
    }
  })();

  // No body = one quiet line; body = folded behind the summary.
  if (!body) {
    return (
      <div className="toolcall inline">
        <span className="toolname">{label}</span>
        <span className="tool-hint">{hint}</span>
      </div>
    );
  }
  return (
    <details className="toolcall fold" open={defaultOpen}>
      <summary>
        <span className="toolname">{label}</span>
        <span className="tool-hint">{hint}</span>
      </summary>
      <div className="tool-body">{body}</div>
    </details>
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
          <summary>{t("more_lines", { n: hidden })}</summary>
          <pre>{lines.slice(8).join("\n")}</pre>
        </details>
      )}
    </div>
  );
}
