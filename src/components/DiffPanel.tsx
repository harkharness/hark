import { useEffect, useMemo, useState } from "react";
import { Check, ChevronRight, FileText, GitBranch, RefreshCw } from "lucide-react";
import * as ipc from "../lib/ipc";
import { t } from "../lib/i18n";
import { highlightFile } from "../lib/highlight";
import { isSeen, markSeen, seenKey, splitPath, totals, type Seen } from "../lib/diff";
import type { FileChange, RepoState } from "../types";

function readSeen(root: string): Seen {
  try {
    const raw = window.localStorage.getItem(seenKey(root));
    return raw ? (JSON.parse(raw) as Seen) : {};
  } catch {
    return {};
  }
}

/**
 * The diff of the workspace on screen: every changed file against HEAD
 * (untracked ones too), the lines of the one selected, and a tick per
 * file you have read — kept on this machine, cleared by the next edit of
 * that file. A readout over `git`, zero tokens; it commits nothing.
 */
export default function DiffPanel({
  workspace,
  repo,
  tick,
  onOpenPath,
}: {
  /** The task's workspace (git resolves the root). */
  workspace: string;
  repo?: RepoState;
  /** Bumped when a worker wrote a file: the list re-reads. */
  tick: number;
  /** Open the file in the files window. */
  onOpenPath: (abs: string) => void;
}) {
  const [root, setRoot] = useState("");
  const [changes, setChanges] = useState<FileChange[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [diff, setDiff] = useState<string>("");
  const [seen, setSeen] = useState<Seen>({});
  const [error, setError] = useState<string | null>(null);

  async function load() {
    try {
      const out = await ipc.repoChanges(workspace);
      setRoot(out.root);
      setChanges(out.changes);
      setSeen(readSeen(out.root));
      setError(null);
      if (selected && !out.changes.some((c) => c.path === selected)) setSelected(null);
    } catch (err) {
      setError(String(err));
      setChanges([]);
    }
  }
  useEffect(() => {
    void load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [workspace, tick]);

  useEffect(() => {
    if (!selected) {
      setDiff("");
      return;
    }
    let live = true;
    ipc
      .repoDiff(workspace, selected)
      .then((text) => live && setDiff(text))
      .catch((err) => live && setDiff(String(err)));
    return () => {
      live = false;
    };
  }, [workspace, selected, tick]);

  const sum = useMemo(() => totals(changes), [changes]);
  const current = changes.find((c) => c.path === selected);

  function toggleSeen(c: FileChange) {
    const next = isSeen(seen, c) ? Object.fromEntries(Object.entries(seen).filter(([k]) => k !== c.path)) : markSeen(seen, c);
    setSeen(next);
    try {
      window.localStorage.setItem(seenKey(root), JSON.stringify(next));
    } catch {
      /* no storage: the tick lives for this render only */
    }
  }

  return (
    <div className="diff-panel">
      <div className="diff-head">
        <span className="pill-btn" title={root}>
          <GitBranch size={11} /> {repo?.branch || t("repo_detached")} → {t("diff_tree")}
        </span>
        <span className="dim diff-sum">
          {t("diff_files_n", { n: sum.files })} · <span className="add">+{sum.added}</span> <span className="del">−{sum.removed}</span>
        </span>
        <button className="frame-ctl" onClick={() => void load()} title={t("diff_refresh")}>
          <RefreshCw size={12} />
        </button>
      </div>
      {error && <div className="diff-empty">{error}</div>}
      {!error && changes.length === 0 && <div className="diff-empty">{t("diff_clean")}</div>}
      <div className="diff-list">
        {changes.map((c) => {
          const { name, dir } = splitPath(c.path);
          const on = selected === c.path;
          return (
            <div key={c.path} className={`diff-row ${on ? "on" : ""}`} onClick={() => setSelected(on ? null : c.path)} title={c.path}>
              <ChevronRight size={11} className={`diff-caret ${on ? "open" : ""}`} />
              <FileText size={12} className="dim" />
              <span className="diff-name">{name}</span>
              {dir && <span className="dim diff-dir">{dir}</span>}
              <span className="diff-stat">
                {c.binary ? <span className="dim">{t("diff_binary")}</span> : c.untracked ? <span className="add">{t("diff_new")}</span> : (
                  <>
                    <span className="add">+{c.added}</span> <span className="del">−{c.removed}</span>
                  </>
                )}
              </span>
              <button
                className={`diff-seen ${isSeen(seen, c) ? "on" : ""}`}
                title={isSeen(seen, c) ? t("diff_unseen") : t("diff_mark_seen")}
                onClick={(e) => {
                  e.stopPropagation();
                  toggleSeen(c);
                }}
              >
                <Check size={12} />
              </button>
            </div>
          );
        })}
      </div>
      {current && (
        <div className="diff-body">
          <div className="diff-body-head">
            <span className="mono">{current.path}</span>
            <button className="pill-btn" onClick={() => onOpenPath(`${root}/${current.path}`)}>
              {t("diff_open_file")}
            </button>
          </div>
          <pre className="viewer-code diff-code" dangerouslySetInnerHTML={{ __html: highlightFile("x.diff", diff) }} />
        </div>
      )}
    </div>
  );
}
