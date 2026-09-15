import type { AgentCapabilities, AgentPlugin } from "./ipc";
import { t } from "./i18n";
import { costLabel } from "./format";
import type { ModelTiers } from "../components/ModelSelect";

/**
 * What the UI knows about each agent: its name and capability sheet.
 * Every surface that offers a feature asks here first, so a feature the
 * plugin lacks shows as DISABLED WITH A REASON — never hidden (the user
 * would think Hark lacks it), never a control that silently does nothing.
 */
export type AgentSheet = {
  id: string;
  name: string;
  capabilities: AgentCapabilities | null;
  /** "claude" (native) or "acp". */
  plugin?: string;
  /** What this agent calls each tier; empty when its registry line has no table. */
  models?: Record<string, string>;
};
export type Catalog = Record<string, AgentSheet>;

export function toCatalog(plugins: AgentPlugin[]): Catalog {
  return Object.fromEntries(
    plugins.map((p) => [
      p.id,
      { id: p.id, name: p.name, capabilities: p.capabilities, plugin: p.plugin, models: p.models ?? {} },
    ]),
  );
}

export const TIER_KEYS = ["light", "standard", "heavy", "max"] as const;
export type TierKey = (typeof TIER_KEYS)[number];

/**
 * Which tier a pill value stands for: the tier's own key, or the GLOBAL
 * table's name for it ("haiku" is claude's light). An explicit model id
 * belongs to no tier; so does nothing at all.
 */
export function tierOf(value: string | undefined, global: ModelTiers): TierKey | undefined {
  if (!value) return undefined;
  if ((TIER_KEYS as readonly string[]).includes(value)) return value as TierKey;
  return TIER_KEYS.find((k) => global[k] === value);
}

/**
 * What the model pill offers in a chat on this agent: its own tier table
 * when the registry line has one; the global table for the native claude
 * plugin (that table IS claude's vocabulary) and for any agent nothing is
 * known about; and for an ACP agent with no table, the pill disabled with
 * the config knob as its reason — offering claude's names to a codex is a
 * control that lies. Mirrors `agents::model_id` in the core.
 */
export function modelPillFor(
  catalog: Catalog | undefined,
  agent: string | undefined,
  global: ModelTiers,
): { tiers: ModelTiers; disabled?: string } {
  const sheet = catalog && agent ? catalog[agent] : undefined;
  if (!sheet || !sheet.capabilities) return { tiers: global };
  const table = sheet.models ?? {};
  if (TIER_KEYS.every((k) => table[k])) {
    return { tiers: { light: table.light, standard: table.standard, heavy: table.heavy, max: table.max } };
  }
  if (sheet.plugin === "claude") return { tiers: global };
  return { tiers: global, disabled: t("cap_no_model_table", { name: sheet.name, id: sheet.id }) };
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

/**
 * Why bypass is off in a chat on this agent, or undefined when it may be
 * picked. Bypass on claude runs under a deny floor (kubectl, terraform…
 * cannot run at all); nothing of the kind crosses ACP, so on any other
 * plugin the driver opens acceptEdits instead and the row says so. An
 * agent nothing is known about is not accused.
 */
export function bypassFloorFor(catalog: Catalog | undefined, agent: string | undefined): string | undefined {
  const sheet = catalog && agent ? catalog[agent] : undefined;
  if (!sheet || !sheet.capabilities || !sheet.plugin) return undefined;
  return sheet.plugin === "claude" ? undefined : t("cap_no_bypass_floor", { name: sheet.name });
}
