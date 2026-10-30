import { describe, expect, it } from "vitest";
import { childrenOf, crumbOf, recents, rows, fold } from "./artifacts";
import type { ArtifactNode, ArtifactTree } from "../types";

const node = (id: string, title: string, parent: string | null = null, updated_at = "2026-10-28T10:00:00Z"): ArtifactNode => ({
  id,
  title,
  path: `${id}.md`,
  parent,
  kind: "doc",
  updated_at,
});

/** hark: Visão geral · Decisões[ADR 003, ADR 004] · Fase 10[Proposta] */
const tree: ArtifactTree = {
  nodes: [
    node("visao", "Visão geral", null, "2026-10-20T10:00:00Z"),
    node("decisoes", "Decisões", null, "2026-10-21T10:00:00Z"),
    node("adr-003", "ADR 003: teto por turno", "decisoes", "2026-10-22T10:00:00Z"),
    node("adr-004", "ADR 004: janela única", "decisoes", "2026-10-28T09:00:00Z"),
    node("fase-10", "Fase 10 · cabine", null, "2026-10-27T10:00:00Z"),
    node("proposta", "Proposta: cabine organizada", "fase-10", "2026-10-27T12:00:00Z"),
  ],
  order: { "": ["visao", "decisoes", "fase-10"], decisoes: ["adr-003", "adr-004"], "fase-10": ["proposta"] },
};

describe("rows: the tree flattened the way the column paints it", () => {
  it("keeps the index order and shows children only under open parents", () => {
    expect(rows(tree, new Set(), "").map((r) => [r.node.id, r.depth])).toEqual([["visao", 0], ["decisoes", 0], ["fase-10", 0]]);
    expect(rows(tree, new Set(["decisoes"]), "").map((r) => r.node.id)).toEqual(["visao", "decisoes", "adr-003", "adr-004", "fase-10"]);
  });
  it("says which rows have children, so the caret knows", () => {
    const r = rows(tree, new Set(), "");
    expect(r.find((x) => x.node.id === "decisoes")?.hasChildren).toBe(true);
    expect(r.find((x) => x.node.id === "visao")?.hasChildren).toBe(false);
  });
  it("a query flattens to the matches, accent-insensitive, parents open or not", () => {
    expect(rows(tree, new Set(), "decisoes").map((r) => [r.node.id, r.depth])).toEqual([["decisoes", 0]]);
    expect(rows(tree, new Set(), "ADR").map((r) => r.node.id)).toEqual(["adr-003", "adr-004"]);
  });
});

describe("childrenOf / crumbOf / recents", () => {
  it("children come in index order, orphans of the order list last", () => {
    expect(childrenOf(tree, "decisoes").map((n) => n.id)).toEqual(["adr-003", "adr-004"]);
    const loose = { ...tree, order: { ...tree.order, decisoes: ["adr-004"] } };
    expect(childrenOf(loose, "decisoes").map((n) => n.id)).toEqual(["adr-004", "adr-003"]);
  });
  it("the breadcrumb is the titles from the root down", () => {
    expect(crumbOf(tree, "adr-004")).toEqual(["Decisões", "ADR 004: janela única"]);
    expect(crumbOf(tree, "visao")).toEqual(["Visão geral"]);
  });
  it("recents is newest first, at most n", () => {
    expect(recents(tree, 3).map((n) => n.id)).toEqual(["adr-004", "proposta", "fase-10"]);
  });
});

describe("fold: the lowercase, accent-free haystack", () => {
  it("drops accents and case", () => {
    expect(fold("Decisões ÚNICA")).toBe("decisoes unica");
  });
});

describe("recents: what moved last", () => {
  it("leaves out a page whose file is gone: there is nothing to open", () => {
    const t: ArtifactTree = {
      nodes: [
        { ...node("gone", "Sumiu", null, "2026-10-30T12:00:00Z"), orphan: true },
        node("here", "Está aqui", null, "2026-10-29T12:00:00Z"),
      ],
      order: { "": ["gone", "here"] },
    };
    expect(recents(t, 12).map((n) => n.id)).toEqual(["here"]);
  });
});
