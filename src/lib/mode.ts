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
