import { describe, expect, it } from "vitest";
import { artifactFile, harkTarget, pageId } from "./harkLink";

describe("a hark:// link clicked in a chat", () => {
  it("names a page by its id, with or without the file's extension", () => {
    expect(harkTarget("hark://artifacts/tulipa/adr-004")).toEqual({ kind: "page", space: "tulipa", id: "adr-004" });
    expect(harkTarget("hark://artifacts/tulipa/custos.sheet.json")).toEqual({ kind: "page", space: "tulipa", id: "custos" });
    expect(harkTarget("hark://artifacts/tulipa/home.html")).toEqual({ kind: "page", space: "tulipa", id: "home" });
    expect(harkTarget("hark://artifacts/meu%20projeto/plano.board.json#sel=n1")).toEqual({ kind: "page", space: "meu projeto", id: "plano" });
  });

  it("names a chat by its session", () => {
    expect(harkTarget("hark://chat/0c4f91ab")).toEqual({ kind: "chat", session: "0c4f91ab" });
  });

  it("anything else is not ours", () => {
    expect(harkTarget("https://example.com/x")).toBeNull();
    expect(harkTarget("src/lib.rs:38")).toBeNull();
    expect(harkTarget("hark://artifacts/tulipa/")).toBeNull();
    expect(harkTarget("hark://artifacts/../x/y")).toBeNull();
  });
});

describe("an output path inside the artifacts folder", () => {
  it("is a page of its space, in ~ or absolute form, whatever the extension", () => {
    expect(artifactFile("~/.hark/artifacts/tulipa/custos.sheet.json")).toEqual({ space: "tulipa", file: "custos.sheet.json" });
    expect(artifactFile("/Users/ana/.hark/artifacts/tulipa/docs/adr-004.md")).toEqual({ space: "tulipa", file: "docs/adr-004.md" });
    expect(artifactFile("/Users/ana/.hark/artifacts/tulipa/home.html")).toEqual({ space: "tulipa", file: "home.html" });
  });

  it("is not a page when the path is elsewhere or names the space itself", () => {
    expect(artifactFile("~/.claude/plans/x.md")).toBeNull();
    expect(artifactFile("~/.hark/artifacts/tulipa")).toBeNull();
    expect(artifactFile("~/.hark/artifacts/tulipa/index.json")).toBeNull();
  });

  it("finds the page by its file in the tree, because the id may not be the file's stem", () => {
    const nodes = [{ id: "test-documento", path: "home.html" }, { id: "custos", path: "custos.sheet.json" }];
    expect(pageId("home.html", nodes)).toBe("test-documento");
    expect(pageId("custos.sheet.json", nodes)).toBe("custos");
    expect(pageId("novo.board.json", nodes)).toBe("novo");
  });
});
