/**
 * For the setState of a poll.
 *
 * A poll answers on a timer whether or not anything happened, and a fresh
 * array from IPC is never `===` the one before — so `setOwners(list)`
 * re-rendered the whole project window every 2.5s with nothing new to
 * show. Measured with the app idle and unfocused: 14% of a core per
 * tick, growing with the transcript on screen, until a long thread spent
 * seconds inside each render and hovers stopped painting.
 *
 * Handing React the OLD value when the new one says the same thing lets
 * it bail out of the update entirely. Equality is structural, via JSON:
 * polled payloads are plain data from serde, in a fixed field order.
 */
export function keepIfSame<T>(next: T): (old: T) => T {
  const key = JSON.stringify(next);
  return (old) => (JSON.stringify(old) === key ? old : next);
}
