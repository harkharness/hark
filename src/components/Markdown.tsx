import type { ReactNode } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeHighlight from "rehype-highlight";
import { Copy, Play, SquareTerminal } from "lucide-react";
import bash from "highlight.js/lib/languages/bash";
import json from "highlight.js/lib/languages/json";
import yaml from "highlight.js/lib/languages/yaml";
import rust from "highlight.js/lib/languages/rust";
import typescript from "highlight.js/lib/languages/typescript";
import python from "highlight.js/lib/languages/python";
import diff from "highlight.js/lib/languages/diff";
import sql from "highlight.js/lib/languages/sql";
import { t } from "../lib/i18n";
import * as ipc from "../lib/ipc";

// Only the grammars this workflow actually shows, to keep the bundle small.
const languages = { bash, json, yaml, rust, typescript, python, diff, sql };

/** Plain text of a rendered code block (highlight spans flattened). */
function textOf(node: ReactNode): string {
  if (node == null) return "";
  if (typeof node === "string" || typeof node === "number") return String(node);
  if (Array.isArray(node)) return node.map(textOf).join("");
  if (typeof node === "object" && "props" in node) {
    return textOf((node as { props: { children?: ReactNode } }).props.children);
  }
  return "";
}

/**
 * Assistant prose rendered like a real markdown document. Shell code
 * blocks grow action buttons: run in the terminal, insert without
 * running, copy — the Claude Code flow.
 */
export default function Markdown({
  children,
  onRun,
  onOpenPath,
}: {
  children: string;
  /** Send a command to the in-app terminal (execute=false just types it). */
  onRun?: (cmd: string, execute: boolean) => void;
  /** Owns path routing: resolves relative paths, picks editor vs OS. */
  onOpenPath?: (path: string) => void;
}) {
  return (
    <div className="md">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        rehypePlugins={[[rehypeHighlight, { languages, detect: false }]]}
        components={{
          // A link click must NEVER navigate the webview (that reloads the
          // SPA as the mother and loses the chat). URLs open in the OS
          // browser; file paths go to onOpenPath, which resolves relative
          // paths and picks editor vs OS app.
          a(props) {
            const href = props.href ?? "";
            return (
              <a
                {...props}
                onClick={(e) => {
                  e.preventDefault();
                  if (!href) return;
                  if (/^https?:/i.test(href)) {
                    ipc.openExternal(href).catch(() => {});
                    return;
                  }
                  const path = decodeURI(href);
                  if (onOpenPath) onOpenPath(path);
                  else ipc.openExternal(path).catch(() => {});
                }}
              />
            );
          },
          pre(props) {
            const child = props.children as {
              props?: { className?: string; children?: ReactNode };
            } | null;
            const lang = child?.props?.className ?? "";
            const isShell = /language-(bash|sh|shell|zsh|console)/.test(lang);
            if (!isShell) return <pre {...props} />;
            const cmd = textOf(child?.props?.children ?? null).trim();
            return (
              <div className="codeblock">
                <pre {...props} />
                <div className="code-actions">
                  {onRun && (
                    <>
                      <button title={t("code_run")} onClick={() => onRun(cmd, true)}>
                        <Play size={12} />
                      </button>
                      <button
                        title={t("code_insert")}
                        onClick={() => onRun(cmd, false)}
                      >
                        <SquareTerminal size={12} />
                      </button>
                    </>
                  )}
                  <button
                    title={t("code_copy")}
                    onClick={() => navigator.clipboard.writeText(cmd).catch(() => {})}
                  >
                    <Copy size={12} />
                  </button>
                </div>
              </div>
            );
          },
        }}
      >
        {children}
      </ReactMarkdown>
    </div>
  );
}
