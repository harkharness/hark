/**
 * A markdown code fence around a tool's input (a command, a file body)
 * that the body cannot close. A closing fence needs at least as many
 * backticks as the opening one, so the fence is one longer than the
 * longest run inside — a heredoc carrying its own ``` stays text instead
 * of ending the block and rendering what follows as live markdown.
 */
export function fence(lang: string, body: string): string {
  const longest = Math.max(0, ...(body.match(/`+/g) ?? []).map((run) => run.length));
  const ticks = "`".repeat(Math.max(3, longest + 1));
  return ticks + lang + "\n" + body.trimEnd() + "\n" + ticks;
}
