import { describe, expect, it } from "vitest";
import { costFooter, modelPillFor, tierOf, unsupported, type Catalog } from "./support";
import { DEFAULT_TIERS } from "../components/ModelSelect";
import { t } from "./i18n";

const caps = (over: Partial<Catalog[string]["capabilities"] & object>) => ({
  resume: true,
  permissions: true,
  structured_output: true,
  cost_reporting: true,
  history: true,
  live_list: true,
  slash_commands: true,
  memory_file: null,
  shell_tools: [],
  fork: true,
  directive_mode: true,
  directive_model: true,
  directive_effort: true,
  ...over,
});

const catalog: Catalog = {
  claude: { id: "claude", name: "Claude Code", capabilities: caps({}) },
  gemini: {
    id: "gemini",
    name: "Gemini CLI",
    // Gemini over ACP offers permission modes but no model or effort knob.
    capabilities: caps({
      cost_reporting: false,
      directive_model: false,
      directive_effort: false,
      fork: false,
      history: false,
    }),
  },
  mystery: { id: "mystery", name: "Mystery", capabilities: null },
};

describe("a feature the plugin lacks says so, in the plugin's name", () => {
  it("names the plugin and the feature when the sheet says no", () => {
    const why = unsupported(catalog, "gemini", "directive_effort", "esforço");
    expect(why).toBe(t("cap_unsupported", { name: "Gemini CLI", feature: "esforço" }));
  });

  it("stays silent when the sheet says yes, knob by knob", () => {
    expect(unsupported(catalog, "claude", "directive_model", "x")).toBeUndefined();
    // The same agent can take one directive and not another.
    expect(unsupported(catalog, "gemini", "directive_mode", "x")).toBeUndefined();
    expect(unsupported(catalog, "gemini", "directive_model", "x")).toBeDefined();
  });

  it("never accuses an agent it knows nothing about", () => {
    // No sheet negotiated yet, or an agent not in the catalog: claiming
    // "unsupported" would be a guess, and a wrong one disables a control
    // that works.
    expect(unsupported(catalog, "mystery", "fork", "x")).toBeUndefined();
    expect(unsupported(catalog, "nobody", "fork", "x")).toBeUndefined();
    expect(unsupported(catalog, undefined, "fork", "x")).toBeUndefined();
  });
});

describe("the price slot of a reply", () => {
  it("shows the dollars when the turn was priced", () => {
    expect(costFooter(catalog, { agent: "claude", cost: 0.0123 })).toEqual({ label: "$0.0123" });
  });

  it("holds the slot with a dash when the plugin cannot price, and the hover says which plugin", () => {
    const slot = costFooter(catalog, { agent: "gemini", cost: undefined });
    expect(slot.label).toBe("$ –");
    expect(slot.title).toBe(t("cap_unsupported", { name: "Gemini CLI", feature: t("pl_cap_cost") }));
  });

  it("calls a missing price on a pricing plugin a gap in this turn, not a missing feature", () => {
    const slot = costFooter(catalog, { agent: "claude", cost: undefined });
    expect(slot.label).toBe("$ –");
    expect(slot.title).toBe(t("cost_unknown_turn"));
  });

  it("with no agent at all, still never prints zero", () => {
    const slot = costFooter(catalog, { cost: undefined });
    expect(slot.label).toBe("$ –");
    expect(slot.title).toBe(t("cost_unknown_turn"));
  });
});

describe("the model pill speaks in tiers, each agent in its own names", () => {
  const withTables: Catalog = {
    claude: { ...catalog.claude, plugin: "claude", models: { light: "haiku", standard: "sonnet", heavy: "opus", max: "fable" } },
    gemini: {
      ...catalog.gemini,
      plugin: "acp",
      models: { light: "gemini-2.5-flash-lite", standard: "gemini-2.5-flash", heavy: "gemini-2.5-pro", max: "gemini-2.5-pro" },
      capabilities: caps({ directive_model: true }),
    },
    codex: { id: "codex", name: "Codex CLI", plugin: "acp", models: {}, capabilities: caps({ directive_model: true }) },
    twin: { id: "twin", name: "Claude via gateway", plugin: "claude", models: {}, capabilities: caps({}) },
    mystery: catalog.mystery,
  };

  it("reads a pick as a tier: its key, or the global table's name for it", () => {
    expect(tierOf("light", DEFAULT_TIERS)).toBe("light");
    expect(tierOf("haiku", DEFAULT_TIERS)).toBe("light");
    expect(tierOf("fable", DEFAULT_TIERS)).toBe("max");
    // An explicit id belongs to no tier; nothing picked is no tier either.
    expect(tierOf("gemini-2.5-pro", DEFAULT_TIERS)).toBeUndefined();
    expect(tierOf("", DEFAULT_TIERS)).toBeUndefined();
    expect(tierOf(undefined, DEFAULT_TIERS)).toBeUndefined();
  });

  it("offers gemini's own names in a gemini chat", () => {
    const pill = modelPillFor(withTables, "gemini", DEFAULT_TIERS);
    expect(pill.tiers.light).toBe("gemini-2.5-flash-lite");
    expect(pill.tiers.max).toBe("gemini-2.5-pro");
    expect(pill.disabled).toBeUndefined();
  });

  it("offers the global table to claude, and to a claude twin without a table", () => {
    expect(modelPillFor(withTables, "claude", DEFAULT_TIERS).tiers).toEqual(DEFAULT_TIERS);
    expect(modelPillFor(withTables, "twin", DEFAULT_TIERS).tiers).toEqual(DEFAULT_TIERS);
  });

  it("disables the pill for an ACP agent with no table, naming the config knob", () => {
    // Claude's names would be refused or ignored by codex; the pill says
    // so rather than offering them. The driver sends no model either.
    const pill = modelPillFor(withTables, "codex", DEFAULT_TIERS);
    expect(pill.disabled).toBe(t("cap_no_model_table", { name: "Codex CLI", id: "codex" }));
  });

  it("never disables the pill for an agent it knows nothing about", () => {
    expect(modelPillFor(withTables, "mystery", DEFAULT_TIERS)).toEqual({ tiers: DEFAULT_TIERS });
    expect(modelPillFor(withTables, "nobody", DEFAULT_TIERS)).toEqual({ tiers: DEFAULT_TIERS });
    expect(modelPillFor(undefined, "gemini", DEFAULT_TIERS)).toEqual({ tiers: DEFAULT_TIERS });
  });
});
