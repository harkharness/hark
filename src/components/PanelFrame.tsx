import { Maximize2, Minimize2, X } from "lucide-react";

/**
 * The standard typed-window chrome: name on the left, per-type actions,
 * expand (full work area, menu stays) and close. Closing a frame never
 * kills what runs underneath — workers/terminals keep going headless.
 */
export default function PanelFrame({
  title,
  actions,
  expanded,
  onToggleExpand,
  onClose,
  children,
}: {
  title: string;
  actions?: React.ReactNode;
  expanded: boolean;
  onToggleExpand: () => void;
  onClose: () => void;
  children: React.ReactNode;
}) {
  return (
    <div className={`frame ${expanded ? "expanded" : ""}`}>
      <div className="frame-head">
        <span className="frame-title">{title}</span>
        <span className="frame-actions">
          {actions}
          <button onClick={onToggleExpand} title={expanded ? "restaurar" : "expandir"}>
            {expanded ? <Minimize2 size={13} /> : <Maximize2 size={13} />}
          </button>
          <button onClick={onClose} title="fechar janela (continua rodando por baixo)">
            <X size={13} />
          </button>
        </span>
      </div>
      <div className="frame-body">{children}</div>
    </div>
  );
}
