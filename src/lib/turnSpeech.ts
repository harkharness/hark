/**
 * What a chat says out loud when its work lands, once.
 *
 * A turn ending is not the news. A chat that sent agents off in parallel
 * ends a turn every time one of them comes back, and announcing each one
 * was the same sentence six times in a row, with no room to think. The
 * news is what the main chat says when it has gone quiet: nothing
 * running in the background, no new turn started.
 *
 * So a turn's words are held, not spoken. Activity in the same chat drops
 * them (a later turn will have newer ones); background work keeps them
 * waiting; a quiet spell with neither speaks them, one time.
 */

/** How long a chat must stay quiet before its news is spoken. */
export const QUIET_MS = 5000;

type Held = { text: string; timer: ReturnType<typeof setTimeout> | null };

export function createTurnSpeech(say: (text: string) => void, quietMs = QUIET_MS) {
  const held = new Map<string, Held>();
  const background = new Map<string, number>();

  const disarm = (taskId: string) => {
    const h = held.get(taskId);
    if (h?.timer != null) clearTimeout(h.timer);
    if (h) h.timer = null;
  };
  const arm = (taskId: string) => {
    disarm(taskId);
    const h = held.get(taskId);
    if (!h) return;
    h.timer = setTimeout(() => {
      h.timer = null;
      // Still working behind the scenes: its end, or a new turn, decides.
      if ((background.get(taskId) ?? 0) > 0) return;
      held.delete(taskId);
      say(h.text);
    }, quietMs);
  };

  return {
    /** A turn ended with these words: the chat's news, if nothing follows. */
    turnDone(taskId: string, text: string) {
      disarm(taskId);
      held.set(taskId, { text, timer: null });
      arm(taskId);
    },
    /** The chat is working again: what it had to say is already old. */
    activity(taskId: string) {
      disarm(taskId);
      held.delete(taskId);
    },
    backgroundStarted(taskId: string) {
      background.set(taskId, (background.get(taskId) ?? 0) + 1);
    },
    /** The last background job landing starts the quiet spell over. */
    backgroundDone(taskId: string) {
      const left = Math.max(0, (background.get(taskId) ?? 0) - 1);
      background.set(taskId, left);
      if (left === 0 && held.has(taskId)) arm(taskId);
    },
    /** The chat is gone: nothing of it is said later. */
    forget(taskId: string) {
      disarm(taskId);
      held.delete(taskId);
      background.delete(taskId);
    },
  };
}
