import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import Markdown from "./Markdown";
import ToolCall, { ToolOutput } from "./ToolCall";
import type { TranscriptEntry } from "../types";

/**
 * Read-only view of a past session, loaded from its log file.
 * Nothing is executed and no tokens are spent: this is just reading.
 */
export default function Reader({
  sessionId,
  title,
  onClose,
  onResume,
}: {
  sessionId: string;
  title: string;
  onClose: () => void;
  onResume?: () => void;
}) {
  const [entries, setEntries] = useState<TranscriptEntry[] | null>(null);
  const [sessionTitle, setSessionTitle] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setEntries(null);
    setSessionTitle(null);
    setError(null);
    invoke<{ session_title: string | null; entries: TranscriptEntry[] }>(
      "read_transcript",
      { sessionId, limit: 200 },
    )
      .then((out) => {
        setEntries(out.entries);
        setSessionTitle(out.session_title);
      })
      .catch((err) => setError(String(err)));
  }, [sessionId]);

  return (
    <div className="reader">
      <div className="reader-head">
        <button className="back" onClick={onClose}>
          ← voltar
        </button>
        <span className="reader-title">{sessionTitle ?? title}</span>
        <span className="reader-meta">
          {sessionTitle && sessionTitle !== title ? `task: ${title} · ` : ""}
          somente leitura · {sessionId.slice(0, 8)}
        </span>
        {onResume && (
          <button className="resume" onClick={onResume}>
            ▶ retomar
          </button>
        )}
      </div>
      <div className="reader-body">
        {error && <div className="reader-empty">{error}</div>}
        {!entries && !error && <div className="reader-empty">lendo o histórico…</div>}
        {entries?.length === 0 && (
          <div className="reader-empty">sessão sem conversa registrada</div>
        )}
        {entries?.map((e, i) => (
          <div key={i} className={`entry ${e.role}`}>
            <span className="entry-ts">{e.ts.slice(5, 16).replace("T", " ")}</span>
            {e.role === "user" ? (
              <div className="entry-user">{e.text}</div>
            ) : e.role === "assistant" ? (
              <Markdown>{e.text}</Markdown>
            ) : e.role === "tool_use" ? (
              <ToolCall name={e.tool ?? "tool"} input={e.text} />
            ) : (
              <ToolOutput content={e.text} isError={e.is_error} />
            )}
          </div>
        ))}
      </div>
    </div>
  );
}
