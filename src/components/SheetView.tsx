import { useCallback, useEffect, useState } from "react";
import { Check, Copy, History, Sparkles, Undo2 } from "lucide-react";
import * as ipc from "../lib/ipc";
import { t } from "../lib/i18n";
import type { CellRef, SheetView as View } from "../types";

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

/**
 * The sheet page: tabs, a formula bar, the grid Hark computed (zero
 * tokens), the cells the agent changed since you last accepted marked
 * green with accept / undo above, and your own edits written straight
 * to the file. Numbers are formatted in the window's locale.
 */
export default function SheetView({ space, id, onChanged }: { space: string; id: string; onChanged?: () => void }) {
  const [view, setView] = useState<View | null>(null);
  const [tab, setTab] = useState(0);
  const [sel, setSel] = useState<{ row: number; col: number } | null>(null);
  const [editing, setEditing] = useState<{ row: number; col: number; value: string } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

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
  async function copyCsv() {
    if (!current) return;
    await navigator.clipboard.writeText(tabCsv(current));
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
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

  const fmt = (c: View["computed"]["tabs"][number]["cells"][number][number]) =>
    c.number != null && !c.error ? c.number.toLocaleString(undefined, { maximumFractionDigits: 2 }) : c.text;

  return (
    <div className="sheet-view">
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
        {view.computed.tabs.map((tb, i) => (
          <button key={tb.name} className={`sheet-tab ${i === tabIndex ? "on" : ""}`} onClick={() => { setTab(i); setSel(null); setEditing(null); }}>
            {tb.name}
          </button>
        ))}
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
        <span className="dim sheet-bar-hint">{t("sheet_formula_hint")}</span>
      </div>
      <div className="sheet-grid-wrap">
        <table className="sheet-grid">
          <thead>
            <tr>
              <th className="rh" />
              {current.columns.map((_, c) => (
                <th key={c} className="ch">{columnName(c)}</th>
              ))}
            </tr>
            <tr>
              <th className="rh">1</th>
              {current.columns.map((name, c) => (
                <th key={c} className="hd">{name}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {current.cells.map((row, r) => (
              <tr key={r}>
                <th className="rh">{r + 2}</th>
                {current.columns.map((_, c) => {
                  const cell = row[c] ?? { raw: "", text: "", formula: false };
                  const on = sel?.row === r && sel?.col === c;
                  const edit = editing?.row === r && editing?.col === c;
                  return (
                    <td
                      key={c}
                      className={`${cell.number != null ? "num" : ""} ${cell.formula ? "f" : ""} ${isChanged(r, c) ? "changed" : ""} ${on ? "sel" : ""} ${cell.error ? "err" : ""}`}
                      title={cell.error ?? (cell.formula ? cell.raw : undefined)}
                      onClick={() => setSel({ row: r, col: c })}
                      onDoubleClick={() => setEditing({ row: r, col: c, value: cell.raw })}
                    >
                      {edit ? (
                        <input
                          autoFocus
                          className="sheet-input mono"
                          value={editing.value}
                          onChange={(e) => setEditing({ ...editing, value: e.target.value })}
                          onKeyDown={(e) => {
                            if (e.key === "Enter") void commit({ tab: tabIndex, row: r, col: c }, editing.value);
                            if (e.key === "Escape") setEditing(null);
                          }}
                          onBlur={() => void commit({ tab: tabIndex, row: r, col: c }, editing.value)}
                        />
                      ) : (
                        fmt(cell)
                      )}
                    </td>
                  );
                })}
              </tr>
            ))}
            <tr className="sheet-add">
              <th className="rh">{current.cells.length + 2}</th>
              {current.columns.map((_, c) => (
                <td key={c} className="empty" onDoubleClick={() => setEditing({ row: current.cells.length, col: c, value: "" })}>
                  {editing?.row === current.cells.length && editing.col === c ? (
                    <input
                      autoFocus
                      className="sheet-input mono"
                      value={editing.value}
                      onChange={(e) => setEditing({ ...editing, value: e.target.value })}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") void commit({ tab: tabIndex, row: current.cells.length, col: c }, editing.value);
                        if (e.key === "Escape") setEditing(null);
                      }}
                      onBlur={() => void commit({ tab: tabIndex, row: current.cells.length, col: c }, editing.value)}
                    />
                  ) : null}
                </td>
              ))}
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  );
}
