import type { Directives } from "../types";

const MODE_LABEL: Record<string, string> = {
  manual: "manual",
  acceptEdits: "edições ok",
  plan: "plano",
  auto: "auto",
  bypass: "sem trava",
};

/** Session directives as short footer chips, empty when using defaults. */
export function directiveLabels(d?: Directives): string[] {
  if (!d) return [];
  return [
    d.mode ? MODE_LABEL[d.mode] : undefined,
    d.effort ? `esforço ${d.effort}` : undefined,
  ].filter((x): x is string => !!x);
}

/** Format the model id for the footer: "claude-sonnet-5" -> "sonnet-5". */
export function shortModel(model?: string): string {
  if (!model) return "?";
  return model.replace(/^claude-/, "");
}
