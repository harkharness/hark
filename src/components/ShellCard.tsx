import { Square } from "lucide-react";
import { shellEnd, type ShellMsg } from "../lib/bang";
import { t } from "../lib/i18n";

/** What the card says about an end that was not simply "it worked". */
function endNote(m: ShellMsg): string | null {
  if (m.error) return t("shell_not_started", { e: m.error });
  const end = shellEnd(m);
  if (end === "stopped") return t("shell_stopped");
  if (end === "timeout") return t("shell_timeout");
  if (end === "failed") return m.code == null ? t("shell_killed") : t("shell_exit", { n: m.code });
  return null;
}

/** A folder by its last two names: where the project's own name is. */
function shortDir(dir: string): string {
  const parts = dir.split(/[\\/]/).filter(Boolean);
  return parts.length > 2 ? `…/${parts.slice(-2).join("/")}` : dir;
}

/**
 * A command the person ran with "!" in the composer: the command, the
 * folder, what it printed. The output is text and stays text: a command
 * can print anything, a repository's own files included, and none of it
 * becomes a link, an image or markup here.
 */
export default function ShellCard({
  m,
  onStop,
  toAgent,
}: {
  m: ShellMsg;
  onStop?: (id: string) => void;
  /** The chat has an agent: the output rides with the next message. */
  toAgent?: boolean;
}) {
  const end = shellEnd(m);
  const note = endNote(m);
  return (
    <div className={`shell-card ${end}`}>
      <div className="shell-head">
        <span className="shell-bang">!</span>
        <code className="shell-cmd">{m.command}</code>
        {m.cwd && (
          <span className="shell-dir" title={m.cwd}>
            {shortDir(m.cwd)}
          </span>
        )}
        {m.running && onStop && (
          <button className="shell-stop" onClick={() => onStop(m.id)}>
            <Square size={10} /> {t("shell_stop")}
          </button>
        )}
      </div>
      {m.output ? (
        <pre className="shell-out">{m.output}</pre>
      ) : (
        !m.running && !m.error && <div className="shell-note">{t("shell_no_output")}</div>
      )}
      {m.running && <div className="shell-note shimmer-text">{t("shell_running")}</div>}
      {!!m.omitted && <div className="shell-note">{t("shell_cut", { n: m.omitted })}</div>}
      {note && <div className="shell-note end">{note}</div>}
      {toAgent && !m.running && !m.error && !m.told && <div className="shell-note">{t("shell_to_agent")}</div>}
    </div>
  );
}
