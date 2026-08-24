import type { Directives } from "../types";
import { t } from "./i18n";

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

/**
 * Backend errors carry a code when the diagnosis is known ("agent_missing:
 * /path (os error 2)"). Show the sentence a person can act on, and keep
 * the technical tail — the path or the CLI's own words are usually what
 * identifies the real problem.
 */
export function agentError(err: unknown): string {
  const raw = String(err);
  const known = ["agent_missing", "agent_blocked", "agent_auth", "agent_failed"] as const;
  for (const code of known) {
    if (!raw.startsWith(`${code}:`)) continue;
    const detail = raw.slice(code.length + 1).trim();
    const head = t(code);
    return code === "agent_failed" ? `${head} ${detail}` : `${head} (${detail})`;
  }
  return raw;
}
