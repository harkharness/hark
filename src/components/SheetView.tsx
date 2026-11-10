import { useCallback, useEffect, useRef, useState } from "react";
import { Check, Copy, History, Plus, Sparkles, Undo2, X } from "lucide-react";
import * as ipc from "../lib/ipc";
import { t } from "../lib/i18n";
import type { CellRef, SheetFormat, SheetOp, SheetView as View } from "../types";

/** A column index as its letter name: 0 → A, 26 → AA. */
export function columnName(i: number): string {
  let n = i;
  let out = "";
  for (;;) {
    out = String.fromCharCode(65 + (n % 26)) + out;
    if (n < 26) break;
    n = Math.floor(n / 26) - 1;
  }
  return out;
}

/** One tab as csv, values not formulas, quoted the usual way. */
export function tabCsv(tab: View["computed"]["tabs"][number]): string {
  const q = (s: string) => (/[",\n]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s);
  const lines = [tab.columns.map(q).join(",")];
  for (const row of tab.cells) lines.push(row.map((c) => q(c.text)).join(","));
  return lines.join("\n") + "\n";
}

type Cell = View["computed"]["tabs"][number]["cells"][number][number];

/** A cell as the grid shows it: its number in the column's format, else its text. */
export function shown(c: Cell, format?: SheetFormat | null): string {
  if (c.number == null || c.error) return c.text;
  const n = c.number;
  switch (format) {
    case "money":
      return n.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 });
    case "pct":
      return `${n.toLocaleString(undefined, { maximumFractionDigits: 1 })}%`;
    case "int":
      return Math.round(n).toLocaleString();
    case "text":
      return c.text;
    default:
      return n.toLocaleString(undefined, { maximumFractionDigits: 2 });
  }
}

const FORMATS: (SheetFormat | null)[] = [null, "money", "pct", "int", "date", "text"];

type Menu = { kind: "col"; col: number } | { kind: "row"; row: number } | { kind: "tab"; tab: number };

/**
 * The sheet page: tabs, a formula bar, the grid Hark computed (zero
 * tokens), the cells the agent changed since you last accepted marked
 * green with accept / undo above, and your own edits written straight
 * to the file: a cell on a double click; a column's letter opens its
 * menu (insert, remove, format), dragging the letter moves the column,
 * its edge resizes it; a row number opens the row menu; a tab is
 * renamed on a double click. Numbers are formatted in the window's
 * locale, in the column's format when it has one.
 */
