import { GitBranch } from "lucide-react";
import { t } from "../lib/i18n";
import type { RepoState } from "../types";

/**
 * One line at the top of the composer: which branch this task's work is
 * on, how much is uncommitted, how far from the remote. It is a READOUT,
 * not a git client — clicking nothing, deciding nothing. What it buys is
 * that "sobe isso" is now said with the branch in sight.
 */
export default function RepoRuler({ state }: { state: RepoState }) {
  const parts: string[] = [];
  if (state.dirty > 0) parts.push(t("repo_dirty", { n: state.dirty }));
  if (state.ahead > 0) parts.push(t("repo_ahead", { n: state.ahead }));
  if (state.behind > 0) parts.push(t("repo_behind", { n: state.behind }));

  return (
    <div className="repo-ruler">
      <GitBranch size={12} />
      <span className="rr-branch">{state.branch || t("repo_detached")}</span>
      {(state.added > 0 || state.removed > 0) && (
        <span className="rr-stat">
          <span className="add">+{state.added.toLocaleString("pt-BR")}</span>{" "}
          <span className="del">−{state.removed.toLocaleString("pt-BR")}</span>
        </span>
      )}
      <span className="rr-state">
        {parts.length > 0 ? parts.join(" · ") : t("repo_clean")}
      </span>
    </div>
  );
}
