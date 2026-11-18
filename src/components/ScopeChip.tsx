import { useEffect, useRef, useState } from "react";
import { Check, ChevronUp, Globe } from "lucide-react";
import { t } from "../lib/i18n";
import { breadcrumb, scopeChoices } from "../lib/scope";
import type { Project, Scope } from "../types";

/** The scope chip: a globe for "all projects", else the project's dot and
 *  the breadcrumb (project › chat). It says where the message lands, and
 *  where nothing is open yet it also CHOOSES it: all projects (Hark
 *  answers) or one of them (the message opens a chat there). Inside a
 *  chat it is a label: the target was chosen by opening the chat. */
export default function ScopeChip({
  scope,
  colour,
  projects,
  onPick,
  colourOf,
}: {
  scope: Scope;
  colour?: string;
  /** The registry, for the picker. */
  projects?: Project[];
  /** Null = all projects. Absent = the chip is only a label. */
  onPick?: (project: Project | null) => void;
  colourOf?: (path?: string) => string | undefined;
}) {
  const [open, setOpen] = useState(false);
  const anchor = useRef<HTMLSpanElement>(null);
  useEffect(() => {
    if (!open) return;
    const away = (e: MouseEvent) => {
      if (!anchor.current?.contains(e.target as Node)) setOpen(false);
    };
    const esc = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    document.addEventListener("mousedown", away);
    document.addEventListener("keydown", esc);
    return () => {
      document.removeEventListener("mousedown", away);
      document.removeEventListener("keydown", esc);
    };
  }, [open]);

  const crumbs = breadcrumb(scope);
  const body =
    crumbs.length === 0 ? (
      <>
        <Globe size={12} /> {t("scope_all")}
      </>
    ) : (
      <>
        {colour && <span className="dot" />}
        {crumbs.map((c, i) => (
          <span key={i}>
            {i > 0 && <span className="sep"> › </span>}
            {c}
          </span>
        ))}
      </>
    );
  const style = colour ? ({ "--proj": colour } as React.CSSProperties) : undefined;
  const choices = onPick && projects ? scopeChoices(projects, scope) : null;
  if (!choices) {
    return (
      <span className="scope-chip" style={style} title={crumbs.length ? crumbs.join(" › ") : t("scope_all")}>
        {body}
      </span>
    );
  }
  return (
    <span className="mode-anchor" ref={anchor}>
      <button className="scope-chip pick" style={style} title={t("scope_pick_title")} onClick={() => setOpen((o) => !o)}>
        {body} <ChevronUp size={11} />
      </button>
      {open && (
        <div className="mode-menu scope-menu" role="menu">
          <div className="mode-menu-head">{t("scope_pick_head")}</div>
          {choices.map(({ project, current }) => (
            <button
              key={project?.path ?? "-"}
              role="menuitem"
              className={`mm-row${current ? " on" : ""}`}
              title={project?.path}
              onClick={() => {
                setOpen(false);
                if (!current) onPick!(project);
              }}
            >
              <span className="mm-text">
                <span className="mm-name">
                  {project ? (
                    <span className="scope-dot" style={{ background: colourOf?.(project.path) ?? "var(--dim)" }} />
                  ) : (
                    <Globe size={12} />
                  )}{" "}
                  {project ? project.name : t("scope_all")}
                </span>
                <span className="mm-hint">{project ? t("scope_project_hint") : t("scope_all_hint")}</span>
              </span>
              {current && (
                <span className="mm-check">
                  <Check size={13} strokeWidth={3} />
                </span>
              )}
            </button>
          ))}
        </div>
      )}
    </span>
  );
}
