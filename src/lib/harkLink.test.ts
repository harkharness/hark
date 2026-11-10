import { describe, expect, it } from "vitest";
import { harkTarget } from "./harkLink";

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
