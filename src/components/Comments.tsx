import { useEffect, useState } from "react";
import { Check, CheckCircle2, MessageSquare, Send, Trash2, Undo2 } from "lucide-react";
import * as ipc from "../lib/ipc";
import { t } from "../lib/i18n";
import { anchorLabel, mentionsHark } from "../lib/comments";
import type { Destination } from "../lib/takeTo";
import type { CommentAnchor, PageComment } from "../types";

const ago = (iso: string) => {
  const m = Math.max(0, Math.round((Date.now() - new Date(iso).getTime()) / 60000));
  if (m < 1) return "agora";
  if (m < 60) return `${m} min`;
  const h = Math.round(m / 60);
  if (h < 48) return `${h} h`;
  return `${Math.round(h / 24)} d`;
};

const destLabel = (d: Destination) =>
  d.kind === "origin" ? t("cm_dest_origin", { t: d.title }) : d.kind === "chat" ? t("cm_dest_chat", { t: d.title }) : d.kind === "new" ? t("cm_dest_new", { p: d.project }) : t("cm_dest_hark");
const destKey = (d: Destination) => (d.kind === "origin" || d.kind === "chat" ? `${d.kind}:${d.title}` : d.kind === "new" ? `new:${d.project}` : "hark");

/**
 * The comments column of a page (fase 14.3): the bubble on top, anchored
 * to whatever is selected (a cell, a note, a panel, a passage, else the
 * page), with the `@hark` chip, "send to hark" (checked the moment the
 * text names @hark) and the chat it goes to, the origin first; the
 * comments under it, newest first, each with its anchor, its request
 * badge ("answered" once that chat wrote the page), resolve and remove.
 */
export default function Comments({
  space,
  id,
  comments,
  anchor,
  destinations,
  onChanged,
  onSend,
  onPick,
}: {
  space: string;
  id: string;
  comments: PageComment[];
  anchor: CommentAnchor;
  destinations: Destination[];
  onChanged: () => void;
  /** A request leaves for a chat: the text, already built. */
  onSend: (dest: Destination, text: string) => Promise<void> | void;
  /** A comment clicked: the page shows its anchor. */
  onPick?: (c: PageComment) => void;
}) {
  const [text, setText] = useState("");
  const [send, setSend] = useState(false);
  const [touched, setTouched] = useState(false);
  const [dest, setDest] = useState<string>(() => destKey(destinations.find((d) => d.kind === "origin") ?? destinations[0] ?? { kind: "hark" }));
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);

  // The mention decides the checkbox until the person touches it.
  useEffect(() => {
    if (!touched) setSend(mentionsHark(text));
  }, [text, touched]);
  useEffect(() => {
    if (!destinations.some((d) => destKey(d) === dest)) setDest(destKey(destinations.find((d) => d.kind === "origin") ?? destinations[0] ?? { kind: "hark" }));
  }, [destinations, dest]);

  async function submit() {
    const body = text.trim();
    if (!body || busy) return;
    setBusy(true);
    setErr(null);
    try {
      const c = await ipc.artifactCommentAdd(space, id, body, anchor);
      const target = destinations.find((d) => destKey(d) === dest);
      if (send && target) {
        const title = target.kind === "origin" || target.kind === "chat" ? target.title : null;
        const session = target.kind === "origin" || target.kind === "chat" ? target.sessionId ?? null : null;
        const request = await ipc.artifactCommentSend(space, id, c.id, session, title);
        await onSend(target, request);
        setNote(t("cm_sent", { t: destLabel(target) }));
      }
      setText("");
      setTouched(false);
      onChanged();
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  }
  async function resolve(c: PageComment, on: boolean) {
    await ipc.artifactCommentResolve(space, id, c.id, on).catch((e) => setErr(String(e)));
    onChanged();
  }
  async function remove(c: PageComment) {
    await ipc.artifactCommentRemove(space, id, c.id).catch((e) => setErr(String(e)));
    onChanged();
  }

  const open = comments.filter((c) => !c.resolved).reverse();
  const done = comments.filter((c) => c.resolved).reverse();

  return (
    <div className="cm-col">
      <div className="cm-head">
        <MessageSquare size={12} /> {t("cm_title")} · {comments.length}
      </div>
      <div className="cm-bubble">
        <div className="cm-anchor dim">{t("cm_at", { a: anchorLabel(anchor) })}</div>
        <textarea
          rows={3}
          placeholder={t("cm_ph")}
          value={text}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) void submit();
          }}
        />
        <div className="cm-row">
          <button
            className={`cm-chip ${mentionsHark(text) ? "on" : ""}`}
            title={t("cm_mention_hint")}
            onClick={() => setText((v) => (mentionsHark(v) ? v : `@hark ${v}`))}
          >
            {t("cm_mention")}
          </button>
          <label className="cm-send">
            <input type="checkbox" checked={send} onChange={(e) => { setTouched(true); setSend(e.target.checked); }} /> {t("cm_send")}
          </label>
        </div>
        {send && (
          <label className="cm-dest">
            <span className="dim">{t("cm_dest")}</span>
            <select value={dest} onChange={(e) => setDest(e.target.value)}>
              {destinations.map((d) => (
                <option key={destKey(d)} value={destKey(d)}>{destLabel(d)}</option>
              ))}
            </select>
          </label>
        )}
        <div className="cm-row">
          <button className="pill-btn on" onClick={() => void submit()} disabled={!text.trim() || busy}>
            {send ? <Send size={11} /> : <Check size={11} />} {t("cm_submit")}
          </button>
          {note && <span className="dim cm-note">{note}</span>}
        </div>
        {err && <div className="cm-err">{t("cm_err", { e: err })}</div>}
      </div>
      {comments.length === 0 && <div className="dim cm-empty">{t("cm_none")}</div>}
      {[...open, ...done].map((c) => (
        <div key={c.id} className={`cm-item ${c.resolved ? "done" : ""}`} onClick={() => onPick?.(c)}>
          <div className="cm-item-anchor dim">
            {anchorLabel(c.anchor)} · {ago(c.at)}
          </div>
          <div className="cm-item-text">{c.text}</div>
          {c.sent && (
            <div className={`cm-badge ${c.answered_at ? "ok" : ""}`} title={c.answered_at ? t("cm_answered_hint") : undefined}>
              {c.answered_at ? (
                <>
                  <CheckCircle2 size={10} /> {t("cm_answered")}
                </>
              ) : (
                <>
                  <Send size={10} /> {t("cm_sent", { t: c.sent.task ?? c.sent.session_id?.slice(0, 8) ?? "hark" })}
                </>
              )}
            </div>
          )}
          <div className="cm-item-acts" onClick={(e) => e.stopPropagation()}>
            <button className="pill-btn" onClick={() => void resolve(c, !c.resolved)}>
              {c.resolved ? <Undo2 size={10} /> : <Check size={10} />} {c.resolved ? t("cm_reopen") : t("cm_resolve")}
            </button>
            <button className="pill-btn" onClick={() => void remove(c)} title={t("cm_remove")}>
              <Trash2 size={10} />
            </button>
          </div>
        </div>
      ))}
    </div>
  );
}
