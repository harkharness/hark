// The desktop the window runs on. One question, asked once: the key the
// shortcuts sit on (Cmd on a Mac, Ctrl elsewhere), the voice hotkey's
// default, and the data-os attribute the stylesheet reads for the title
// bar (macOS paints the traffic lights over the page; the others keep
// their own decorations above it).

export type Os = "mac" | "linux" | "windows";

export function osOf(platform: string): Os {
  if (/mac|iphone|ipad/i.test(platform)) return "mac";
  if (/win/i.test(platform)) return "windows";
  return "linux";
}

export const OS: Os = osOf(typeof navigator === "undefined" ? "" : navigator.platform ?? "");
export const IS_MAC = OS === "mac";

/** Every "Cmd+" in a label becomes "Ctrl+" off the Mac. The dictionaries
 *  are written for the Mac and this one rule covers them all. */
export function shortcutLabels(text: string, os: Os = OS): string {
  return os === "mac" ? text : text.replaceAll("Cmd+", "Ctrl+");
}

export function defaultHotkey(os: Os = OS): string {
  return os === "mac" ? "cmd+shift+space" : "ctrl+shift+space";
}
