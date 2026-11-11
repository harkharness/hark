// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import { t } from "../lib/i18n";

const tree = {
  nodes: [
    { id: "adr-004", title: "ADR 004: one window", path: "decisions/adr-004.md", parent: "decisions", kind: "doc", origin: null, links: [], updated_at: "2026-11-10T10:00:00Z" },
    { id: "decisions", title: "Decisions", path: "decisions.md", parent: null, kind: "doc", origin: null, links: [], updated_at: "2026-11-10T10:00:00Z" },
    { id: "adr-005", title: "ADR 005: the rail", path: "decisions/adr-005.md", parent: "decisions", kind: "doc", origin: null, links: [], updated_at: "2026-11-10T10:00:00Z" },
  ],
  order: {},
};
const read = vi.fn(async (_space: string, id: string) => {
  const node = tree.nodes.find((n) => n.id === id)!;
  return { node, text: `# ${node.title}\n\nOne window, every project.`, abs: `/Users/ana/.hark/artifacts/tulipa/${node.path}`, breadcrumb: ["Decisions"] };
});
vi.mock("../lib/ipc", () => ({
  artifactsTree: async () => tree,
  artifactsSpaces: async () => [{ name: "tulipa", project: "/p/tulipa", pages: 3 }],
  artifactRead: (space: string, id: string) => read(space, id),
  artifactComments: async () => [],
  fileRead: async () => ({ content: "", truncated: false }),
  openExternal: async () => {},
}));

import { ArtifactPane } from "./Artifacts";

afterEach(cleanup);

const base = {
  tick: 0,
  colourOf: () => undefined,
  onOpenTask: () => {},
  onTakeTo: () => {},
  onSendTo: () => {},
  recentChats: [],
};

describe("the artifact pane beside a chat", () => {
  it("shows the one page, its title in the crumb, without the full tree", async () => {
    const { getByText, container } = render(<ArtifactPane sel={{ space: "tulipa", id: "adr-004" }} onSel={() => {}} {...base} />);
    await waitFor(() => expect(getByText("ADR 004: one window", { selector: "b" })).toBeTruthy());
    expect(container.querySelector(".art-tree")).toBeNull();
    expect(container.querySelector(".art-pane")).toBeTruthy();
    expect(read).toHaveBeenCalledWith("tulipa", "adr-004");
  });

  it("a sibling page named in the page opens in the same pane", async () => {
    const onSel = vi.fn();
    const { getByText } = render(<ArtifactPane sel={{ space: "tulipa", id: "decisions" }} onSel={onSel} {...base} />);
    await waitFor(() => expect(getByText("ADR 005: the rail")).toBeTruthy());
    fireEvent.click(getByText("ADR 005: the rail"));
    expect(onSel).toHaveBeenCalledWith({ space: "tulipa", id: "adr-005" });
  });

  it("re-reads the page when the agent writes it", async () => {
    const { rerender } = render(<ArtifactPane sel={{ space: "tulipa", id: "adr-004" }} onSel={() => {}} {...base} />);
    await waitFor(() => expect(read).toHaveBeenCalled());
    const n = read.mock.calls.length;
    rerender(<ArtifactPane sel={{ space: "tulipa", id: "adr-004" }} onSel={() => {}} {...base} tick={1} />);
    await waitFor(() => expect(read.mock.calls.length).toBeGreaterThan(n));
  });

  it("the back button of the full view returns to the chat", () => {
    expect(t("art_back_chat")).toBeTruthy();
  });
});
