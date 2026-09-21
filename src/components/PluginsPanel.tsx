import { useCallback, useEffect, useState } from "react";
import { ArrowUpCircle, Check, Plug, Plus, RefreshCw, TerminalSquare } from "lucide-react";
import * as ipc from "../lib/ipc";
import AgentMark from "./AgentMark";
import AgentDetail, { CAP_ROWS } from "./AgentDetail";
import { markStateOf } from "../lib/agentMark";
import { t } from "../lib/i18n";

/** A registry id: what `[agents.<id>]` accepts without quoting. */
const ID_OK = /^[a-z0-9][a-z0-9_-]*$/;

/** One line under the name: what this agent runs, or what is wrong. */
function cardMeta(p: ipc.AgentPlugin) {
  if (!p.enabled) return <span className="plugin-meta-off">{t("pl_off")}</span>;
  if (!p.detected) return <span className="plugin-meta-warn">{t("pl_missing")}</span>;
  if (p.version?.freshness.state === "behind")
    return (
      <span className="plugin-meta-warn">
        <ArrowUpCircle size={11} /> {t("pl_card_update", { v: p.version.freshness.current })}
      </span>
    );
  const model = p.models?.standard ?? p.models?.light ?? "";
  const caps = p.capabilities ? CAP_ROWS.filter(([k]) => p.capabilities![k]).length : 0;
  return <span>{[model, caps ? t("pl_card_caps", { n: caps }) : ""].filter(Boolean).join(" · ")}</span>;
}

