import { useEffect, useState } from "react";
import { Bug } from "lucide-react";
import * as ipc from "../lib/ipc";
import { t } from "../lib/i18n";

/**
 * "Reportar um problema": the bug report, assembled here and filed by the
 * person, under their own GitHub account, in their own browser.
 *
 * Nothing leaves the machine through Hark. The diagnostics (build, OS,
 * agents, config with secrets masked, the scrubbed tail of the app log)
 * are shown BEFORE anything is sent, and only ride along when the box is
 * ticked — the issue is public, and the dialog says so in as many words.
 * The form opens prefilled; when the log does not fit a URL it goes to
 * the clipboard, to paste into the "Logs" field. With `gh` logged in, the
 * issue can be created directly, logs and all, and opened for editing.
 */
export default function ReportDialog({
  open,
  onClose,
  onNote,
}: {
  open: boolean;
  onClose: () => void;
  /** A line for the window's own feed once something was filed or saved. */
  onNote?: (text: string) => void;
}) {
  const [title, setTitle] = useState("");
  const [what, setWhat] = useState("");
  const [steps, setSteps] = useState("");
  const [includeLogs, setIncludeLogs] = useState(false);
  const [bundle, setBundle] = useState<ipc.ReportBundle | null>(null);
  const [gh, setGh] = useState(false);
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setNote(null);
    ipc.reportBundle().then(setBundle).catch(() => setBundle(null));
    ipc.reportGhStatus().then(setGh).catch(() => setGh(false));
  }, [open]);

  if (!open) return null;
  const ready = title.trim().length > 0 && what.trim().length > 0;
  const fail = (err: unknown) => setNote(t("report_error", { err: String(err).slice(0, 200) }));

  async function openGithub() {
    setBusy(true);
    try {
      const out = await ipc.reportIssueUrl(title, what, steps, includeLogs);
      // The form takes the logs in the URL only while it stays short;
      // past that they travel on the clipboard and the dialog stays open
      // to say so.
      const byClipboard = includeLogs && !out.logs_in_url && !!out.logs;
      if (byClipboard) {
        await navigator.clipboard.writeText(out.logs!).catch(() => {});
        setNote(t("report_pasted_hint"));
      }
      await ipc.openExternal(out.url);
      if (!byClipboard) {
        onNote?.(t("report_opened"));
        onClose();
      }
    } catch (err) {
      fail(err);
    } finally {
      setBusy(false);
    }
  }

  async function createWithGh() {
    setBusy(true);
    try {
      const url = await ipc.reportGhCreate(title, what, steps, includeLogs);
      await ipc.openExternal(url).catch(() => {});
      onNote?.(t("report_done_gh", { url }));
      onClose();
    } catch (err) {
      fail(err);
    } finally {
      setBusy(false);
    }
  }

  async function save() {
    setBusy(true);
    try {
      const path = await ipc.reportSave(title, what, steps);
      setNote(t("report_saved", { path: path.replace(/^\/Users\/[^/]+/, "~") }));
    } catch (err) {
      fail(err);
    } finally {
      setBusy(false);
    }
  }

  async function copy() {
    const text = `# ${title}\n\n${what}\n\n${steps}\n\n${includeLogs ? (bundle?.markdown ?? "") : ""}`.trimEnd();
    await navigator.clipboard.writeText(text).catch(() => {});
    setNote(t("report_copied"));
  }

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal report" onClick={(e) => e.stopPropagation()}>
        <h2>
          <Bug size={14} /> {t("report_title")}
        </h2>
        <p className="report-intro">{t("report_intro")}</p>
        <input
          autoFocus
          placeholder={t("report_field_title")}
          value={title}
          onChange={(e) => setTitle(e.target.value)}
        />
        <textarea
          placeholder={t("report_field_what")}
          value={what}
          onChange={(e) => setWhat(e.target.value)}
        />
        <textarea
          placeholder={t("report_field_steps")}
          value={steps}
          onChange={(e) => setSteps(e.target.value)}
        />
        <label className="report-check">
          <input
            type="checkbox"
            checked={includeLogs}
            onChange={(e) => setIncludeLogs(e.target.checked)}
          />
          {t("report_include_logs")}
          {bundle ? ` · ${bundle.version} · ${bundle.os} · ${t("report_lines", { n: bundle.log_lines })}` : ""}
        </label>
        {includeLogs && <div className="report-warn">⚠ {t("report_public_warning")}</div>}
        {includeLogs && bundle && (
          <details className="report-preview">
            <summary>{t("report_preview")}</summary>
            <pre>{bundle.markdown}</pre>
          </details>
        )}
        {note && <div className="report-note">{note}</div>}
        <div className="row">
          <button className="plain" onClick={onClose}>
            {t("m_cancel")}
          </button>
          <button className="plain" onClick={copy} disabled={!ready}>
            {t("report_copy")}
          </button>
          <button className="plain" onClick={save} disabled={!ready || busy}>
            {t("report_save")}
          </button>
          {gh && (
            <button className="plain" onClick={createWithGh} disabled={!ready || busy}>
              {t("report_gh_direct")}
            </button>
          )}
          <button className="primary" onClick={openGithub} disabled={!ready || busy}>
            {t("report_open_github")}
          </button>
        </div>
      </div>
    </div>
  );
}
