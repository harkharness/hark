import { describe, expect, it } from "vitest";
import { costFooter, unsupported, type Catalog } from "./support";
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
  directives: true,
  ...over,
});

const catalog: Catalog = {
  claude: { id: "claude", name: "Claude Code", capabilities: caps({}) },
  gemini: {
    id: "gemini",
    name: "Gemini CLI",
    capabilities: caps({ cost_reporting: false, directives: false, fork: false, history: false }),
  },
  mystery: { id: "mystery", name: "Mystery", capabilities: null },
};

describe("a feature the plugin lacks says so, in the plugin's name", () => {
  it("names the plugin and the feature when the sheet says no", () => {
    const why = unsupported(catalog, "gemini", "directives", "modo, modelo e esforço");
    expect(why).toBe(t("cap_unsupported", { name: "Gemini CLI", feature: "modo, modelo e esforço" }));
  });

  it("stays silent when the sheet says yes", () => {
    expect(unsupported(catalog, "claude", "directives", "x")).toBeUndefined();
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
