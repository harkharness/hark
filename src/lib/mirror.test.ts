import { describe, expect, it } from "vitest";
import { condense } from "./mirror";
import type { Msg } from "../types";

const T = "Billing v2";
const msgs: Msg[] = [
  { who: "user", text: "a permissão de deploy pode, mas só no staging", task: T },
  { who: "permission", requestId: "r1", tool: "Bash", input: '{"command":"deploy --env staging"}', decision: "allow", task: T },
  { who: "tool", name: "Bash", input: '{"command":"cargo test -p billing"}', task: T },
  { who: "output", content: "14 passed", error: false, task: T },
  { who: "tool", name: "Read", input: '{"file_path":"/p/hark/src/billing.rs"}', task: T },
  { who: "output", content: "no such file", error: true, task: T },
  { who: "compact", text: "…", task: T },
  { who: "hark", text: "Deploy no staging rodou. Abri o PR #142.", task: T },
  { who: "sys", text: "contexto carregado", task: T },
];

describe("the condensed mirror of a chat in focus", () => {
  it("turns tools into one line each, marked by their result, and keeps the words whole", () => {
    const out = condense(msgs);
    expect(out.map((l) => l.kind)).toEqual(["user", "perm", "tool", "tool", "text"]);
    expect(out[1]).toMatchObject({ kind: "perm", text: "Bash", state: "allow" });
    expect(out[2]).toMatchObject({ kind: "tool", text: "Bash · cargo test -p billing", ok: true });
    expect(out[3]).toMatchObject({ kind: "tool", text: "Read · /p/hark/src/billing.rs", ok: false });
    expect(out[4].text).toBe("Deploy no staging rodou. Abri o PR #142.");
  });

  it("keeps only the tail, newest last", () => {
    const out = condense(msgs, 2);
    expect(out.map((l) => l.kind)).toEqual(["tool", "text"]);
  });
});
