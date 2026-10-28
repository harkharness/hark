import { useState, type ReactNode } from "react";
import { Check, ExternalLink, Share2 } from "lucide-react";
import * as ipc from "../lib/ipc";
import { t } from "../lib/i18n";

/** Where the shared-spaces discussion lives: the footer's "vote" link. */
export const DISCUSSIONS_URL = "https://github.com/harkharness/hark/discussions";

/** One thing the popover does: every row delivers something local now. */
export type ShareRow = {
  icon: ReactNode;
  label: string;
  hint: string;
  kbd?: string;
  /** Runs the action; the resolved text is the toast (undefined = none). */
  run: () => Promise<string | void> | string | void;
};

/**
 * The share popover of a page or a task: useful today, honest about
 * tomorrow. Each row delivers something on this machine (a link that
 * reopens Hark in place, markdown, a file, the Finder); the footer says
 * the limit of today and where to vote for shared spaces. No dead button.
 */
export default function SharePopover({ title, rows, onClose }: { title: string; rows: ShareRow[]; onClose: () => void }) {
  const [toast, setToast] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  async function fire(row: ShareRow) {
    setError(null);
    try {
      const out = await row.run();
      if (out) {
        setToast(out);
        window.setTimeout(() => setToast(null), 2200);
      }
    } catch (err) {
      setError(String(err));
    }
  }
  return (
    <>
      <div className="share-backdrop" onClick={onClose} />
      <div className="share-pop" role="dialog" aria-label={t("share_title")}>
        <div className="share-head">
          <Share2 size={13} />
          <b>{title}</b>
        </div>
        {rows.map((row) => (
          <button key={row.label} className="share-row" onClick={() => void fire(row)}>
            <span className="share-ico">{row.icon}</span>
            <span className="share-text">
              <span className="share-label">{row.label}</span>
              <span className="share-hint">{row.hint}</span>
            </span>
            {row.kbd && <kbd>{row.kbd}</kbd>}
          </button>
        ))}
        <div className="share-foot">
          <b>{t("share_soon_title")}</b> {t("share_soon_body")}
          <div className="share-foot-row">
            <button className="pill-btn" onClick={() => void ipc.openExternal(DISCUSSIONS_URL).catch(() => {})}>
              <ExternalLink size={10} /> {t("share_vote")}
            </button>
            <span className="dim">{t("share_vote_hint")}</span>
          </div>
        </div>
        {toast && (
          <div className="share-toast">
            <Check size={11} /> {toast}
          </div>
        )}
        {error && <div className="share-error">{error}</div>}
      </div>
    </>
  );
}
