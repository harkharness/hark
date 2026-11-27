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

describe("a page in a project that keeps its pages in itself", () => {
  // domain::pages_home: `<project>/docs/hark/` is the space's folder.
  const homes = [{ space: "tulipafin", dir: "/Users/ana/code/tulipafin/docs/hark" }];

  it("is a page of the project's space, not a file for the editor", () => {
    expect(artifactFile("/Users/ana/code/tulipafin/docs/hark/decisoes/adr-004.md", homes)).toEqual({ space: "tulipafin", file: "decisoes/adr-004.md" });
  });

  it("is the same page written with the home as ~", () => {
    expect(artifactFile("~/code/tulipafin/docs/hark/home.html", homes)).toEqual({ space: "tulipafin", file: "home.html" });
  });

  it("leaves the project's other documents to the editor", () => {
    expect(artifactFile("/Users/ana/code/tulipafin/docs/guia.md", homes)).toBeNull();
    expect(artifactFile("/Users/ana/code/tulipafin/docs/hark-old/a.md", homes)).toBeNull();
    expect(artifactFile("/Users/ana/code/horta/docs/hark/a.md", homes)).toBeNull();
  });

  it("never takes the space's index or the folder itself for a page", () => {
    expect(artifactFile("/Users/ana/code/tulipafin/docs/hark/index.json", homes)).toBeNull();
    expect(artifactFile("/Users/ana/code/tulipafin/docs/hark", homes)).toBeNull();
    expect(artifactFile("/Users/ana/code/tulipafin/docs/hark/", homes)).toBeNull();
  });

  it("still knows a page of the machine's root", () => {
    expect(artifactFile("~/.hark/artifacts/pessoal/notas.md", homes)).toEqual({ space: "pessoal", file: "notas.md" });
  });

  it("the innermost project owns a page when one project sits in another", () => {
    const nested = [
      { space: "mono", dir: "/p/mono/docs/hark" },
      { space: "api", dir: "/p/mono/apps/api/docs/hark" },
    ];
    expect(artifactFile("/p/mono/apps/api/docs/hark/a.md", nested)).toEqual({ space: "api", file: "a.md" });
  });
});
