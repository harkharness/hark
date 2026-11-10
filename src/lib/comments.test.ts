import { describe, expect, it } from "vitest";
import { anchorLabel, mentionsHark, sameAnchor } from "./comments";
import { t } from "./i18n";

describe("a comment's mention and its anchor", () => {
  it("knows a mention of hark as a word", () => {
    expect(mentionsHark("@hark troca o título")).toBe(true);
    expect(mentionsHark("troca, @Hark!")).toBe(true);
    expect(mentionsHark("@harkness")).toBe(false);
    expect(mentionsHark("hark@example.com")).toBe(false);
    expect(mentionsHark("nada")).toBe(false);
  });
  it("reads an anchor the way the person sees the page", () => {
    expect(anchorLabel({ kind: "cell", tab: 0, row: 2, col: 1, tab_name: "Mensal" })).toBe(t("cm_anchor_cell", { c: "B4", t: "Mensal" }));
    expect(anchorLabel({ kind: "slide", n: 2 })).toBe(t("cm_anchor_slide", { n: 3 }));
    expect(anchorLabel({ kind: "quote", text: "um   trecho\n citado" })).toBe(t("cm_anchor_quote", { q: "um trecho citado" }));
    expect(sameAnchor({ kind: "cell", tab: 0, row: 1, col: 1 }, { kind: "cell", tab: 0, row: 1, col: 1 })).toBe(true);
    expect(sameAnchor({ kind: "note", id: "n1" }, { kind: "note", id: "n2" })).toBe(false);
    expect(anchorLabel({ kind: "region", x: 120.4, y: 80, w: 300, h: 48, slide: 1, over: " Billing v2 " })).toBe(
      `${t("cm_anchor_region", { x: "120", y: "80", w: "300", h: "48" })}${t("cm_anchor_region_slide", { n: 2 })}${t("cm_anchor_over", { q: "Billing v2" })}`,
    );
    expect(anchorLabel({ kind: "region", x: 10, y: 10, w: 0, h: 0 })).toBe(t("cm_anchor_point", { x: "10", y: "10" }));
    expect(sameAnchor({ kind: "region", x: 1, y: 2, w: 3, h: 4, slide: 0 }, { kind: "region", x: 1, y: 2, w: 3, h: 4, slide: 0 })).toBe(true);
  });
});
