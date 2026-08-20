import type { ReactNode } from "react";
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

/** Three render shapes: one quiet line, a folded payload, or an
 *  always-visible card (deliverables — never hidden behind a fold). */
type Rendered =
  | { label: string; hint: string; body: ReactNode | null }
  | { card: ReactNode };

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
  /** When set, owns ALL path routing (resolve relative, editor vs OS). */
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
    if (onOpenPath) onOpenPath(path);
    else ipc.openExternal(path).catch(() => {});
  };

  const pathLine = (path: string) =>
    path ? (
      <button className="path clickable" title={t("tc_open_editor")} onClick={() => openFile(path)}>
        {shortPath(path)}
      </button>
    ) : null;

  const fileChip = (path: string) => {
    const base = path.split("/").filter(Boolean).pop() ?? path;
    const ext = base.includes(".") ? (base.split(".").pop() ?? "").slice(0, 5) : "file";
    return (
      <button
        key={path}
        className="tool-file"
        title={OS_ONLY.test(path) ? t("tc_open_os") : t("tc_open_editor")}
        onClick={() => openFile(path)}
      >
        <span className="tool-file-badge">{ext}</span>
        <span className="tool-file-name">{base}</span>
        <span className="tool-file-dir">{shortPath(path)}</span>
      </button>
    );
  };

  /** summary hint + optional folded body, per tool. */
  const rendered: Rendered = (() => {
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
      case "ExitPlanMode": {
        // ONE line: "plano proposto <arquivo>". The full plan lives in the
        // file — clicking opens the markdown viewer, the chat stays clean.
        const plan = str("plan") ?? "";
        const planPath = str("planFilePath") ?? "";
        return {
          card: (
            <div className="toolcall inline">
              <span className="toolname">{t("tc_plan")}</span>
              {planPath ? (
                pathLine(planPath)
              ) : (
                <span className="tool-hint">{plan.split("\n")[0].slice(0, 90)}</span>
              )}
            </div>
          ),
        };
      }
      case "SendUserFile": {
        // The deliverable: always visible, caption once, files as cards.
        const files = Array.isArray(parsed["files"])
          ? (parsed["files"] as unknown[]).filter((f): f is string => typeof f === "string")
          : [];
        const caption = str("caption");
        return {
          card: (
            <div className="tool-deliver">
              <div className="deliver-head">{t("tc_send_file")}</div>
              {caption && <div className="deliver-caption">{caption}</div>}
              <div className="tool-files">{files.map(fileChip)}</div>
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

  if ("card" in rendered) return <>{rendered.card}</>;
  const { label, hint, body } = rendered;

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
