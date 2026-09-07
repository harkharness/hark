import { useMemo, useState } from "react";
import { t } from "../lib/i18n";
import type { SessionHit } from "../types";

const fold = (s: string) =>
  s
    .toLowerCase()
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "");

/** Every typed word must appear in the chat's title or last prompt. */
function matches(chat: SessionHit, query: string): boolean {
  const hay = fold(`${chat.title} ${chat.last_prompt ?? ""}`);
  return fold(query)
    .split(/\s+/)
    .filter(Boolean)
    .every((term) => hay.includes(term));
}

function ago(iso?: string | null): string {
  if (!iso) return "";
  const mins = Math.max(0, Math.round((Date.now() - new Date(iso).getTime()) / 60000));
  if (mins < 60) return t("time_min", { n: mins });
  if (mins < 1440) return t("time_hour", { n: Math.floor(mins / 60) });
  return t("time_day", { n: Math.floor(mins / 1440) });
}

/**
 * The history search, as a palette instead of a field in the sidebar.
 *
 * The sidebar's job is the handful of chats you are working in; digging
 * through a project's whole Claude Code history is a different act, and
 * it deserves the whole window's attention — the same shape Cmd+P uses
 * for files. Local index, zero tokens.
 */
export default function ChatPalette({
  chats,
  onPick,
  onClose,
}: {
  chats: SessionHit[];
  onPick: (chat: SessionHit) => void;
  onClose: () => void;
}) {
  const [query, setQuery] = useState("");
  const [sel, setSel] = useState(0);

  // Empty query = the most recent, so opening the palette already shows
  // something useful instead of a blank list waiting to be typed at.
  const hits = useMemo(() => {
    const list = query.trim() === "" ? chats : chats.filter((c) => matches(c, query));
    return list.slice(0, 40);
  }, [chats, query]);

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="quickopen" onClick={(e) => e.stopPropagation()}>
        <input
          autoFocus
          placeholder={t("palette_chats", { n: chats.length })}
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
            setSel(0);
          }}
          onKeyDown={(e) => {
            if (e.key === "Escape") onClose();
            if (e.key === "ArrowDown" || e.key === "ArrowUp") {
              e.preventDefault();
              const delta = e.key === "ArrowDown" ? 1 : -1;
              if (hits.length > 0) setSel((sel + delta + hits.length) % hits.length);
            }
            if (e.key === "Enter" && hits[sel]) onPick(hits[sel]);
          }}
        />
        <div className="quickopen-list">
          {hits.map((chat, i) => (
            <button
              key={chat.session_id}
              className={i === sel ? "sel" : ""}
              onMouseEnter={() => setSel(i)}
              onClick={() => onPick(chat)}
            >
              <span className="pal-title">{chat.title}</span>
              <span className="pal-when">{ago(chat.last_ts)}</span>
            </button>
          ))}
          {hits.length === 0 && <div className="side-empty">{t("nothing_found")}</div>}
        </div>
      </div>
    </div>
  );
}
