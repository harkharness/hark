/**
 * The session bar: the chat's own column, as in the Claude Code app. It
 * sits between the conversation and the chat's panels (terminal, files,
 * browser) and is not one of them, so it never trades height with a
 * panel. What it trades is width with the conversation, and it is the
 * first to yield: when the chat's column cannot hold both, the bar hides
 * by itself, and its button shows it over the page for a look.
 */

/** The bar's width, in px. Fixed: a bar that resizes is a panel. */
export const BAR_WIDTH = 288;

/** The least width the conversation keeps beside the bar, in px. */
export const CHAT_ROOM = 440;

export type BarState = {
  /** The person's choice for a window with room: bar docked, or put away. */
  pinned: boolean;
  /** Shown by hand in a window without room, over the page. */
  peek: boolean;
};

export type BarMode = "docked" | "over" | "hidden";

const PINNED_KEY = "hark.sessionbar.pinned";

/** Whether the chat's column, `room` px wide, holds the conversation
 *  and the bar side by side. */
export function barFits(room: number): boolean {
  return room >= CHAT_ROOM + BAR_WIDTH;
}

export function barMode(state: BarState, fits: boolean): BarMode {
  if (fits) return state.pinned ? "docked" : "hidden";
  return state.peek ? "over" : "hidden";
}

/** The bar's button. With room it sets the choice that is remembered;
 *  without, it is only a look, and the choice stays as it was. */
export function toggleBar(state: BarState, fits: boolean): BarState {
  return fits ? { pinned: !state.pinned, peek: false } : { ...state, peek: !state.peek };
}

/** The room changed (the window, a panel dragged wider, another chat): a look
 *  taken before does not carry over to the new room. */
export function settleBar(state: BarState, _fits: boolean): BarState {
  return state.peek ? { ...state, peek: false } : state;
}

/** The remembered choice. The bar starts docked: it is part of the window. */
export function readPinned(): boolean {
  try {
    return window.localStorage.getItem(PINNED_KEY) !== "0";
  } catch {
    return true;
  }
}

export function writePinned(pinned: boolean) {
  try {
    window.localStorage.setItem(PINNED_KEY, pinned ? "1" : "0");
  } catch {
    /* a private window keeps nothing */
  }
}

/** How long something has been running, the way a person says it. */
export function elapsed(ms: number): string {
  const s = Math.max(1, Math.round(ms / 1000));
  if (s < 60) return `${s} s`;
  const min = Math.floor(s / 60);
  if (min < 60) return `${min} min`;
  const h = Math.floor(min / 60);
  if (h < 24) return min % 60 ? `${h} h ${min % 60} min` : `${h} h`;
  const d = Math.floor(h / 24);
  return h % 24 ? `${d} d ${h % 24} h` : `${d} d`;
}
