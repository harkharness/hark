import { emit as tauriEmit, listen as tauriListen, type EventCallback, type UnlistenFn } from "@tauri-apps/api/event";
import { demoEmit, demoListen, isDemo } from "./demo";

/**
 * The one door backend events come through. Components and hooks import
 * `listen` from here, never from the Tauri API: in demo mode (`?demo=`)
 * the events come from the local script instead of the Rust shell.
 */
export function listen<T>(name: string, cb: EventCallback<T>): Promise<UnlistenFn> {
  return isDemo() ? demoListen(name, cb) : tauriListen(name, cb);
}

/** An event this window raises for the others (and for itself). */
export function emit(name: string, payload?: unknown): Promise<void> {
  if (isDemo()) {
    demoEmit(name, payload);
    return Promise.resolve();
  }
  return tauriEmit(name, payload);
}
