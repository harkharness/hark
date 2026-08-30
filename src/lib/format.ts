import type { Directives, Reply } from "../types";
import { t } from "./i18n";

/** ONE reply→message mapping for every surface (mother, project, HUD echo):
 *  `fala` is the spoken headline; the screen also gets detalhes + itens.
 *  Three hand-rolled copies of this is how the mother once dropped the list
 *  the user asked for. */
export function askReplyMsg(reply: {
  fala: string;
  detalhes?: string;
  itens?: string[];
  cost_usd?: number;
  model?: string;
}): { text: string; detalhes?: string; itens?: string[]; cost?: number; model?: string } {
  const detalhes = reply.detalhes?.trim();
  return {
    text: reply.fala,
    // Detalhes that merely repeat the headline add nothing on screen.
    detalhes: detalhes && detalhes !== reply.fala.trim() ? detalhes : undefined,
    itens: reply.itens?.length ? reply.itens : undefined,
    cost: reply.cost_usd,
    model: reply.model,
  };
}

/** The full reply for echo payloads (HUD → mother thread). */
export function echoReply(reply: Reply) {
  return {
    fala: reply.fala,
    detalhes: reply.detalhes,
    itens: reply.itens,
    cost_usd: reply.cost_usd,
    model: reply.model,
  };
}

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
/**
 * Is this failure an authentication one? The COD source of truth is the
 * backend's health code (agent_auth) riding the error/event; the phrase
 * list is the fallback mirror of health.rs for errors that reach the
 * front uncoded (an older backend, a raw CLI result) — one classifier,
 * used by every surface, so typed and spoken input fail identically.
 */
export function isAuthError(raw: unknown): boolean {
  const s = String(raw).toLowerCase();
  if (s.includes("agent_auth")) return true;
  return [
    "oauth",
    "failed to authenticate",
    "authentication failed",
    "not logged in",
    "please log in",
    "invalid api key",
    "unauthorized",
  ].some((p) => s.includes(p));
}

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
