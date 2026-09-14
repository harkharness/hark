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
import { t } from "../lib/i18n";

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

describe("the footer of a reply tells the truth about its price", () => {
  it("an unpriced turn says so, signed by its agent, never $0.0000", () => {
    // Observed with claude-code-acp: the footer read "acp · $0.0000" for a
    // turn that burned 27k tokens of opus on the user's subscription.
    const { container } = render(
      <Parent
        messages={[
          { who: "user", text: "oi", ts: 1_000 },
          { who: "hark", text: "resposta", ts: 2_000, model: "claude-acp" },
        ]}
      />,
    );
    const text = container.textContent ?? "";
    expect(text).not.toContain("$0.0000");
    expect(text).toContain(t("cost_unknown"));
    expect(text).toContain("claude-acp · ");
    // The bare "acp" was the id with its prefix cut off as if it were a model.
    expect(text).not.toMatch(/(?<!claude-)acp · /);
  });

  it("a priced turn still shows its dollars", () => {
    const { container } = render(
      <Parent
        messages={[
          { who: "user", text: "oi", ts: 1_000 },
          { who: "hark", text: "resposta", ts: 2_000, model: "claude-haiku-4-5", cost: 0.0123 },
        ]}
      />,
    );
    expect(container.textContent).toContain("$0.0123");
  });
});
