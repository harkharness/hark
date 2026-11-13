import { describe, expect, it } from "vitest";
import { defaultHotkey, osOf, shortcutLabels } from "./platform";

describe("the desktop the window runs on", () => {
  it("is read from the platform string, mac first because it is the one with a different key", () => {
    expect(osOf("MacIntel")).toBe("mac");
    expect(osOf("Linux x86_64")).toBe("linux");
    expect(osOf("Win32")).toBe("windows");
    expect(osOf("")).toBe("linux");
  });

  it("says Cmd on a Mac and Ctrl everywhere else in every shortcut label", () => {
    expect(shortcutLabels("Salvar (Cmd+S), Cmd+K abre", "mac")).toBe("Salvar (Cmd+S), Cmd+K abre");
    expect(shortcutLabels("Salvar (Cmd+S), Cmd+K abre", "linux")).toBe("Salvar (Ctrl+S), Ctrl+K abre");
  });

  it("offers the voice hotkey on the key the desktop has", () => {
    expect(defaultHotkey("mac")).toBe("cmd+shift+space");
    expect(defaultHotkey("linux")).toBe("ctrl+shift+space");
    expect(defaultHotkey("windows")).toBe("ctrl+shift+space");
  });
});
