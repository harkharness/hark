import type { AgentCapabilities, AgentPlugin } from "./ipc";
import { t } from "./i18n";
import { costLabel } from "./format";

/**
 * What the UI knows about each agent: its name and capability sheet.
 * Every surface that offers a feature asks here first, so a feature the
 * plugin lacks shows as DISABLED WITH A REASON — never hidden (the user
 * would think Hark lacks it), never a control that silently does nothing.
 */
export type AgentSheet = { id: string; name: string; capabilities: AgentCapabilities | null };
export type Catalog = Record<string, AgentSheet>;

export function toCatalog(plugins: AgentPlugin[]): Catalog {
  return Object.fromEntries(
    plugins.map((p) => [p.id, { id: p.id, name: p.name, capabilities: p.capabilities }]),
  );
}

/**
 * The reason a feature is off for this agent, in the plugin's own name —
 * or undefined when it works. Unknown agent or no sheet yet: undefined
 * too. Claiming "unsupported" would be a guess, and a wrong guess
 * disables a control that works.
 */
export function unsupported(
  catalog: Catalog | undefined,
  agent: string | undefined,
  cap: keyof AgentCapabilities,
  feature: string,
): string | undefined {
  if (!catalog || !agent) return undefined;
  const sheet = catalog[agent];
  if (!sheet?.capabilities) return undefined;
  return sheet.capabilities[cap] === false
    ? t("cap_unsupported", { name: sheet.name, feature })
    : undefined;
}

/**
 * The price slot of a reply's footer. Priced: the dollars. Unpriced: the
 * slot is held with a dash and the hover says WHY — the plugin cannot
 * price at all, or it can and this turn simply came without one.
 */
export function costFooter(
  catalog: Catalog | undefined,
  m: { agent?: string; cost?: number },
): { label: string; title?: string } {
  if (m.cost !== undefined) return { label: costLabel(m.cost) };
  const why = unsupported(catalog, m.agent, "cost_reporting", t("pl_cap_cost"));
  return { label: costLabel(undefined), title: why ?? t("cost_unknown_turn") };
}
