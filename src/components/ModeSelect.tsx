import { useState } from "react";
import { ChevronUp } from "lucide-react";

const MODES: { flag: string; name: string; hint: string }[] = [
  { flag: "manual", name: "Manual", hint: "sempre perguntar antes de agir" },
  { flag: "acceptEdits", name: "Aceitar edições", hint: "edições passam direto; o resto pergunta" },
  { flag: "plan", name: "Planejar", hint: "criar um plano antes de mexer" },
  { flag: "auto", name: "Automático", hint: "Claude gerencia as decisões de permissão" },
  { flag: "bypass", name: "Ignorar permissões", hint: "aceita tudo — cuidado" },
];

/**
 * The permission-mode pill (Claude Code's composer selector). With a live
 * worker focused it switches THAT task (process restart, no message);
 * otherwise it sets the default for new tasks born in this window.
 */
export default function ModeSelect({
  value,
  appliesTo,
  onSelect,
}: {
  /** Current mode flag ("manual" | "acceptEdits" | "plan" | "auto" | "bypass"). */
  value: string;
  /** Focused live task name, when the change applies to it. */
  appliesTo?: string;
  onSelect: (flag: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const current = MODES.find((m) => m.flag === value) ?? MODES[0];

  return (
    <span className="mode-anchor">
      <button
        className="mode-pill"
        title={
          appliesTo
            ? `modo de permissão da task "${appliesTo}" (troca reinicia o processo)`
            : "modo de permissão das novas tasks desta janela"
        }
        onClick={() => setOpen((o) => !o)}
      >
        {current.name} <ChevronUp size={11} />
      </button>
      {open && (
        <div className="mode-menu" onMouseLeave={() => setOpen(false)}>
          <div className="mode-menu-head">
            {appliesTo ? `modo · ${appliesTo.slice(0, 26)}` : "modo · novas tasks"}
          </div>
          {MODES.map((m) => (
            <button
              key={m.flag}
              className={m.flag === value ? "on" : ""}
              onClick={() => {
                setOpen(false);
                onSelect(m.flag);
              }}
            >
              <b>{m.name}</b>
              <span>{m.hint}</span>
            </button>
          ))}
        </div>
      )}
    </span>
  );
}
