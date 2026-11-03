import { memo, useEffect, useState } from "react";
import type { ReactNode } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeHighlight from "rehype-highlight";
import { Check, Copy, Play, SquareTerminal } from "lucide-react";
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

/** Extensions that make an inline code span a FILE, not a word. A closed
 *  list on purpose: prose is full of things shaped like paths that are
 *  not files — branch names ("feat/x"), "and/or", "example.com", versions
 *  ("v0.2.7"). Only these open an editor tab. */
const FILE_EXT =
  /\.(md|markdown|txt|rst|rs|ts|tsx|js|jsx|mjs|cjs|json|jsonc|toml|ya?ml|tf|tfvars|hcl|py|go|sh|bash|zsh|fish|sql|css|scss|html|xml|svg|lock|env|cfg|conf|ini|properties|gradle|mod|sum|java|kt|kts|rb|php|pl|lua|c|h|cc|cpp|hpp|cs|swift|m|proto|csv|tsv|log|patch|diff|dockerfile|makefile|mk|bazel|bzl|nix|vue|svelte|astro|ipynb|plist|entitlements)$/i;

/** A path with a real file at the end of it, optionally with :line:col.
 *  Absolute and ./~ forms are unambiguous and pass without an extension. */
export function looksLikePath(raw: string): boolean {
  const text = raw.trim();
  if (!text || /\s/.test(text) || text.length > 240) return false;
  const bare = text.replace(/:\d+(?::\d+)?$/, "");
  if (!/^[\w@~./+-]+(?:\/[\w@.+-]+)*$/.test(bare)) return false;
  if (/^[~.]?\//.test(bare)) return true; // /abs, ~/home, ./rel
  return FILE_EXT.test(bare);
}

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

/** One block's copy button: the icon says "done" for a beat, then not. */
function CopyBlock({ text }: { text: string }) {
  const [done, setDone] = useState(false);
  useEffect(() => {
    if (!done) return;
    const id = window.setTimeout(() => setDone(false), 1400);
    return () => window.clearTimeout(id);
  }, [done]);
  return (
    <button
      className={done ? "done" : ""}
      title={done ? t("code_copied") : t("code_copy")}
      onClick={() => {
        navigator.clipboard
          .writeText(text)
          .then(() => setDone(true))
          .catch(() => {});
      }}
    >
      {done ? <Check size={12} /> : <Copy size={12} />}
    </button>
  );
}

/**
 * Assistant prose rendered like a real markdown document. Every fenced
 * block can be copied on its own (a prompt to paste elsewhere, a config
 * snippet); shell blocks also run in the terminal or land in it typed
 * but not run — the Claude Code flow.
 */
function Markdown({
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
          // An inline `docs/oncall/guia.md` is a file the user can open,
          // and it read as dead text: only markdown LINKS were wired to
          // the path router. Prose names files far more often in backticks
          // than in link syntax.
          code(props) {
            const { children, className, node: _node, ...rest } = props;
            const text = textOf(children);
            if (className || !onOpenPath || !looksLikePath(text)) {
              return (
                <code className={className} {...rest}>
                  {children}
                </code>
              );
            }
            return (
              <code
                className="code-path"
                role="link"
                tabIndex={0}
                title={t("code_open_path", { p: text })}
                {...rest}
                onClick={() => onOpenPath(text.trim())}
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.preventDefault();
                    onOpenPath(text.trim());
                  }
                }}
              >
                {children}
              </code>
            );
          },
          pre(props) {
            const child = props.children as {
              props?: { className?: string; children?: ReactNode };
            } | null;
            const lang = child?.props?.className ?? "";
            const isShell = /language-(bash|sh|shell|zsh|console)/.test(lang);
            const text = textOf(child?.props?.children ?? null).trim();
            return (
              <div className={`codeblock ${isShell ? "shell" : ""}`}>
                <pre {...props} />
                <div className="code-actions">
                  {isShell && onRun && (
                    <>
                      <button title={t("code_run")} onClick={() => onRun(text, true)}>
                        <Play size={12} />
                      </button>
                      <button title={t("code_insert")} onClick={() => onRun(text, false)}>
                        <SquareTerminal size={12} />
                      </button>
                    </>
                  )}
                  <CopyBlock text={text} />
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

/**
 * Memoised on the text and the two callbacks. A render here is a full
 * parse plus a syntax highlight, and a thread re-rendering for any reason
 * used to pay it once per message — the transcript hands down stable
 * callbacks precisely so this comparison can succeed.
 */
export default memo(Markdown);
