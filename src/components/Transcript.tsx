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
  onRunCommand,
}: {
  messages: Msg[];
  directivesFor: (taskLabel?: string) => Directives | undefined;
  onAnswerPermission: (requestId: string, allow: boolean, always?: boolean) => void;
  onOpenPath: (path: string) => void;
  /** ▶ on shell blocks: send the command to the in-app terminal. */
  onRunCommand?: (cmd: string, execute: boolean) => void;
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
              <div className="perm-title">
                <Lock size={13} /> Permitir que{" "}
                <b>{m.task ? m.task.slice(0, 32) : "o worker"}</b> execute{" "}
                <b>{m.tool}</b>?
              </div>
              <ToolCall name={m.tool} input={m.input} onOpenPath={onOpenPath} />
              {m.decision ? (
                <div className={`perm-done ${m.decision}`}>
                  {m.decision === "allow"
                    ? m.auto
                      ? "✓ permitido automaticamente (regra da task)"
                      : "✓ permitido"
                    : "✗ negado"}
                </div>
              ) : (
                <div className="perm-actions">
                  <button
                    className="deny"
                    onClick={() => onAnswerPermission(m.requestId, false)}
                  >
                    Negar <kbd>n</kbd>
                  </button>
                  <button
                    className="always"
                    title={`nunca mais perguntar por ${m.tool} nesta task (até fechar a janela)`}
                    onClick={() => onAnswerPermission(m.requestId, true, true)}
                  >
                    Sempre permitir <kbd>a</kbd>
                  </button>
                  <button
                    className="allow"
                    onClick={() => onAnswerPermission(m.requestId, true)}
                  >
                    Permitir uma vez <kbd>y</kbd>
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
                    <Markdown onRun={onRunCommand}>{m.text}</Markdown>
                  </div>
                  {m.detalhes && (
                    <div className="detalhes">
                      <Markdown onRun={onRunCommand}>{m.detalhes}</Markdown>
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
                    <span
                      className="cost"
                      title={
                        m.usage
                          ? `in ${m.usage.input} · out ${m.usage.output} · cache lido ${m.usage.cache_read} · cache novo ${m.usage.cache_created}` +
                            (m.usage.input + m.usage.cache_read + m.usage.cache_created > 0
                              ? ` · cache ${Math.round((m.usage.cache_read / (m.usage.input + m.usage.cache_read + m.usage.cache_created)) * 100)}%`
                              : "")
                          : undefined
                      }
                    >
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
                  {m.images?.map((src, j) => (
                    <img key={j} className="paste" src={src} alt={`image `} />
                  ))}
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
