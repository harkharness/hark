// Self-update, in two deliberate halves: the DOWNLOAD happens quietly in
// the background (signature checked by the updater plugin against the
// pubkey compiled into the app), but the INSTALL only ever happens on the
// user's click — an app that restarts itself mid-dictation is worse than
// an outdated one.
//
// The status is kept and shown, not swallowed. A feature whose failure
// mode is silence needs somewhere to say "checked two minutes ago, you
// are on the newest build" — otherwise waiting and broken look identical.
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { getVersion } from "@tauri-apps/api/app";

export type UpdateStatus =
  /** Never looked yet. */
  | { kind: "idle" }
  | { kind: "checking" }
  /** This build IS the newest one published. */
  | { kind: "current"; at: number }
  /** Downloaded and verified; waiting for the click. */
  | { kind: "ready"; version: string; at: number }
  /** Offline, rate-limited, or a manifest mid-publish. */
  | { kind: "error"; message: string; at: number }
  /** Dev build: replacing target/debug is never the right move. */
  | { kind: "dev" };

let status: UpdateStatus = import.meta.env.DEV ? { kind: "dev" } : { kind: "idle" };
let prepared: Update | null = null;

export const lastStatus = (): UpdateStatus => status;

/** The running build's version, for "you are on X". */
export const currentVersion = (): Promise<string> => getVersion().catch(() => "?");

/** Poll the releases repo and download anything newer. Never throws. */
export async function checkForUpdate(): Promise<UpdateStatus> {
  if (status.kind === "dev") return status;
  if (prepared) return status;
  status = { kind: "checking" };
  try {
    const update = await check();
    if (!update) {
      status = { kind: "current", at: Date.now() };
      return status;
    }
    await update.download();
    prepared = update;
    status = { kind: "ready", version: update.version, at: Date.now() };
  } catch (err) {
    // The message matters: "404" means a release mid-publish, a network
    // error means offline. Both are fine; both should be readable.
    status = { kind: "error", message: String(err), at: Date.now() };
  }
  return status;
}

/** The user clicked: apply the downloaded bytes and relaunch. */
export async function restartIntoUpdate(): Promise<void> {
  if (!prepared) return;
  await prepared.install();
  await relaunch();
}
