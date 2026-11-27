/**
 * Which permission mode is in force, for the composer's pill. The pill
 * must say what the agent was really started with: it is the one place
 * the person reads whether a tool call will ask first.
 */

/** What an agent gets when it is started without a mode. */
export const NO_MODE = "manual";

/** The mode a chat started from this window gets: the pick, else the
 *  configured default, else none. A config without `worker_mode` reads as
 *  an empty string, which is no mode, not a mode of its own. */
export function windowMode(picked: string | null, configured?: string | null): string {
  return picked || configured || NO_MODE;
}

/** The mode the pill shows: a live chat's own (it keeps the flags it was
 *  started with, whatever the window default became since), else the
 *  window's, which is what the next chat will get. */
export function chatMode(live: { mode?: string | null } | undefined, window: string): string {
  return live ? live.mode || NO_MODE : window;
}

/** The standing rule that answers every tool of a chat. */
export const ANY_TOOL = "*";

/**
 * Whether the window answers a permission itself, the card showing up
 * already decided. Two reasons: a standing "always allow this tool" rule
 * of the chat, or a relaxed mode picked over a running turn, which the
 * backend flags on the ask because the process still has its old flags.
 * An ask that touches production always reaches the person.
 */
export function windowAllows(
  ask: { tool_name: string; mode_relaxed?: boolean; prod_risk?: string | null },
  rules: Set<string> | undefined,
): boolean {
  if (ask.prod_risk) return false;
  return !!ask.mode_relaxed || !!rules?.has(ask.tool_name) || !!rules?.has(ANY_TOOL);
}
