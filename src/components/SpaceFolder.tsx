import { useState } from "react";
import { FolderGit2, HardDrive } from "lucide-react";
import { t } from "../lib/i18n";
import type { ArtifactSpace } from "../types";

/** The folder inside a project where its pages live (domain::pages_home). */
const IN_PROJECT = "docs/hark";

const tilde = (p: string) => p.replace(/^(?:\/Users|\/home)\/[^/]+(?=\/|$)/, "~");

/**
 * Where a space keeps its pages, and the one way they get into a
 * repository: the person asks, per project, and confirms seeing the
 * folder that will be made. Hark never writes `docs/hark` into a
 * repository on its own: a project's documentation going public, or into
 * someone else's repository, is not a default.
 */
export default function SpaceFolder({
  space,
  onMove,
}: {
  space: ArtifactSpace;
  /** Move the space's pages into its project; resolves to how many went. */
  onMove: (space: string) => Promise<number>;
}) {
  const [asking, setAsking] = useState(false);
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState<{ text: string; error: boolean } | null>(null);
  // A project's pages still on this machine: all of them, or the ones
  // left behind when the folder arrived with a pull.
  const movable = !!space.project && (!space.in_project || space.left_behind > 0);
  const target = space.project ? tilde(`${space.project.replace(/\/+$/, "")}/${IN_PROJECT}`) : "";

  async function move() {
    setAsking(false);
    setBusy(true);
    setNote(null);
    try {
      const n = await onMove(space.name);
      setNote({ text: t("space_moved", { n }), error: false });
    } catch (err) {
      setNote({ text: err instanceof Error ? err.message : String(err), error: true });
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="space-folder">
      <div className="space-folder-row">
        {space.in_project ? <FolderGit2 size={12} /> : <HardDrive size={12} />}
        <span className="space-folder-path mono" title={space.folder}>
          {tilde(space.folder)}
        </span>
        {space.project && <span className={`mm-tag ${space.in_project ? "on" : ""}`}>{space.in_project ? t("space_in_repo") : t("space_on_machine")}</span>}
        {movable && !asking && !busy && (
          <button className="pill-btn" onClick={() => setAsking(true)}>
            {space.in_project ? t("space_bring", { n: space.left_behind }) : t("space_move")}
          </button>
        )}
        {busy && <span className="dim shimmer-text">{t("space_moving")}</span>}
      </div>
      {asking && (
        <div className="space-folder-ask">
          <span>{t("space_move_ask", { dir: target })}</span>
          <span className="space-folder-actions">
            <button className="pill-btn" onClick={() => setAsking(false)}>
              {t("space_move_no")}
            </button>
            <button className="pill-btn on" onClick={() => void move()}>
              {t("space_move_yes")}
            </button>
          </span>
        </div>
      )}
      {note && <div className={`space-folder-note ${note.error ? "err" : ""}`}>{note.text}</div>}
    </div>
  );
}