/**
 * The agent-plugin catalog — the surface where a backend is chosen.
 * Shared by Settings (a section) and the first-run wizard (a step): the
 * product has ONE place that answers "which agent drives Hark?".
 *
 * The grid carries identity and one line of meta; everything else lives
 * in the detail beside it. Clicking a card INSPECTS — only the detail's
 * button changes the driver. The wizard is the exception: with no room
 * for a detail, its only job IS picking, so there the click picks.
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
  const [inspect, setInspect] = useState<string | null>(null);
  /** The detail slot holds the new-agent form instead of an agent. */
  const [adding, setAdding] = useState(false);
  const [newId, setNewId] = useState("");
  const [newPlugin, setNewPlugin] = useState<"claude" | "acp">("claude");
  const [newCmd, setNewCmd] = useState("claude");

  /** The one network call of this panel: read the ACP registry. */
  const checkUpdates = useCallback(async () => {
    setChecking(true);
    try {
      setPlugins(await ipc.agentRegistryRefresh());
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
      // The detail opens on the agent that is driving: the answer the
      // screen exists for is on screen before anything is clicked.
      setInspect((cur) => cur ?? list.find((p) => p.selected)?.id ?? list[0]?.id ?? null);
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

  /** On/off is config (`[agents.<id>] enabled`); the detail is the switch. */
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

  // The first answer takes a moment (the backend probes every binary's
  // version): the shape of the cards, not an empty list that reads as
  // "no agents", and no form or footer under nothing.
  const loading = busy && plugins.length === 0;
  const shown = plugins.find((p) => p.id === inspect) ?? null;

  /** In the wizard the click picks; in Settings it opens the detail. */
  const onCard = (p: ipc.AgentPlugin) => {
    if (compact) {
      if (p.detected && p.enabled && !p.selected) void select(p.id);
      return;
    }
    setAdding(false);
    setInspect(p.id);
  };

  const pressable = (act: () => void) => ({
    role: "button",
    tabIndex: 0,
    onClick: act,
    onKeyDown: (e: React.KeyboardEvent) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        act();
      }
    },
  });

  return (
    <div className={`plugins ${compact ? "plugins-compact" : ""}`}>
      {!compact && (
        <div className="plugins-top">
          <div className="plugins-title">
            <h2>{t("set_plugins")}</h2>
            <p className="plugins-intro">
              <Plug size={13} /> {t("pl_intro")}
            </p>
          </div>
          {!loading && (
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
          )}
        </div>
      )}
      {error && <div className="ob-warn">{error}</div>}

      <div className="plugins-body">
        <div className="plugins-grid">
          {loading &&
            [0, 1, 2].map((i) => (
              <div key={i} className="plugin-card plugin-skeleton" aria-hidden>
                <div className="plugin-head">
                  <div className="sk-mark" />
                  <div className="sk-lines">
                    <div className="sk-line" style={{ width: "62%" }} />
                    <div className="sk-line thin" style={{ width: "84%" }} />
                  </div>
                </div>
                <div className="sk-line thin" style={{ width: "48%" }} />
              </div>
            ))}

          {plugins.map((p) => (
            <div
              key={p.id}
              className={`plugin-card ${inspect === p.id && !adding && !compact ? "inspect" : ""} ${
                p.selected ? "driving" : ""
              } ${p.enabled ? "" : "soon"}`}
              {...pressable(() => onCard(p))}
            >
              <div className="plugin-head">
                <AgentMark vendor={p.vendor} id={p.id} state={markStateOf(p)} />
                <div className="plugin-ident">
                  <div className="plugin-name-row">
                    <span className="plugin-name">{p.name}</span>
                    {p.selected && (
                      <span className="plugin-inuse">
                        <Check size={11} /> {t("pl_in_use")}
                      </span>
                    )}
                  </div>
                  {/* The id, not the binary: two entries can share one
                      binary, and the id is what names the config table. */}
                  <span className="plugin-crate">
                    {p.id}
                    {p.version?.installed ? ` · v${p.version.installed}` : ""}
                  </span>
                </div>
              </div>
              <div className="plugin-meta">{cardMeta(p)}</div>

              {/* The wizard has no detail panel to hold the fix, and a
                  fresh machine is exactly where the fix is needed: the
                  install line and the switch stay on the card there. */}
              {compact && !p.enabled && (
                <button
                  className="ob-btn"
                  title={t("pl_enable_hint")}
                  onClick={(e) => {
                    e.stopPropagation();
                    void toggle(p.id, true);
                  }}
                >
                  {t("pl_enable")}
                </button>
              )}
              {compact && p.enabled && !p.detected && p.install && (
                <div className="plugin-install">
                  <div className="ob-warn">
                    <TerminalSquare size={12} /> {t("pl_not_found", { name: p.name })}
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
            </div>
          ))}

          {!loading && onEditConfig && (
            <div
              className={`plugin-card plugin-add ${adding ? "inspect" : ""}`}
              {...pressable(() => setAdding(true))}
            >
              <div className="plugin-head">
                <span className="agent-mark plugin-add-mark">
                  <Plus size={15} />
                </span>
                <div className="plugin-ident">
                  <span className="plugin-name">{t("pl_add_agent")}</span>
                  <span className="plugin-crate">{t("pl_add_sub")}</span>
                </div>
              </div>
            </div>
          )}
        </div>

        {loading && <div className="plugins-loading-note">{t("pl_loading")}</div>}

        {!compact && adding && (
          <aside className="agent-detail">
            <header className="agent-detail-head">
              <span className="agent-mark plugin-add-mark big">
                <Plus size={18} />
              </span>
              <div className="agent-detail-ident">
                <div className="agent-detail-name">{t("pl_add_agent")}</div>
                <span className="agent-detail-crate">{t("pl_add_sub")}</span>
              </div>
            </header>
            <form
              className="plugins-add"
              onSubmit={(e) => {
                e.preventDefault();
                void addAgent();
              }}
            >
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
              <input
                value={newCmd}
                placeholder={t("pl_add_cmd_ph")}
                onChange={(e) => setNewCmd(e.target.value)}
              />
              <button
                className="ob-btn accent"
                type="submit"
                disabled={!ID_OK.test(newId) || !newCmd.trim()}
              >
                <Plus size={12} /> {t("pl_add_go")}
              </button>
              <span className="plugins-add-hint">{t("pl_add_hint")}</span>
            </form>
          </aside>
        )}

        {!compact && !adding && shown && (
          <AgentDetail
            p={shown}
            busy={busy}
            onUse={() => void select(shown.id)}
            onToggle={(enabled) => void toggle(shown.id, enabled)}
            onRecheck={() => void load()}
            onEditConfig={onEditConfig}
          />
        )}
      </div>

      {!compact && !loading && <div className="plugins-foot">{t("pl_foot")}</div>}
    </div>
  );
}
