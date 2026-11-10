import { describe, expect, it } from "vitest";
import { boardPreview, dashPreview, docPreview, sheetPreview } from "./thumbs";

describe("a card's thumbnail is the page at scale, read from its file", () => {
  it("a sheet shows its first tab's first cells, formulas as text", () => {
    const p = sheetPreview(`{"title":"Custos","tabs":[{"name":"Mensal","columns":["mês","infra","db","total","extra"],"rows":[["set",412,96,"=B2+C2",1],["out",438,101,"=B3+C3",2],["nov",451,104,"=B4+C4",3],["dez",1,2,3,4],["jan",5,6,7,8]]},{"name":"Por serviço","columns":["x"],"rows":[]}]}`);
    expect(p).not.toBeNull();
    expect(p!.columns).toEqual(["mês", "infra", "db", "total"]);
    expect(p!.rows.length).toBe(4);
    expect(p!.rows[0]).toEqual([{ text: "set", num: false, formula: false }, { text: "412", num: true, formula: false }, { text: "96", num: true, formula: false }, { text: "=B2+C2", num: false, formula: true }]);
    expect(p!.tabs).toEqual(["Mensal", "Por serviço"]);
    expect(sheetPreview("not json")).toBeNull();
    expect(sheetPreview("{}")).toBeNull();
  });

  it("a board becomes rectangles at their places, scaled to the box; notes without a place fall into a grid", () => {
    const p = boardPreview(`{"groups":[{"id":"g1","title":"A","x":0,"y":0,"w":400,"h":300}],"notes":[{"id":"n1","text":"x","x":20,"y":20,"color":"blue","group":"g1"},{"id":"n2","text":"y","x":600,"y":500,"color":"pink"},{"id":"n3","text":"z"}],"links":[{"from":"n1","to":"n2"}],"timeline":[{"date":"2026-11-12","title":"a"},{"date":"2026-12-03","title":"b"}]}`, 240, 80);
    expect(p).not.toBeNull();
    expect(p!.notes.length).toBe(3);
    const n2 = p!.notes.find((n) => n.id === "n2")!;
    expect(n2.color).toBe("pink");
    expect(n2.x).toBeGreaterThan(p!.notes[0].x);
    expect(n2.x + n2.w).toBeLessThanOrEqual(240);
    expect(n2.y + n2.h).toBeLessThanOrEqual(80);
    expect(p!.groups[0].title).toBe("A");
    expect(p!.links.length).toBe(1);
    expect(p!.milestones).toBe(2);
    // Notes and groups without a place are laid out as the viewer does:
    // groups side by side, two notes per row, so the thumb is never a
    // tall thin strip of dots.
    const laid = boardPreview(`{"groups":[{"id":"g1","title":"A"},{"id":"g2","title":"B"}],"notes":[{"id":"a","text":"1","group":"g1"},{"id":"b","text":"2","group":"g1"},{"id":"c","text":"3","group":"g1"},{"id":"d","text":"4","group":"g2"},{"id":"e","text":"5"}]}`, 240, 80)!;
    expect(laid.groups[1].x).toBeGreaterThan(laid.groups[0].x + laid.groups[0].w - 1);
    expect(laid.groups[0].w).toBeGreaterThan(40);
    const a = laid.notes.find((n) => n.id === "a")!;
    const c = laid.notes.find((n) => n.id === "c")!;
    expect(c.y).toBeGreaterThan(a.y);
    expect(a.w).toBeGreaterThan(14);
    // Fifteen groups side by side (as the viewer lays them) are re-packed
    // into rows that fit the box, and a note never shrinks below a few pixels.
    const wide = { groups: [] as unknown[], notes: [] as unknown[] };
    for (let i = 0; i < 15; i++) {
      wide.groups.push({ id: `g${i}`, title: `G${i}`, x: 24 + i * 366, y: 24, w: 338, h: 246 });
      wide.notes.push({ id: `n${i}`, text: "x", x: 48 + i * 366, y: 60, group: `g${i}` });
    }
    const packed = boardPreview(JSON.stringify(wide), 240, 80)!;
    const rowsY = new Set(packed.groups.map((g) => Math.round(g.y)));
    expect(rowsY.size).toBeGreaterThan(1);
    expect(Math.min(...packed.notes.map((n) => n.w))).toBeGreaterThanOrEqual(12);
    expect(boardPreview("{}", 240, 80)!.notes.length).toBe(0);
    expect(boardPreview("nope", 240, 80)).toBeNull();
  });

  it("a dashboard becomes one tile per panel, by its chart", () => {
    const p = dashPreview(`{"sources":[],"queries":[],"panels":[{"title":"ativos","query":"a","chart":"number"},{"title":"por dia","query":"a","chart":"bar"},{"title":"ret","query":"a","chart":"line"},{"title":"tudo","query":"a"}]}`);
    expect(p!.map((t) => t.chart)).toEqual(["number", "bar", "line", "table"]);
    expect(p![0].title).toBe("ativos");
    expect(dashPreview("{}")).toEqual([]);
    expect(dashPreview("x")).toBeNull();
  });

  it("a document shows its title and opening lines without markdown marks", () => {
    const p = docPreview(`---\ntitle: Home\n---\n# Home do espaço\n\nEscopo da **frente** de [plataforma](x). Esta página é a *fonte*.\n\n## Onde começar\n- [Decisões](y): os ADRs\n- outro item\n\n\`\`\`sh\nls\n\`\`\`\n`, "Home");
    expect(p.title).toBe("Home do espaço");
    expect(p.lines[0]).toBe("Escopo da frente de plataforma. Esta página é a fonte.");
    expect(p.lines).toContain("Onde começar");
    expect(p.lines).toContain("Decisões: os ADRs");
    expect(p.lines.join(" ")).not.toContain("```");
    expect(docPreview("", "Sem título").title).toBe("Sem título");
  });
});
