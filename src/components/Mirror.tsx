import { Check, MessageSquare, X } from "lucide-react";
import { t } from "../lib/i18n";
import type { MirrorLine } from "../lib/mirror";

/**
 * The condensed mirror of the chat in focus, inside Início: tools as
 * lines, words whole, the full chat one click away in Code.
 */
export default function Mirror({
  lines,
  writing,
  onOpenFull,
}: {
  lines: MirrorLine[];
  /** A turn is open in the chat: the last line says "escrevendo…". */
  writing: boolean;
  onOpenFull: () => void;
}) {
  return (
    <div className="mirror">
      <div className="mirror-head">
        <span className="mirror-title">
          <MessageSquare size={12} /> {t("mirror_title")}
        </span>
        <button className="pill-btn" onClick={onOpenFull}>
          {t("mirror_read_full")} ›
        </button>
      </div>
      {lines.map((l, i) => {
        if (l.kind === "user") return <div key={i} className="mirror-user">{l.text}</div>;
        if (l.kind === "text") return <div key={i} className="mirror-text">{l.text}</div>;
        if (l.kind === "tool")
          return (
            <div key={i} className={`mirror-row ${l.ok === false ? "err" : ""}`}>
              {l.ok === false ? <X size={12} /> : l.ok ? <Check size={12} /> : <span className="mirror-pending">·</span>}
              <span className="mono">{l.text}</span>
            </div>
          );
        const key = l.state === "allow" ? "mirror_perm_allow" : l.state === "deny" ? "mirror_perm_deny" : l.state === "expired" ? "mirror_perm_expired" : "mirror_perm_waiting";
        return (
          <div key={i} className={`mirror-row ${l.state === "waiting" ? "warn" : ""}`}>
            <span className="mirror-pending">!</span>
            <span>{t(key, { tool: l.text })}</span>
          </div>
        );
      })}
      {writing && <div className="mirror-row dim"><span className="shimmer-text">{t("mirror_writing")}</span></div>}
    </div>
  );
}
