import { useEffect, useRef } from "react";
import { Lock } from "lucide-react";
import Markdown from "./Markdown";
import ToolCall, { ToolOutput } from "./ToolCall";
import { directiveLabels, shortModel } from "../lib/format";
import type { Directives, Msg } from "../types";

/**
 * The visible thread: one task at a time (or the general vox conversation).
 * Pure rendering; all state lives in App.
 */
export default function Transcript({
  messages,
  directivesFor,
  onAnswerPermission,
  onOpenPath,
}: {
  messages: Msg[];
  directivesFor: (taskLabel?: string) => Directives | undefined;
  onAnswerPermission: (requestId: string, allow: boolean) => void;
  onOpenPath: (path: string) => void;
}) {
  const endRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    endRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  return (
    <div className="transcript">
      {messages.map((m, i) => (
        <div key={i} className={`msg ${m.who}`}>
          {m.who === "sys" ? (
            <span>{m.text}</span>
          ) : m.who === "tool" ? (
            <ToolCall name={m.name} input={m.input} onOpenPath={onOpenPath} />
          ) : m.who === "output" ? (
            <ToolOutput content={m.content} isError={m.error} />
          ) : m.who === "permission" ? (
            <div className={`permission ${m.decision ?? "waiting"}`}>
              <div className="perm-head">
                <Lock size={12} /> {m.tool} pede permissão
                {m.task && <span className="tasktag">{m.task.slice(0, 24)}</span>}
              </div>
              <ToolCall name={m.tool} input={m.input} onOpenPath={onOpenPath} />
              {m.decision ? (
                <div className={`perm-done ${m.decision}`}>
                  {m.decision === "allow" ? "✓ permitido" : "✗ negado"}
                </div>
              ) : (
                <div className="perm-actions">
                  <button className="deny" onClick={() => onAnswerPermission(m.requestId, false)}>
                    negar <kbd>n</kbd>
                  </button>
                  <button className="allow" onClick={() => onAnswerPermission(m.requestId, true)}>
                    permitir <kbd>y</kbd>
                  </button>
                </div>
              )}
            </div>
          ) : (
            <div className="bubble">
              <span className="tag">
                {m.who === "user" ? "você" : "vox"}
                {m.task ? ` → ${m.task.slice(0, 12)}` : ""}
              </span>
              {m.who === "vox" ? (
                <>
                  <div className="fala">
                    <Markdown>{m.text}</Markdown>
                  </div>
                  {m.detalhes && (
                    <div className="detalhes">
                      <Markdown>{m.detalhes}</Markdown>
                    </div>
                  )}
                  {m.itens && m.itens.length > 0 && (
                    <ul className="itens">
                      {m.itens.map((it, j) => (
                        <li key={j}>{it}</li>
                      ))}
                    </ul>
                  )}
                  {(m.cost != null || m.model) && (
                    <span className="cost">
                      {[
                        shortModel(m.model),
                        ...directiveLabels(directivesFor(m.task)),
                        `$${(m.cost ?? 0).toFixed(4)}`,
                      ].join(" · ")}
                    </span>
                  )}
                </>
              ) : (
                <>
                  <div>{m.text}</div>
                  {m.image && <img className="paste" src={m.image} alt="pasted" />}
                </>
              )}
            </div>
          )}
        </div>
      ))}
      <div ref={endRef} />
    </div>
  );
}
