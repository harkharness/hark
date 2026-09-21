import { useCallback, useEffect, useState } from "react";
import { ArrowUpCircle, Check, FileCode2, KeyRound, Plug, Plus, RefreshCw, TerminalSquare } from "lucide-react";
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

/** A registry id: what `[agents.<id>]` accepts without quoting. */
const ID_OK = /^[a-z0-9][a-z0-9_-]*$/;

/**
 * The agent-plugin catalog — the surface where a backend is chosen. Shared
 * by Settings (a section) and the first-run wizard (a step): the product
 * has ONE place that answers "which agent drives Hark?". Each card shows
 * the knobs its `[agents.<id>]` table carries (env NAMES, args, the login
 * hint) and, where an editor is at hand, opens the file on that table.
 */
export default function PluginsPanel({
  onReady,
  compact = false,
  onEditConfig,
}: {
  /** Fires with true once a usable plugin is selected and detected. */
  onReady?: (ready: boolean) => void;
  compact?: boolean;
  /** Open config.toml on this table (`agents.<id>`). Settings has an
   *  editor; the wizard does not, so it passes nothing and gets no button. */
  onEditConfig?: (table: string) => void;
}) {
  const [plugins, setPlugins] = useState<ipc.AgentPlugin[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [checking, setChecking] = useState(false);
  const [newId, setNewId] = useState("");
  const [newPlugin, setNewPlugin] = useState<"claude" | "acp">("claude");
  const [newCmd, setNewCmd] = useState("claude");

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

  /** A new `[agents.<id>]` with the three keys every entry needs, written
   *  as a surgical patch; env and models are filled in the editor, where
   *  the user lands next. */
  const addAgent = async () => {
    const id = newId.trim();
    const cmd = newCmd.trim();
    if (!ID_OK.test(id) || !cmd || !onEditConfig) return;
    try {
      await ipc.configWrite({
        [`agents.${id}.plugin`]: newPlugin,
        [`agents.${id}.cmd`]: cmd,
        [`agents.${id}.enabled`]: true,
      });
      onEditConfig(`agents.${id}`);
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
        const envKeys = p.env_keys ?? [];
        const args = p.args ?? [];
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

            {(envKeys.length > 0 || args.length > 0 || p.login_hint) && (
              // The rest of the `[agents.<id>]` table: how the binary is
              // called, which env vars its processes get (names only — the
              // values are secrets and stay in the file), what to do when
              // it answers "not logged in".
              <div className="plugin-knobs">
                {args.length > 0 && (
                  <code className="plugin-args">
                    {p.cmd} {args.join(" ")}
                  </code>
                )}
                {envKeys.map((key) => (
                  <span key={key} className="plugin-cap plugin-env" title={t("pl_env_hint")}>
                    env · {key}
                  </span>
                ))}
                {p.login_hint && (
                  <span className="plugin-hint">
                    <KeyRound size={11} /> {p.login_hint}
                  </span>
                )}
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

            {onEditConfig && (
              <button
                className="plugin-edit"
                title={t("pl_edit_config")}
                onClick={(e) => {
                  e.stopPropagation();
                  onEditConfig(`agents.${p.id}`);
                }}
              >
                <FileCode2 size={12} /> config.toml ›
              </button>
            )}
          </div>
        );
      })}

      {onEditConfig && (
        <form
          className="plugins-add"
          onSubmit={(e) => {
            e.preventDefault();
            void addAgent();
          }}
        >
          <span className="plugins-add-label">{t("pl_add_agent")}</span>
          <input
            value={newId}
            placeholder={t("pl_add_id_ph")}
            onChange={(e) => setNewId(e.target.value.trim())}
          />
          <select
            value={newPlugin}
            onChange={(e) => {
              const plugin = e.target.value as "claude" | "acp";
              setNewPlugin(plugin);
              // The native plugin runs one binary; an ACP agent names its own.
              if (plugin === "claude") setNewCmd("claude");
              else if (newCmd === "claude") setNewCmd("");
            }}
          >
            <option value="claude">claude</option>
            <option value="acp">acp</option>
          </select>
          <input value={newCmd} placeholder={t("pl_add_cmd_ph")} onChange={(e) => setNewCmd(e.target.value)} />
          <button className="ob-btn" type="submit" disabled={!ID_OK.test(newId) || !newCmd.trim()}>
            <Plus size={12} /> {t("pl_add_go")}
          </button>
          <span className="plugins-add-hint">{t("pl_add_hint")}</span>
        </form>
      )}

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
