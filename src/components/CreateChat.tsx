import { useState } from "react";
import { FolderOpen, Terminal } from "lucide-react";
import type { CliFolder } from "../types";
import { t } from "../lib/i18n";
import type { Project } from "../types";

/**
 * "Criar", the Code door's first row: the project comes first, as in
 * Claude Code. Picking one starts a draft chat there (the session is born
 * with the first message); a folder not yet registered becomes a project;
 * the CLI's own sessions can be imported instead.
 */
export default function CreateChat({
  projects,
  colourOf,
  onPick,
  onAddProject,
  onImport,
  folders = [],
}: {
  projects: Project[];
  colourOf?: (path: string) => string | undefined;
  onPick: (project: Project) => void;
  onAddProject: (path: string) => void;
  /** The palette over every session, for the one you remember by topic. */
  onImport: () => void;
  /** Folders the CLI worked in that are no project yet: one click registers. */
  folders?: CliFolder[];
}) {
  const [adding, setAdding] = useState(false);
  return (
    <div className="create">
      <div className="create-card">
        <div className="create-kicker">{t("create_project")}</div>
        {projects.length === 0 && <div className="create-empty">{t("create_no_projects")}</div>}
        {projects.map((p) => {
          const colour = colourOf?.(p.path);
          return (
            <button key={p.path} className="create-row" onClick={() => onPick(p)}>
              <span className="proj-dot" style={colour ? ({ "--proj": colour } as React.CSSProperties) : undefined} />
              <span className="create-name">{p.name}</span>
              <span className="create-path">{p.path}</span>
            </button>
          );
        })}
        {adding ? (
          <input
            className="side-rename create-input"
            autoFocus
            placeholder="~/Projects/…"
            onBlur={() => setAdding(false)}
            onKeyDown={(e) => {
              if (e.key === "Escape") setAdding(false);
              if (e.key === "Enter") {
                const value = (e.target as HTMLInputElement).value.trim();
                if (value) onAddProject(value);
                setAdding(false);
              }
            }}
          />
        ) : (
          <button className="create-row" onClick={() => setAdding(true)}>
            <FolderOpen size={13} />
            <span className="create-name">{t("create_pick_folder")}</span>
            <span className="create-path">{t("create_pick_folder_hint")}</span>
          </button>
        )}
      </div>
      <div className="create-card">
        <div className="create-kicker">{t("create_import")}</div>
        {folders.length === 0 && <div className="create-empty">{t("create_import_none")}</div>}
        {folders.map((f) => (
          <button key={f.path} className="create-row" onClick={() => onAddProject(f.path)} title={t("create_import_hint")}>
            <Terminal size={13} />
            <span className="create-name">{f.path.split("/").filter(Boolean).pop() ?? f.path}</span>
            <span className="create-path">{t("create_import_n", { n: f.sessions })} · {f.path}</span>
          </button>
        ))}
        <button className="create-row" onClick={onImport}>
          <FolderOpen size={13} />
          <span className="create-name">{t("create_import_search")}</span>
          <span className="create-path">Cmd+K</span>
        </button>
      </div>
    </div>
  );
}
