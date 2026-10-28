import { describe, expect, it } from "vitest";
import { collectOutputs, type Output } from "./outputs";
import type { Msg } from "../types";

const tool = (name: string, input: Record<string, unknown>, task?: string): Msg =>
  ({ who: "tool", name, input: JSON.stringify(input), task }) as Msg;

describe("collectOutputs: the files a thread produced, newest touch first", () => {
  it("lists a written file once with the number of touches", () => {
    const msgs = [tool("Write", { file_path: "/p/a.ts", content: "x" }), tool("Edit", { file_path: "/p/a.ts", old_string: "x", new_string: "y" })];
    expect(collectOutputs(msgs, null)).toEqual<Output[]>([{ path: "/p/a.ts", kind: "written", touches: 2 }]);
  });
  it("orders by the last touch, newest first", () => {
    const msgs = [tool("Write", { file_path: "/p/a.ts" }), tool("Write", { file_path: "/p/b.ts" }), tool("Edit", { file_path: "/p/a.ts" })];
    expect(collectOutputs(msgs, null).map((o) => o.path)).toEqual(["/p/a.ts", "/p/b.ts"]);
  });
  it("an edit without a write is 'edited'; a Write makes it 'written'; a delivery wins over both", () => {
    expect(collectOutputs([tool("Edit", { file_path: "/p/a.ts" })], null)[0].kind).toBe("edited");
    expect(collectOutputs([tool("MultiEdit", { file_path: "/p/a.ts", edits: [] })], null)[0].kind).toBe("edited");
    expect(collectOutputs([tool("Edit", { file_path: "/p/a.ts" }), tool("SendUserFile", { files: ["/p/a.ts"] })], null)[0].kind).toBe("delivered");
    expect(collectOutputs([tool("Write", { file_path: "/p/a.ts" }), tool("Edit", { file_path: "/p/a.ts" })], null)[0].kind).toBe("written");
  });
  it("only counts the thread asked for", () => {
    const msgs = [tool("Write", { file_path: "/p/a.ts" }, "Billing"), tool("Write", { file_path: "/p/b.ts" }, "Retry"), tool("Write", { file_path: "/p/c.ts" })];
    expect(collectOutputs(msgs, "Billing").map((o) => o.path)).toEqual(["/p/a.ts"]);
    expect(collectOutputs(msgs, null).map((o) => o.path)).toEqual(["/p/c.ts"]);
  });
  it("ignores reads, shells and payloads it cannot parse", () => {
    const msgs: Msg[] = [
      tool("Read", { file_path: "/p/a.ts" }),
      tool("Bash", { command: "ls" }),
      { who: "tool", name: "Write", input: "{not json" } as Msg,
      { who: "user", text: "oi" } as Msg,
    ];
    expect(collectOutputs(msgs, null)).toEqual([]);
  });
  it("a delivery of several files lists each one", () => {
    const out = collectOutputs([tool("SendUserFile", { files: ["/p/a.png", "/p/b.png"] })], null);
    expect(out.map((o) => [o.path, o.kind])).toEqual([["/p/a.png", "delivered"], ["/p/b.png", "delivered"]]);
  });
});
