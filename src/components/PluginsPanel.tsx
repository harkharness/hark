import { useCallback, useEffect, useState } from "react";
import { Check, Plug, RefreshCw, TerminalSquare } from "lucide-react";
import * as ipc from "../lib/ipc";
import { t } from "../lib/i18n";

/** Capability flags worth showing, in the order the product cares about. */
const CAP_ROWS: [keyof ipc.AgentCapabilities, string][] = [
  ["permissions", "pl_cap_perm"],
  ["cost_reporting", "pl_cap_cost"],
  ["history", "pl_cap_hist"],
  ["resume", "pl_cap_resume"],
  ["slash_commands", "pl_cap_slash"],
  ["live_list", "pl_cap_live"],
];

/**
 * The agent-plugin catalog — the surface where a backend is chosen. Shared
 * by Settings (a section) and the first-run wizard (a step): the product
 * has ONE place that answers "which agent drives Hark?".
 */
export default function PluginsPanel({
  onReady,
  compact = false,
}: {
  /** Fires with true once a usable plugin is selected and detected. */
  onReady?: (ready: boolean) => void;
  compact?: boolean;
}) {
  const [plugins, setPlugins] = useState<ipc.AgentPlugin[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    setBusy(true);
    try {
      const list = await ipc.agentPlugins();
      setPlugins(list);
      onReady?.(list.some((p) => p.selected && p.detected));
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }, [onReady]);

  useEffect(() => {
    void load();
  }, [load]);

  const select = async (id: string) => {
    try {
      await ipc.agentPluginSelect(id);
      await load();
    } catch (err) {
      setError(String(err));
    }
  };

  /** On/off is config (`[agents.<id>] enabled`); the card is the switch. */
  const toggle = async (id: string, enabled: boolean) => {
    try {
      await ipc.agentPluginEnable(id, enabled);
      await load();
    } catch (err) {
      setError(String(err));
    }
  };

  return (
    <div className="plugins">
      {!compact && (
        <div className="plugins-intro">
          <Plug size={14} /> {t("pl_intro")}
        </div>
      )}
      {error && <div className="ob-warn">{error}</div>}

      {plugins.map((p) => {
        // Runnable: in the registry, installed, switched on. Both plugins
        // (native claude, ACP) have a runtime now.
        const usable = p.detected && p.enabled;
        return (
          <div
            key={p.id}
            className={`plugin-card ${p.selected ? "on" : ""} ${usable ? "" : "soon"}`}
            onClick={() => usable && !p.selected && select(p.id)}
          >
            <div className="plugin-head">
              <div className="plugin-name">
                {p.name}
                <span className="plugin-crate">{p.cmd}</span>
              </div>
              {p.selected ? (
                <span className="plugin-badge on">
                  <Check size={11} /> {t("pl_in_use")}
                </span>
              ) : usable ? (
                <span className="plugin-badge pick">{t("pl_use")}</span>
              ) : !p.enabled ? (
                // Off in config. The badge IS the switch: a built-in that
                // ships off (claude over ACP) was unreachable without
                // editing the file.
                <button
                  className="plugin-badge plugin-toggle"
                  title={t("pl_enable_hint")}
                  onClick={(e) => {
                    e.stopPropagation();
                    void toggle(p.id, true);
                  }}
                >
                  {t("pl_off")} · {t("pl_enable")}
                </button>
              ) : (
                <span className="plugin-badge warn">{t("pl_missing")}</span>
              )}
              {p.enabled && !p.selected && (
                <button
                  className="plugin-toggle-off"
                  title={t("pl_disable_hint")}
                  onClick={(e) => {
                    e.stopPropagation();
                    void toggle(p.id, false);
                  }}
                >
                  {t("pl_disable")}
                </button>
              )}
            </div>

            {p.detected && p.detail && <div className="plugin-path">{p.detail}</div>}

            {!p.detected && p.enabled && (
              <div className="plugin-install">
                <div className="ob-warn">
                  <TerminalSquare size={13} /> {t("pl_not_found", { name: p.name })}
                </div>
                <pre className="ob-code">{p.install}</pre>
                <button
                  className="ob-btn"
                  disabled={busy}
                  onClick={(e) => {
                    e.stopPropagation();
                    void load();
                  }}
                >
                  <RefreshCw size={12} /> {t("pl_recheck")}
                </button>
              </div>
            )}

            {p.capabilities && (
              <div className="plugin-caps">
                {CAP_ROWS.filter(([key]) => p.capabilities![key]).map(([key, label]) => (
                  <span key={key} className="plugin-cap">
                    {t(label as never)}
                  </span>
                ))}
              </div>
            )}
          </div>
        );
      })}

      <div className="plugins-foot">{t("pl_foot")}</div>
    </div>
  );
}
