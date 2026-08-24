// Self-update, in two deliberate halves: the DOWNLOAD happens quietly in
// the background (signature checked by the updater plugin against the
// pubkey compiled into the app), but the INSTALL only ever happens on the
// user's click — an app that restarts itself mid-dictation is worse than
// an outdated one.
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

let prepared: Update | null = null;

/** Poll the releases repo; resolve with the version once one is ready. */
export async function prepareUpdate(): Promise<string | null> {
  // Dev builds would try to replace target/debug — never.
  if (import.meta.env.DEV) return null;
  if (prepared) return prepared.version;
  try {
    const update = await check();
    if (!update) return null;
    await update.download();
    prepared = update;
    return update.version;
  } catch {
    // Offline, rate-limited, or the manifest is mid-publish: silence.
    // The next poll tries again; an update is never worth a nag.
    return null;
  }
}

/** The user clicked: apply the downloaded bytes and relaunch. */
export async function restartIntoUpdate(): Promise<void> {
  if (!prepared) return;
  await prepared.install();
  await relaunch();
}
