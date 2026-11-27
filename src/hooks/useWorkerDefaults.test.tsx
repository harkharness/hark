// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, renderHook } from "@testing-library/react";
import { useWorkerDefaults } from "./useWorkerDefaults";
import type { Overview } from "../types";

const configWrite = vi.fn(async (_patch: Record<string, string>) => undefined);
vi.mock("../lib/ipc", () => ({
  configRead: vi.fn(async () => ({ values: {} })),
  configWrite: (patch: Record<string, string>) => configWrite(patch),
}));

afterEach(() => {
  cleanup();
  configWrite.mockClear();
});

const overview = (default_mode: string) => ({ default_mode, default_model: "", default_effort: "" }) as unknown as Overview;

describe("the mode a message is sent with is the mode the pill shows", () => {
  it("is manual on both sides when the config has no mode", () => {
    const { result } = renderHook(() => useWorkerDefaults(overview("")));
    expect(result.current.modeInForce).toBe("manual");
    expect(result.current.mode).toBe(result.current.modeInForce);
  });

  it("is the configured mode on both sides", () => {
    const { result } = renderHook(() => useWorkerDefaults(overview("auto")));
    expect(result.current.modeInForce).toBe("auto");
    expect(result.current.mode).toBe("auto");
  });

  it("is the pick on both sides, and the pick is remembered for the next chats", () => {
    const { result } = renderHook(() => useWorkerDefaults(overview("")));
    act(() => result.current.setModeDefault("auto"));
    expect(result.current.modeInForce).toBe("auto");
    expect(result.current.mode).toBe("auto");
    expect(configWrite).toHaveBeenCalledWith({ worker_mode: "auto" });
  });

  it("is manual before the config has even arrived", () => {
    const { result } = renderHook(() => useWorkerDefaults(null));
    expect(result.current.mode).toBe("manual");
  });
});
