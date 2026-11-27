import { describe, expect, it } from "vitest";
import { BAR_WIDTH, CHAT_ROOM, barFits, barMode, elapsed, settleBar, toggleBar, type BarState } from "./sessionBar";

const docked: BarState = { pinned: true, peek: false };
const put_away: BarState = { pinned: false, peek: false };

describe("barFits", () => {
  // The bar is the chat's: it lives in the chat's column, between the
  // chat and the panels, so the room that counts is that column's.
  it("fits a chat column wide enough for the conversation and the bar", () => {
    expect(barFits(900)).toBe(true);
  });

  it("does not fit once a panel beside the chat has taken the column's room", () => {
    expect(barFits(640)).toBe(false);
  });

  it("fits exactly when the conversation keeps its least width beside the bar", () => {
    expect(barFits(CHAT_ROOM + BAR_WIDTH)).toBe(true);
    expect(barFits(CHAT_ROOM + BAR_WIDTH - 1)).toBe(false);
  });

  it("never fits a column not measured yet", () => {
    expect(barFits(0)).toBe(false);
  });
});

describe("barMode", () => {
  it("is docked beside the chat while there is room and the person wants it", () => {
    expect(barMode(docked, true)).toBe("docked");
  });

  it("is hidden while there is room and the person put it away", () => {
    expect(barMode(put_away, true)).toBe("hidden");
  });

  it("hides by itself when the room runs out, wanted or not", () => {
    expect(barMode(docked, false)).toBe("hidden");
  });

  it("shown by hand without room, it lies over the page instead of squeezing it", () => {
    expect(barMode({ pinned: true, peek: true }, false)).toBe("over");
    expect(barMode({ pinned: false, peek: true }, false)).toBe("over");
  });
});

describe("toggleBar", () => {
  it("with room, puts the bar away and brings it back, and that is remembered", () => {
    expect(toggleBar(docked, true)).toEqual(put_away);
    expect(toggleBar(put_away, true)).toEqual(docked);
  });

  it("without room, shows it over the page and hides it again", () => {
    const shown = toggleBar(docked, false);
    expect(barMode(shown, false)).toBe("over");
    expect(barMode(toggleBar(shown, false), false)).toBe("hidden");
  });

  it("a look taken without room does not change what the person chose for a wide window", () => {
    expect(toggleBar(put_away, false).pinned).toBe(false);
    expect(toggleBar(docked, false).pinned).toBe(true);
  });
});

describe("settleBar", () => {
  it("when the room runs out, a bar shown by hand before does not come back by itself", () => {
    const narrow = settleBar({ pinned: true, peek: true }, false);
    expect(barMode(narrow, false)).toBe("hidden");
  });

  it("when the room comes back, the bar returns to what the person chose", () => {
    expect(barMode(settleBar({ pinned: true, peek: true }, true), true)).toBe("docked");
    expect(barMode(settleBar({ pinned: false, peek: true }, true), true)).toBe("hidden");
  });

  it("changes nothing when there is nothing to settle", () => {
    expect(settleBar(docked, true)).toBe(docked);
  });
});

describe("elapsed", () => {
  // How long a background task has been running, the way a person says it.
  it("counts seconds under a minute", () => {
    expect(elapsed(16_000)).toBe("16 s");
    expect(elapsed(400)).toBe("1 s");
  });

  it("counts minutes under an hour", () => {
    expect(elapsed(5 * 60_000 + 20_000)).toBe("5 min");
  });

  it("counts hours and minutes under a day", () => {
    expect(elapsed(3 * 3_600_000 + 12 * 60_000)).toBe("3 h 12 min");
    expect(elapsed(2 * 3_600_000)).toBe("2 h");
  });

  it("counts days and hours past a day", () => {
    expect(elapsed(4 * 86_400_000 + 16 * 3_600_000)).toBe("4 d 16 h");
    expect(elapsed(86_400_000)).toBe("1 d");
  });
});
