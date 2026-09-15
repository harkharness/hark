import { useEffect, useMemo, useRef, useState } from "react";
import { ChevronDown, ChevronRight, Search, X } from "lucide-react";
import * as ipc from "../lib/ipc";
import FileTree from "./FileTree";
import type { OpenFile, Project } from "../types";
import { t } from "../lib/i18n";

type Hit = { project: Project; rel: string };

/** "src/lib/foo.ts" → ["foo.ts", "src/lib/"]: the name leads, the folder follows, dimmed. */
export function splitHit(rel: string): [string, string] {
  const cut = rel.lastIndexOf("/");
  return cut < 0 ? [rel, ""] : [rel.slice(cut + 1), rel.slice(0, cut + 1)];
}

/**
 * The tree column of the files window: fuzzy filter across every project on
 * top, collapsible per-project trees below. All local, zero tokens; a pick
 * opens a tab beside it. The tab in front pulls its project open so the
 * tree can reveal and mark it.
 */
export default function FilesPanel({
  projects,
  initialProject,
  activeAbs,
  onOpen,
}: {
  projects: Project[];
  /** Project expanded when the panel opens (sidebar folder click). */
  initialProject?: string;
  /** Absolute path of the tab in front. */
  activeAbs?: string;
  onOpen: (file: OpenFile) => void;
}) {
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<Hit[]>([]);
  const [sel, setSel] = useState(0);
  const [open, setOpen] = useState<Set<string>>(
    new Set(initialProject ? [initialProject] : projects.slice(0, 1).map((p) => p.path)),
  );
  const inputRef = useRef<HTMLInputElement>(null);
  const debounceRef = useRef<number>(0);

  const activeProject = useMemo(
    () => (activeAbs ? projects.find((p) => activeAbs.startsWith(`${p.path}/`)) : undefined),
    [activeAbs, projects],
  );
  const reveal = (path: string) =>
    setOpen((old) => (old.has(path) ? old : new Set(old).add(path)));
  useEffect(() => {
    if (activeProject) reveal(activeProject.path);
  }, [activeProject]);
  // A folder clicked in the sidebar while the panel is already up.
  useEffect(() => {
    if (initialProject) reveal(initialProject);
  }, [initialProject]);

  useEffect(() => {
    if (!query.trim()) {
      setHits([]);
      return;
    }
    window.clearTimeout(debounceRef.current);
    debounceRef.current = window.setTimeout(async () => {
      const results = await Promise.all(
        projects.map((project) =>
          ipc
            .projectFiles(project.path, query, 10)
            .then((rels) => rels.map((rel) => ({ project, rel })))
            .catch(() => []),
        ),
      );
      setHits(results.flat().slice(0, 30));
      setSel(0);
    }, 140);
  }, [query, projects]);

  function pick(hit: Hit) {
    onOpen({ abs: `${hit.project.path}/${hit.rel}`, rel: hit.rel, project: hit.project });
  }

  function clear() {
    setQuery("");
    inputRef.current?.focus();
  }

  return (
    <div className="filespanel">
      <div className="filespanel-search">
        <Search size={13} />
        <div className="filespanel-field">
          <input
            ref={inputRef}
            placeholder={t("filespanel_ph")}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Escape") {
                clear();
                e.stopPropagation();
                return;
              }
              if (!query.trim()) return;
              if (e.key === "ArrowDown" || e.key === "ArrowUp") {
                e.preventDefault();
                const delta = e.key === "ArrowDown" ? 1 : -1;
                if (hits.length > 0) setSel((s) => (s + delta + hits.length) % hits.length);
              }
              if (e.key === "Enter" && hits[sel]) pick(hits[sel]);
            }}
          />
          {query && (
            <button className="filespanel-clear" title={t("filespanel_clear")} onClick={clear}>
              <X size={11} />
            </button>
          )}
        </div>
      </div>
      {query.trim() ? (
        <div className="quickopen-list">
          {hits.map((hit, i) => {
            const [name, dir] = splitHit(hit.rel);
            return (
              <button
                key={`${hit.project.name}/${hit.rel}`}
                className={i === sel ? "sel" : ""}
                title={`${hit.project.name}/${hit.rel}`}
                onMouseEnter={() => setSel(i)}
                onClick={() => pick(hit)}
              >
                <span className="hit-name">{name}</span>
                <span className="hit-dir">
                  {hit.project.name}/{dir}
                </span>
              </button>
            );
          })}
          {hits.length === 0 && <div className="side-empty">{t("nothing_found")}</div>}
        </div>
      ) : (
        <div className="filespanel-trees">
          {projects.map((p) => (
            <section key={p.path}>
              <button
                className="filespanel-proj"
                onClick={() =>
                  setOpen((old) => {
                    const next = new Set(old);
                    if (next.has(p.path)) next.delete(p.path);
                    else next.add(p.path);
                    return next;
                  })
                }
              >
                {open.has(p.path) ? <ChevronDown size={13} /> : <ChevronRight size={13} />}
                {p.name}
              </button>
              {open.has(p.path) && (
                <FileTree
                  project={p}
                  activeRel={
                    activeAbs?.startsWith(`${p.path}/`) ? activeAbs.slice(p.path.length + 1) : undefined
                  }
                  onOpen={onOpen}
                />
              )}
            </section>
          ))}
        </div>
      )}
    </div>
  );
}
