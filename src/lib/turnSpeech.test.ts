import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createTurnSpeech, QUIET_MS } from "./turnSpeech";

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

function setup() {
  const said: string[] = [];
  const speech = createTurnSpeech((text) => said.push(text));
  return { said, speech };
}

describe("what a chat says out loud when its work lands", () => {
  it("says the turn's news once the chat has gone quiet", () => {
    const { said, speech } = setup();
    speech.turnDone("t1", "Site: branch main atualizada.");
    expect(said).toEqual([]);
    vi.advanceTimersByTime(QUIET_MS);
    expect(said).toEqual(["Site: branch main atualizada."]);
  });

  it("says one thing for a chat whose agents land one after the other", () => {
    // Six agents in parallel: each one coming back wakes the main chat,
    // which ends a turn of its own. Six turns, one piece of news.
    const { said, speech } = setup();
    for (let i = 1; i <= 6; i++) {
      speech.turnDone("t1", `Site: parte ${i}.`);
      vi.advanceTimersByTime(1500);
      speech.activity("t1");
      vi.advanceTimersByTime(1500);
    }
    speech.turnDone("t1", "Site: tudo pronto, 6 páginas traduzidas.");
    vi.advanceTimersByTime(QUIET_MS);
    expect(said).toEqual(["Site: tudo pronto, 6 páginas traduzidas."]);
  });

  it("keeps quiet while the chat has work running in the background", () => {
    const { said, speech } = setup();
    speech.backgroundStarted("t1");
    speech.turnDone("t1", "Site: disparei o build.");
    vi.advanceTimersByTime(QUIET_MS * 3);
    expect(said).toEqual([]);
  });

  it("speaks when the background work ends and nothing else follows", () => {
    const { said, speech } = setup();
    speech.backgroundStarted("t1");
    speech.turnDone("t1", "Site: disparei o build.");
    vi.advanceTimersByTime(QUIET_MS);
    speech.backgroundDone("t1");
    expect(said).toEqual([]);
    vi.advanceTimersByTime(QUIET_MS);
    expect(said).toEqual(["Site: disparei o build."]);
  });

  it("drops the old news when the chat goes back to work", () => {
    const { said, speech } = setup();
    speech.turnDone("t1", "Site: metade feita.");
    speech.activity("t1");
    vi.advanceTimersByTime(QUIET_MS * 2);
    expect(said).toEqual([]);
  });

  it("keeps the chats apart: one working does not silence another", () => {
    const { said, speech } = setup();
    speech.turnDone("t1", "Site: pronto.");
    speech.activity("t2");
    speech.turnDone("t2", "App: pronto.");
    vi.advanceTimersByTime(QUIET_MS);
    expect(said).toEqual(["Site: pronto.", "App: pronto."]);
  });

  it("forgets a chat that is gone", () => {
    const { said, speech } = setup();
    speech.backgroundStarted("t1");
    speech.turnDone("t1", "Site: pronto.");
    speech.forget("t1");
    speech.backgroundDone("t1");
    vi.advanceTimersByTime(QUIET_MS * 2);
    expect(said).toEqual([]);
  });
});
