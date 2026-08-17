import { useEffect, useRef, useState } from "react";
import * as ipc from "../lib/ipc";
import type { OpenFile, Project } from "../types";

/**
 * Cmd+P: fuzzy file search across every registered project, VSCode-style.
 * Enter opens the selection in the file viewer.
 */
export default function QuickOpen({
  projects,
  onPick,
  onClose,
}: {
  projects: Project[];
  onPick: (file: OpenFile) => void;
  onClose: () => void;
}) {
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<{ project: Project; rel: string }[]>([]);
  const [sel, setSel] = useState(0);
  const debounceRef = useRef<number>(0);

  useEffect(() => {
    window.clearTimeout(debounceRef.current);
    debounceRef.current = window.setTimeout(async () => {
      const results = await Promise.all(
        projects.map((project) =>
          ipc
            .projectFiles(project.path, query, 12)
            .then((rels) => rels.map((rel) => ({ project, rel })))
            .catch(() => []),
        ),
      );
      setHits(results.flat().slice(0, 40));
      setSel(0);
    }, 130);
  }, [query, projects]);

  function pick(hit: { project: Project; rel: string }) {
    onPick({ abs: `${hit.project.path}/${hit.rel}`, rel: hit.rel, project: hit.project });
  }

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="quickopen" onClick={(e) => e.stopPropagation()}>
        <input
          autoFocus
          placeholder="arquivo… (Enter abre, Esc fecha)"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") onClose();
            if (e.key === "ArrowDown" || e.key === "ArrowUp") {
              e.preventDefault();
              const delta = e.key === "ArrowDown" ? 1 : -1;
              if (hits.length > 0) setSel((sel + delta + hits.length) % hits.length);
            }
            if (e.key === "Enter" && hits[sel]) pick(hits[sel]);
          }}
        />
        <div className="quickopen-list">
          {hits.map((hit, i) => (
            <button
              key={`${hit.project.name}/${hit.rel}`}
              className={i === sel ? "sel" : ""}
              onMouseEnter={() => setSel(i)}
              onClick={() => pick(hit)}
            >
              <span className="mention-proj">{hit.project.name}/</span>
              {hit.rel}
            </button>
          ))}
          {hits.length === 0 && <div className="side-empty">nada encontrado</div>}
        </div>
      </div>
    </div>
  );
}
