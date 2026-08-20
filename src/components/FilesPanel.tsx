import { useEffect, useRef, useState } from "react";
import { ChevronDown, ChevronRight, Search } from "lucide-react";
import * as ipc from "../lib/ipc";
import FileTree from "./FileTree";
import type { OpenFile, Project } from "../types";
import { t } from "../lib/i18n";

/**
 * The "Arquivos" window: fuzzy filter across every project on top,
 * collapsible per-project trees below. All local, zero tokens; clicking
 * a file opens it as a tab in the editor window.
 */
export default function FilesPanel({
  projects,
  initialProject,
  onOpen,
}: {
  projects: Project[];
  /** Project expanded when the panel opens (sidebar folder click). */
  initialProject?: string;
  onOpen: (file: OpenFile) => void;
}) {
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<{ project: Project; rel: string }[]>([]);
  const [open, setOpen] = useState<Set<string>>(
    new Set(initialProject ? [initialProject] : projects.slice(0, 1).map((p) => p.path)),
  );
  const debounceRef = useRef<number>(0);

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
    }, 140);
  }, [query, projects]);

  return (
    <div className="filespanel">
      <div className="filespanel-search">
        <Search size={13} />
        <input
          placeholder={t("filespanel_ph")}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") {
              setQuery("");
              e.stopPropagation();
            }
          }}
        />
      </div>
      {query.trim() ? (
        <div className="quickopen-list">
          {hits.map((hit) => (
            <button
              key={`${hit.project.name}/${hit.rel}`}
              onClick={() =>
                onOpen({ abs: `${hit.project.path}/${hit.rel}`, rel: hit.rel, project: hit.project })
              }
            >
              <span className="mention-proj">{hit.project.name}/</span>
              {hit.rel}
            </button>
          ))}
          {hits.length === 0 && <div className="side-empty">nada encontrado</div>}
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
              {open.has(p.path) && <FileTree project={p} onOpen={onOpen} />}
            </section>
          ))}
        </div>
      )}
    </div>
  );
}
