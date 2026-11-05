import { ClipboardList } from "lucide-react";
import { t } from "../lib/i18n";

/**
 * The soft reminder that a chat has no task (domain::task_nudge decides
 * when). One line above the composer, two verbs, never a modal: a task
 * is a deliberate record, the window only offers.
 */
export default function TaskNudge({ onRegister, onLater }: { onRegister: () => void; onLater: () => void }) {
  return (
    <div className="task-nudge" role="status">
      <ClipboardList size={12} />
      <span className="task-nudge-text">{t("nudge_no_task")}</span>
      <button className="link-btn" onClick={onRegister} title={t("nudge_register_hint")}>
        {t("nudge_register")}
      </button>
      <span className="task-nudge-sep">·</span>
      <button className="link-btn" onClick={onLater}>
        {t("nudge_later")}
      </button>
    </div>
  );
}
