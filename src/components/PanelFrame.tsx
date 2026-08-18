import { Maximize2, Minimize2, X } from "lucide-react";

/**
 * The standard typed-window chrome: ONE bar — name, the window's own tabs
 * (when it has them), actions, expand and close. Closing a frame never
 * kills what runs underneath — workers/terminals keep going headless.
 */
export default function PanelFrame({
  title,
  tabs,
  actions,
  expanded,
  onToggleExpand,
  onClose,
  children,
}: {
  title: string;
  /** Tab strip rendered inside the header (no second bar). */
  tabs?: React.ReactNode;
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
        {tabs && <div className="frame-tabs">{tabs}</div>}
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
