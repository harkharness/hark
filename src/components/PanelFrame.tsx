import { ChevronDown, ChevronUp, GripVertical, Maximize2, Minimize2, X } from "lucide-react";

/**
 * The standard typed-window chrome: ONE bar — grip (drag to reorder in
 * the rail), name, the window's own tabs, actions, collapse, expand and
 * close. Collapsed = just this bar; the body stays MOUNTED but hidden
 * (terminals keep their screen, editors keep unsaved drafts). Closing a
 * frame never kills what runs underneath.
 */
export default function PanelFrame({
  title,
  tabs,
  actions,
  expanded,
  collapsed = false,
  onToggleExpand,
  onToggleCollapse,
  onClose,
  dragProps,
  children,
}: {
  title: string;
  /** Tab strip rendered inside the header (no second bar). */
  tabs?: React.ReactNode;
  actions?: React.ReactNode;
  expanded: boolean;
  /** Shrunk to the title bar (rail panels only). */
  collapsed?: boolean;
  onToggleExpand: () => void;
  /** Present = the frame lives in the rail and can collapse. */
  onToggleCollapse?: () => void;
  onClose: () => void;
  /** Drag-to-reorder handlers, applied to the header. */
  dragProps?: React.HTMLAttributes<HTMLDivElement>;
  children: React.ReactNode;
}) {
  return (
    <div className={`frame ${expanded ? "expanded" : ""} ${collapsed ? "collapsed" : ""}`}>
      <div
        className="frame-head"
        {...dragProps}
        onClick={onToggleCollapse}
        title={onToggleCollapse ? (collapsed ? "expandir painel" : "recolher painel") : undefined}
      >
        {dragProps && <GripVertical size={13} className="frame-grip" />}
        <span className="frame-title">{title}</span>
        {!collapsed && tabs && (
          <div className="frame-tabs" onClick={(e) => e.stopPropagation()}>
            {tabs}
          </div>
        )}
        <span className="frame-actions" onClick={(e) => e.stopPropagation()}>
          {!collapsed && actions}
          {onToggleCollapse && (
            <button onClick={onToggleCollapse} title={collapsed ? "expandir" : "recolher"}>
              {collapsed ? <ChevronUp size={13} /> : <ChevronDown size={13} />}
            </button>
          )}
          {!collapsed && (
            <button onClick={onToggleExpand} title={expanded ? "restaurar" : "tela cheia"}>
              {expanded ? <Minimize2 size={13} /> : <Maximize2 size={13} />}
            </button>
          )}
          <button onClick={onClose} title="fechar janela (continua rodando por baixo)">
            <X size={13} />
          </button>
        </span>
      </div>
      <div className="frame-body" style={collapsed ? { display: "none" } : undefined}>
        {children}
      </div>
    </div>
  );
}
