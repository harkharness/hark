// The board as the screen reads it: which cards a scope shows, the line
// over them, how long a card sat still. Pure: no ipc, no state.
import type { BoardTask, Project } from "../types";

/** A task lives in a project when its workspace IS the project or sits
 *  under it; a sibling sharing the prefix does not. */
export function inProject(task: BoardTask, project: Project): boolean {
  const ws = task.workspace;
  if (!ws) return false;
  const root = project.path.replace(/\/+$/, "");
  return ws === root || ws.startsWith(`${root}/`);
}

/** No project = every card; a project = its own cards only. */
export function filterTasks(tasks: BoardTask[], project: Project | null | undefined): BoardTask[] {
  return project ? tasks.filter((t) => inProject(t, project)) : tasks;
}

/** Cards shown, how many run a worker, how many wait on a permission. */
export function boardSummary(tasks: BoardTask[], live: Set<string>, asking: Set<string>): { cards: number; live: number; asking: number } {
  return {
    cards: tasks.length,
    live: tasks.filter((t) => live.has(t.title)).length,
    asking: tasks.filter((t) => asking.has(t.title)).length,
  };
}

/** Whole days since the card last moved; zero when unreadable. */
export function idleDays(updatedAt: string, now = Date.now()): number {
  const ms = now - Date.parse(updatedAt);
  if (!Number.isFinite(ms) || ms < 0) return 0;
  return Math.floor(ms / 86_400_000);
}
