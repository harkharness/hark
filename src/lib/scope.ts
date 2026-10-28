// Where a message lands: the scope chip of the window. Mirrors
// crates/hark-core/src/domain/scope.rs; the Rust side decides, this side
// prints. Pure: no ipc, no state.
import type { Project, Scope } from "../types";

/** The window state of today (a forced project, a draft, a focused task)
 *  read as ONE scope. Nothing changes behaviour yet: the scope is what the
 *  chip and the title bar print, and what later replaces the three. */
export function scopeOf({
  forcedProject,
  draftChat,
  focusedTask,
  taskProject,
}: {
  forcedProject?: Project;
  draftChat?: Project | null;
  focusedTask?: { title: string; sessionId: string } | null;
  /** The project the focused task lives in (by workspace prefix). */
  taskProject?: Project;
}): Scope {
  if (focusedTask) {
    return {
      kind: "chat",
      title: focusedTask.title,
      sessionId: focusedTask.sessionId || undefined,
      project: taskProject ?? forcedProject,
    };
  }
  const project = draftChat ?? forcedProject;
  return project ? { kind: "project", project } : { kind: "all" };
}

/** Outermost first; empty for "all": nobody is in command. */
export function breadcrumb(scope: Scope): string[] {
  switch (scope.kind) {
    case "all":
      return [];
    case "project":
      return [scope.project.name];
    case "chat":
      return scope.project ? [scope.project.name, scope.title] : [scope.title];
    case "page":
      return [scope.space, scope.title];
  }
}

/** The project of a path, as its position in the registry: stable for as
 *  long as the registry order holds. A path is inside a project when it IS
 *  the project or sits under it; a sibling sharing the prefix is not. */
export function colourIndex(path: string | undefined, projects: Project[]): number | undefined {
  if (!path) return undefined;
  const i = projects.findIndex((p) => {
    const root = p.path.replace(/\/+$/, "");
    return path === root || path.startsWith(`${root}/`);
  });
  return i < 0 ? undefined : i;
}

/** Five slots in styles.css (--proj-0 … --proj-4); past that the palette
 *  wraps, which is fine: five live projects side by side is already rare. */
export const PROJ_SLOTS = 5;
export function projColour(slot: number | undefined): string | undefined {
  return slot === undefined ? undefined : `var(--proj-${slot % PROJ_SLOTS})`;
}
