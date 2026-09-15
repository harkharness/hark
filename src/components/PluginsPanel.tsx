import { useCallback, useEffect, useState } from "react";
import { ArrowUpCircle, Check, Plug, RefreshCw, TerminalSquare } from "lucide-react";
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
  const [checking, setChecking] = useState(false);

  /** The one network call of this panel: read the ACP registry. */
  const checkUpdates = useCallback(async () => {
    setChecking(true);
    try {
      const list = await ipc.agentRegistryRefresh();
      setPlugins(list);
    } catch (err) {
      setError(String(err));
    } finally {
      setChecking(false);
    }
  }, []);

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

  // Detected is not current: a reading older than a day (or none) is
  // refreshed when the catalog opens — opening it IS asking about the
  // agents. A stale answer would hide the update the user came for.
  const checkedAt = plugins[0]?.version?.checked_at ?? null;
  const loaded = plugins.length > 0;
  useEffect(() => {
    if (!loaded || checking) return;
    const age = checkedAt ? Date.now() - Date.parse(checkedAt) : Infinity;
    if (age > 24 * 3600 * 1000) void checkUpdates();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [loaded]);

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
                <span className="plugin-crate">
                  {p.cmd}
                  {p.version?.installed ? ` v${p.version.installed}` : ""}
                </span>
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

            {p.version?.freshness.state === "behind" && (
              // The migration command IS the fix: gemini 0.46's session/new
              // answered a deprecation notice, and nothing but an update
              // resolves it. Shown, never buried in a tooltip.
              <div className="plugin-install">
                <div className="ob-warn">
                  <ArrowUpCircle size={13} />{" "}
                  {t("pl_behind", { installed: p.version.freshness.installed, current: p.version.freshness.current })}
                </div>
                <pre className="ob-code">{p.version.update ?? p.install}</pre>
              </div>
            )}

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

            {Object.keys(p.models ?? {}).length > 0 && (
              // What this agent calls each tier: the model pill's rows in
              // a chat on it, and the `[agents.<id>.models]` knob made
              // visible.
              <div className="plugin-caps" title={t("pl_models")}>
                {(["light", "standard", "heavy", "max"] as const)
                  .filter((k) => p.models[k])
                  .map((k) => (
                    <span key={k} className="plugin-cap">
                      {k} · {p.models[k]}
                    </span>
                  ))}
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

      <div className="plugins-foot">
        {t("pl_foot")}
        <div className="plugins-registry">
          <span>
            {checking
              ? t("pl_checking")
              : checkedAt
                ? t("pl_checked_at", { when: new Date(checkedAt).toLocaleString() })
                : t("pl_never_checked")}
          </span>
          <button className="ob-btn" disabled={checking} onClick={() => void checkUpdates()}>
            <RefreshCw size={12} /> {t("pl_check_updates")}
          </button>
        </div>
      </div>
    </div>
  );
}
