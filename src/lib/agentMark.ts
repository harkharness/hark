/**
 * The mark that identifies an agent in the catalog. Two problems it
 * solves: a grid of text rectangles cannot be scanned with the eye, and
 * two entries pointing at the SAME binary (a twin behind a gateway) look
 * identical until you read the id.
 *
 * Every glyph is drawn by us, tinted with the vendor's hue — no
 * third-party brand asset ships in the app. An agent the user wrote
 * themselves has no vendor, so it falls back to a monogram over a hue
 * derived from its own id: stable, and different from its neighbour's.
 */
export type MarkGlyph = "anthropic" | "google" | "openai" | "deepseek" | "aws" | "monogram";
export type MarkState = "on" | "idle" | "off" | "missing";

/** Vendor strings the catalog emits, lowercased. */
const GLYPHS: Record<string, MarkGlyph> = {
  anthropic: "anthropic",
  google: "google",
  openai: "openai",
  deepseek: "deepseek",
  aws: "aws",
};

export function glyphFor(vendor: string): MarkGlyph {
  return GLYPHS[vendor.trim().toLowerCase()] ?? "monogram";
}

/** The hues we tint a mark with, one per vendor we drew. */
export const VENDOR_HUE: Record<MarkGlyph, string> = {
  anthropic: "#c96442",
  google: "#5b8dee",
  openai: "#4fb79a",
  deepseek: "#6d7ff5",
  aws: "#e0af68",
  monogram: "#7aa2f7",
};

/** The palette a vendorless id is hashed into. */
const PALETTE = ["#7aa2f7", "#bb9af7", "#7dcfff", "#9ece6a", "#e0af68", "#f7768e"];

export function hueFor(id: string): string {
  let sum = 0;
  for (let i = 0; i < id.length; i++) sum += id.charCodeAt(i) * (i + 1);
  return PALETTE[sum % PALETTE.length];
}

/** First letter that is not a separator; "?" when there is none. */
export function monogramOf(id: string): string {
  const letter = id.replace(/^[^a-z0-9]+/i, "")[0];
  return letter ? letter.toUpperCase() : "?";
}

/** The ring's state. Off wins over missing: the switch is the fix, and
 *  a disabled agent is not worth alarming anyone about. */
export function markStateOf(p: { selected: boolean; detected: boolean; enabled: boolean }): MarkState {
  if (p.selected) return "on";
  if (!p.enabled) return "off";
  if (!p.detected) return "missing";
  return "idle";
}
