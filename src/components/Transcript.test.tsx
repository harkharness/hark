// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render } from "@testing-library/react";
import type { Msg } from "../types";

// Tauri is not here: the speaking listener would reach for window.__TAURI__.
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => undefined) }));

// The probe. In the real component every Markdown render is one parse
// plus one syntax highlight, which is exactly the cost these tests pin.
const probe = vi.hoisted(() => ({ renders: 0 }));
vi.mock("./Markdown", () => ({
  default: ({ children }: { children: string }) => {
    probe.renders += 1;
    return <div className="md">{children}</div>;
  },
}));

import Transcript from "./Transcript";

// jsdom lays nothing out, so it has no scrollIntoView to offer.
Element.prototype.scrollIntoView = vi.fn();

const thread: Msg[] = [
  { who: "user", text: "oi", ts: 1_000 },
  { who: "hark", text: "# olá", ts: 2_000 },
  { who: "hark", text: "segunda resposta", ts: 3_000 },
];

/** Stands in for App: every render hands the transcript FRESH callbacks,
 *  the way inline lambdas and plain function declarations do. */
function Parent({ messages }: { messages: Msg[] }) {
  return (
    <Transcript
      messages={messages}
      directivesFor={() => undefined}
      onAnswerPermission={() => {}}
      onOpenPath={() => {}}
      onRunCommand={() => {}}
    />
  );
}

afterEach(() => {
  cleanup();
  probe.renders = 0;
});

describe("Transcript re-renders only what changed", () => {
  it("a parent re-render with the same thread runs no markdown at all", () => {
    const { rerender } = render(<Parent messages={thread} />);
    expect(probe.renders).toBe(2);
    rerender(<Parent messages={thread} />);
    expect(probe.renders).toBe(2);
  });

  it("appending a reply renders that reply and nothing else", () => {
    const { rerender } = render(<Parent messages={thread} />);
    rerender(
      <Parent messages={[...thread, { who: "hark", text: "terceira", ts: 4_000 }]} />,
    );
    expect(probe.renders).toBe(3);
  });
});
