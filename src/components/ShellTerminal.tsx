import { useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import * as ipc from "../lib/ipc";

type TermOut = { id: string; data: string | null; exit: boolean };

/**
 * xterm instances outlive their React mount: closing/reopening the pane
 * must not wipe the scrollback (the PTY on the Rust side stays alive
 * either way). Disposed only when the tab is explicitly closed.
 */
const cache = new Map<string, { term: Terminal; fit: FitAddon }>();

function cssVar(name: string, fallback: string): string {
  const v = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return v || fallback;
}

function instance(id: string, cwd?: string) {
  let entry = cache.get(id);
  if (entry) return entry;
  const term = new Terminal({
    fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
    fontSize: 12.5,
    cursorBlink: true,
    scrollback: 4000,
    theme: {
      background: cssVar("--bg", "#0b0f14"),
      foreground: cssVar("--text", "#c8d3f5"),
      cursor: cssVar("--accent", "#7aa2f7"),
      selectionBackground: "rgba(122,162,247,.3)",
    },
  });
  const fit = new FitAddon();
  term.loadAddon(fit);
  // Keystrokes → PTY. Registered once per terminal lifetime.
  term.onData((data) => ipc.termWrite(id, data).catch(() => {}));
  term.onResize(({ cols, rows }) => ipc.termResize(id, cols, rows).catch(() => {}));
  entry = { term, fit };
  cache.set(id, entry);
  // NO spawn here. The shell prints its prompt the instant it is born,
  // and the "term-out" listener does not exist yet at this point — the
  // prompt bytes were emitted into the void and the pane stayed black
  // forever. The mount effect spawns AFTER the listener is live.
  return entry;
}

/** Kill the shell and forget its buffer (explicit tab close only). */
export function disposeShell(id: string) {
  ipc.termClose(id).catch(() => {});
  cache.get(id)?.term.dispose();
  cache.delete(id);
}

/** One real shell (user's $SHELL, login) attached to a PTY tab. */
export default function ShellTerminal({
  id,
  cwd,
  onExit,
}: {
  id: string;
  cwd?: string;
  onExit?: () => void;
}) {
  const holder = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const { term, fit } = instance(id, cwd);
    if (holder.current) {
      term.open(holder.current);
      fit.fit();
      term.focus();
    }
    let cancelled = false;
    const un = listen<TermOut>("term-out", (e) => {
      if (e.payload.id !== id) return;
      if (e.payload.exit) {
        onExit?.();
        return;
      }
      if (e.payload.data) term.write(e.payload.data);
    });
    // Only spawn once the listener above is REGISTERED: the first bytes a
    // shell emits are its prompt, and bytes emitted before the listener
    // exists are simply gone. Idempotent on the Rust side, so remounts of
    // a live shell are a no-op. Real cols/rows: fit() already ran.
    un.then(() => {
      if (!cancelled) ipc.termOpen(id, cwd, term.cols, term.rows).catch(() => {});
    });
    const ro = new ResizeObserver(() => {
      try {
        fit.fit();
      } catch {
        /* pane mid-collapse */
      }
    });
    if (holder.current) ro.observe(holder.current);
    return () => {
      cancelled = true;
      un.then((f) => f());
      ro.disconnect();
      // No dispose here: the pane may just be hidden. disposeShell() is
      // the only place a terminal actually dies.
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id]);

  return <div className="shellterm" ref={holder} />;
}
