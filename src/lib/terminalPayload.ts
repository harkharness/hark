const PASTE_START = "\x1b[200~";
const PASTE_END = "\x1b[201~";

/**
 * What a code block's run / insert buttons write into the terminal.
 *
 * The block is agent output, so nothing in it may act on the terminal by
 * itself: escape and control characters go (an ESC could end a paste
 * early or drive the terminal, ^C/^D/^U edit the line). Inserting must
 * not run anything, and every newline written raw is an Enter — so a
 * multi-line insert goes as one bracketed paste, which the shell takes
 * into its line editor without executing.
 */
export function terminalPayload(cmd: string, execute: boolean): string {
  const clean = cmd
    .replace(/\r\n?/g, "\n")
    // eslint-disable-next-line no-control-regex
    .replace(/[\x00-\x08\x0b-\x1f\x7f]/g, "")
    .replace(/\s+$/, "");
  if (execute) return clean + "\r";
  return clean.includes("\n") ? PASTE_START + clean + PASTE_END : clean;
}
