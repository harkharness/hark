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
  page,
}: {
  forcedProject?: Project;
  draftChat?: Project | null;
  focusedTask?: { title: string; sessionId: string } | null;
  /** The project the focused task lives in (by workspace prefix). */
  taskProject?: Project;
  /** An artifact page on screen: it is the scope, over any chat. */
  page?: { title: string; path: string; space: string } | null;
}): Scope {
  if (page) return { kind: "page", title: page.title, path: page.path, space: page.space };
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

/** The rows of the chip's picker: "all" first (project null), then the
 *  registry in its order, the one in force marked. Null when the chip
 *  is not a picker: inside a chat or on a page the target was chosen by
 *  opening it, and a chip that moved it would send a message elsewhere. */
export function scopeChoices(projects: Project[], scope: Scope): { project: Project | null; current: boolean }[] | null {
  if (scope.kind === "chat" || scope.kind === "page") return null;
  const here = scope.kind === "project" ? scope.project.path : null;
  return [
    { project: null, current: here === null },
    ...projects.map((project) => ({ project, current: project.path === here })),
  ];
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

/** The projects a panel beside the chat lists. Files, terminal and
 *  session belong to the chat on screen, not to the whole window: inside a
 *  chat or a draft it is that one project. Everything is listed only where
 *  nobody is in command, or for a chat whose folder is no project. */
export function projectsInView(projects: Project[], scope: Scope): Project[] {
  const one = scope.kind === "chat" || scope.kind === "project" ? scope.project : undefined;
  return one ? [one] : projects;
}
