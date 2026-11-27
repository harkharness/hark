/**
 * "!" in the composer: the rest of the message is a shell command, run in
 * the chat's folder by Hark itself, as the terminal would.
 *
 * It is the person's command, typed by them: only the composer's own
 * submit reads a message this way. Nothing an agent prints and nothing
 * the microphone hears is ever run through here (SECURITY.md).
 */
import type { Msg } from "../types";
import { t } from "./i18n";

export type ShellMsg = Extract<Msg, { who: "shell" }>;

export type ShellEnd = "running" | "ok" | "failed" | "stopped" | "timeout";

/** The command a typed message is: what follows a "!" that is the very
 *  first character. A "!" anywhere else is punctuation. */
export function bangCommand(text: string): string | null {
  if (!text.startsWith("!")) return null;
  return text.slice(1).trim() || null;
}

/** Whether what the composer holds will run as a command on Enter. */
export function isBang(text: string): boolean {
  return text.startsWith("!");
}

/**
 * Text for the field that did not come from the keyboard (a page excerpt,
 * a link, a canned sentence), made unable to run. A page is written by an
 * agent, and "!" first would turn its words into a command one reflex
 * Enter away. A space in front keeps every character and is no command.
 */
export function asProse(text: string): string {
  return isBang(text) ? ` ${text}` : text;
}

/** How a command ended, the way the chat says it. */
export function shellEnd(m: ShellMsg): ShellEnd {
  if (m.running) return "running";
  // What the person did is the reason, whatever the process reported.
  if (m.stopped) return "stopped";
  if (m.timedOut) return "timeout";
  return m.code === 0 ? "ok" : "failed";
}

/** The finished commands of a chat that no message has carried to its
 *  agent yet. */
export function untold(messages: Msg[], thread: string | undefined): ShellMsg[] {
  return messages.filter(
    (m): m is ShellMsg =>
      m.who === "shell" && !m.running && !m.told && !m.error && (m.task ?? undefined) === thread,
  );
}

/** The same messages with these commands marked as carried. */
export function markTold(messages: Msg[], ids: string[]): Msg[] {
  if (ids.length === 0) return messages;
  const marked = new Set(ids);
  return messages.map((m) => (m.who === "shell" && marked.has(m.id) ? { ...m, told: true } : m));
}

/** What a command's end adds to the block, when it did not simply work. */
function endLine(m: ShellMsg): string | null {
  const end = shellEnd(m);
  if (end === "stopped") return t("shell_block_stopped");
  if (end === "timeout") return t("shell_block_timeout");
  if (end === "failed") return m.code == null ? t("shell_block_killed") : t("shell_block_exit", { n: m.code });
  return null;
}

/**
 * The commands as they ride inside the next message to the agent: in a
 * terminal the agent would have seen them scroll by, here it has to be
 * told. Attributed and closed, like a cited chat (`crossref`).
 */
export function shellBlock(runs: ShellMsg[]): string {
  if (runs.length === 0) return "";
  const dir = runs.find((m) => m.cwd)?.cwd;
  const lines = [dir ? t("shell_block_head", { dir }) : t("shell_block_head_bare")];
  for (const m of runs) {
    lines.push(`$ ${m.command}`);
    const output = (m.output ?? "").replace(/\n$/, "");
    if (output) lines.push(output);
    if (m.omitted) lines.push(t("shell_block_cut", { n: m.omitted }));
    const end = endLine(m);
    if (end) lines.push(end);
  }
  lines.push(t("shell_block_end"));
  return lines.join("\n");
}
