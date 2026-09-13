import { describe, expect, it } from "vitest";
import { keepIfSame } from "./settle";

describe("keepIfSame — a poll that says nothing new must not wake React", () => {
  it("hands back the OLD reference when the new answer is the same data", () => {
    const old = [{ session_id: "a", name: "x", cwd: "/p", pid: 1 }];
    const next = [{ session_id: "a", name: "x", cwd: "/p", pid: 1 }];
    expect(keepIfSame(next)(old)).toBe(old);
  });

  it("lets a changed field through", () => {
    const old = [{ session_id: "a", pid: 1 }];
    const next = [{ session_id: "a", pid: 2 }];
    expect(keepIfSame(next)(old)).toBe(next);
  });

  it("an element added or removed is a change", () => {
    const old = [{ session_id: "a" }];
    const grown = [{ session_id: "a" }, { session_id: "b" }];
    expect(keepIfSame(grown)(old)).toBe(grown);
    const gone: typeof old = [];
    expect(keepIfSame(gone)(old)).toBe(gone);
  });

  it("works for the nullable payloads too", () => {
    const some = { model: "opus", limits: [] as string[] };
    expect(keepIfSame<typeof some | null>(null)(null)).toBe(null);
    expect(keepIfSame<typeof some | null>({ ...some })(some)).toBe(some);
    expect(keepIfSame<typeof some | null>(null)(some)).toBe(null);
  });
});