export default function SheetView({ space, id, onChanged }: { space: string; id: string; onChanged?: () => void }) {
  const [view, setView] = useState<View | null>(null);
  const [tab, setTab] = useState(0);
  const [sel, setSel] = useState<{ row: number; col: number } | null>(null);
  const [editing, setEditing] = useState<{ row: number; col: number; value: string } | null>(null);
  const [menu, setMenu] = useState<Menu | null>(null);
  const [renaming, setRenaming] = useState<{ tab: number; value: string } | null>(null);
  const [newTab, setNewTab] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const resize = useRef<{ col: number; startX: number; width: number; now: number } | null>(null);
  const [resizing, setResizing] = useState<{ col: number; width: number } | null>(null);

  const load = useCallback(async () => {
    try {
      setView(await ipc.sheetView(space, id));
      setError(null);
    } catch (err) {
      setError(String(err));
    }
  }, [space, id]);
  useEffect(() => {
    void load();
  }, [load]);

  const current = view?.computed.tabs[tab] ?? view?.computed.tabs[0];
  const tabIndex = view?.computed.tabs[tab] ? tab : 0;
  const changedHere = (view?.changed ?? []).filter((c) => c.tab === tabIndex);
  const isChanged = (row: number, col: number) => changedHere.some((c) => c.row === row && c.col === col);
  const selected = sel && current ? current.cells[sel.row]?.[sel.col] : undefined;

  async function accept() {
    await ipc.artifactAccept(space, id);
    await load();
    onChanged?.();
  }
  async function undo() {
    const versions = await ipc.artifactVersions(space, id);
    const last = versions.at(-1);
    if (!last) return;
    await ipc.artifactRestore(space, id, last.at);
    await load();
    onChanged?.();
  }
  async function commit(at: CellRef, typed: string) {
    setEditing(null);
    try {
      await ipc.sheetSetCell(space, id, at, typed);
      await load();
      onChanged?.();
    } catch (err) {
      setError(String(err));
    }
  }
  async function edit(op: SheetOp) {
    setMenu(null);
    try {
      await ipc.sheetEdit(space, id, op);
      await load();
      onChanged?.();
    } catch (err) {
      setError(String(err));
    }
  }
  async function copyCsv() {
    if (!current) return;
    await navigator.clipboard.writeText(tabCsv(current));
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  }

  // The column's edge: drag to resize, written on release.
  function startResize(e: React.MouseEvent, col: number, width: number) {
    e.preventDefault();
    e.stopPropagation();
    resize.current = { col, startX: e.clientX, width, now: width };
    setResizing({ col, width });
    const onMove = (ev: MouseEvent) => {
      if (!resize.current) return;
      resize.current.now = Math.max(40, resize.current.width + ev.clientX - resize.current.startX);
      setResizing({ col, width: resize.current.now });
    };
    const onUp = () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
      const r = resize.current;
      resize.current = null;
      setResizing(null);
      if (r && r.now !== r.width) void edit({ op: "set_width", tab: tabIndex, col: r.col, width: Math.round(r.now) });
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
  }

  if (error) return <div className="sheet-error">{t("sheet_open_err", { e: error })}</div>;
  if (!view) return <div className="sheet-loading dim">…</div>;
  if (view.error) {
    return (
      <div className="sheet-broken">
        <div className="sheet-error">{t("sheet_file_err", { e: view.error })}</div>
        <pre className="mono">{view.raw}</pre>
      </div>
    );
  }
  if (!current) return <div className="sheet-empty dim">{t("sheet_no_tabs")}</div>;

  const widthOf = (c: number) => (resizing?.col === c ? resizing.width : current.widths?.[c] ?? undefined);
  const formatOf = (c: number) => current.formats?.[c] ?? null;
  const colStyle = (c: number) => {
    const w = widthOf(c);
    return w ? { width: `${w}px`, minWidth: `${w}px`, maxWidth: `${w}px` } : undefined;
  };
  const rows = current.cells.length;

  const editor = (r: number, c: number) => (
    <input
      autoFocus
      size={1}
      className="sheet-input mono"
      value={editing?.value ?? ""}
      onChange={(e) => editing && setEditing({ ...editing, value: e.target.value })}
      onKeyDown={(e) => {
        if (e.key === "Enter" && editing) void commit({ tab: tabIndex, row: r, col: c }, editing.value);
        if (e.key === "Escape") setEditing(null);
      }}
      onBlur={() => editing && void commit({ tab: tabIndex, row: r, col: c }, editing.value)}
    />
  );

  return (
    <div className="sheet-view" onClick={() => menu && setMenu(null)}>
      {view.changed.length > 0 && (
        <div className="sheet-banner">
          <Sparkles size={13} />
          <b>{t("sheet_changed_n", { n: view.changed.length })}</b>
          <span className="dim">{t("sheet_changed_hint")}</span>
          <span className="sheet-banner-acts">
            <button className="pill-btn" onClick={() => void undo()} title={t("sheet_undo_hint")}>
              <Undo2 size={11} /> {t("sheet_undo")}
            </button>
            <button className="pill-btn on" onClick={() => void accept()} title={t("sheet_accept_hint")}>
              <Check size={11} /> {t("sheet_accept")}
            </button>
          </span>
        </div>
      )}
      <div className="sheet-tabs">
        {view.computed.tabs.map((tb, i) =>
          renaming?.tab === i ? (
            <input
              key={`r${i}`}
              autoFocus
              className="sheet-tab-input"
              value={renaming.value}
              onChange={(e) => setRenaming({ tab: i, value: e.target.value })}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  const name = renaming.value.trim();
                  setRenaming(null);
                  if (name && name !== tb.name) void edit({ op: "rename_tab", tab: i, name });
                }
                if (e.key === "Escape") setRenaming(null);
              }}
              onBlur={() => setRenaming(null)}
            />
          ) : (
            <button
              key={tb.name}
              className={`sheet-tab ${i === tabIndex ? "on" : ""}`}
              title={t("sheet_rename_hint")}
              onClick={() => { setTab(i); setSel(null); setEditing(null); setMenu(null); }}
              onDoubleClick={() => setRenaming({ tab: i, value: tb.name })}
              onContextMenu={(e) => { e.preventDefault(); setTab(i); setMenu({ kind: "tab", tab: i }); }}
            >
              {tb.name}
              {i === tabIndex && view.computed.tabs.length > 1 && (
                <span className="sheet-tab-x" title={t("sheet_remove_tab")} onClick={(e) => { e.stopPropagation(); void edit({ op: "remove_tab", tab: i }); }}>
                  <X size={9} />
                </span>
              )}
            </button>
          ),
        )}
        {newTab != null ? (
          <input
            autoFocus
            className="sheet-tab-input"
            placeholder={t("sheet_tab_name")}
            value={newTab}
            onChange={(e) => setNewTab(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                const name = newTab.trim();
                setNewTab(null);
                if (name) void edit({ op: "add_tab", name });
              }
              if (e.key === "Escape") setNewTab(null);
            }}
            onBlur={() => setNewTab(null)}
          />
        ) : (
          <button className="sheet-tab add" onClick={() => setNewTab("")} title={t("sheet_add_tab")}>
            <Plus size={11} /> {t("sheet_add_tab")}
          </button>
        )}
        <span className="sheet-tabs-acts">
          <button className="pill-btn" onClick={() => void copyCsv()} title={t("sheet_copy_csv_hint")}>
            <Copy size={11} /> {copied ? t("share_copied") : t("sheet_copy_csv")}
          </button>
          <span className="dim" title={t("sheet_versions_hint")}>
            <History size={11} /> {t("sheet_versions", { n: view.versions })}
          </span>
        </span>
      </div>
      <div className="sheet-bar">
        <span className="mono sheet-ref">{sel ? `${columnName(sel.col)}${sel.row + 2}` : ""}</span>
        <span className={`mono sheet-formula ${selected?.formula ? "f" : ""}`}>{selected?.raw ?? ""}</span>
        <span className="dim sheet-bar-hint" title={t("sheet_col_menu_hint")}>{t("sheet_formula_hint")}</span>
      </div>
      <div className="sheet-grid-wrap">
        <table className="sheet-grid">
          <thead>
            <tr>
              <th className="rh" />
              {current.columns.map((_, c) => (
                <th
                  key={c}
                  className={`ch ${menu?.kind === "col" && menu.col === c ? "on" : ""}`}
                  style={colStyle(c)}
                  draggable
                  onDragStart={(e) => { e.dataTransfer.setData("text/hark-col", String(c)); e.dataTransfer.effectAllowed = "move"; }}
                  onDragOver={(e) => e.preventDefault()}
                  onDrop={(e) => {
                    e.preventDefault();
                    const from = Number(e.dataTransfer.getData("text/hark-col"));
                    if (Number.isInteger(from) && from !== c) void edit({ op: "move_col", tab: tabIndex, from, to: c });
                  }}
                  onClick={(e) => { e.stopPropagation(); setMenu(menu?.kind === "col" && menu.col === c ? null : { kind: "col", col: c }); }}
                >
                  {columnName(c)}
                  {formatOf(c) && <span className="sheet-fmt-tag">{t(`sheet_fmt_${formatOf(c) as SheetFormat}`)}</span>}
                  <span className="sheet-col-edge" onMouseDown={(e) => startResize(e, c, widthOf(c) ?? (e.currentTarget.parentElement?.offsetWidth ?? 90))} />
                  {menu?.kind === "col" && menu.col === c && (
                    <div className="sheet-menu" onClick={(e) => e.stopPropagation()}>
                      <button onClick={() => void edit({ op: "insert_col", tab: tabIndex, at: c })}>{t("sheet_insert_left")}</button>
                      <button onClick={() => void edit({ op: "insert_col", tab: tabIndex, at: c + 1 })}>{t("sheet_insert_right")}</button>
                      <button onClick={() => void edit({ op: "remove_col", tab: tabIndex, at: c })}>{t("sheet_remove_col")}</button>
                      <div className="sheet-menu-title dim">{t("sheet_format")}</div>
                      {FORMATS.map((f) => (
                        <button key={f ?? "none"} className={formatOf(c) === f ? "on" : ""} onClick={() => void edit({ op: "set_format", tab: tabIndex, col: c, format: f })}>
                          {t(`sheet_fmt_${f ?? "none"}`)}
                        </button>
                      ))}
                    </div>
                  )}
                </th>
              ))}
            </tr>
            <tr>
              <th className="rh">1</th>
              {current.columns.map((name, c) => (
                <th key={c} className="hd" style={colStyle(c)}>{name}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {current.cells.map((row, r) => (
              <tr key={r}>
                <th className={`rh ${menu?.kind === "row" && menu.row === r ? "on" : ""}`} onClick={(e) => { e.stopPropagation(); setMenu(menu?.kind === "row" && menu.row === r ? null : { kind: "row", row: r }); }}>
                  {r + 2}
                  {menu?.kind === "row" && menu.row === r && (
                    <div className="sheet-menu" onClick={(e) => e.stopPropagation()}>
                      <button onClick={() => void edit({ op: "insert_row", tab: tabIndex, at: r })}>{t("sheet_insert_above")}</button>
                      <button onClick={() => void edit({ op: "insert_row", tab: tabIndex, at: r + 1 })}>{t("sheet_insert_below")}</button>
                      <button onClick={() => void edit({ op: "remove_row", tab: tabIndex, at: r })}>{t("sheet_remove_row")}</button>
                    </div>
                  )}
                </th>
                {current.columns.map((_, c) => {
                  const cell = row[c] ?? { raw: "", text: "", formula: false };
                  const on = sel?.row === r && sel?.col === c;
                  const edit_ = editing?.row === r && editing?.col === c;
                  return (
                    <td
                      key={c}
                      className={`${cell.number != null ? "num" : ""} ${cell.formula ? "f" : ""} ${isChanged(r, c) ? "changed" : ""} ${on ? "sel" : ""} ${cell.error ? "err" : ""} ${edit_ ? "editing" : ""}`}
                      style={colStyle(c)}
                      title={cell.error ?? (cell.formula ? cell.raw : undefined)}
                      onClick={() => setSel({ row: r, col: c })}
                      onDoubleClick={() => setEditing({ row: r, col: c, value: cell.raw })}
                    >
                      {edit_ ? editor(r, c) : shown(cell, formatOf(c))}
                    </td>
                  );
                })}
              </tr>
            ))}
            <tr className="sheet-add">
              <th className="rh">{rows + 2}</th>
              {current.columns.map((_, c) => (
                <td key={c} className={`empty ${editing?.row === rows && editing.col === c ? "editing" : ""}`} style={colStyle(c)} onDoubleClick={() => setEditing({ row: rows, col: c, value: "" })}>
                  {editing?.row === rows && editing.col === c ? editor(rows, c) : null}
                </td>
              ))}
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  );
}
