import { ArrowUpCircle, Check, FileCode2, KeyRound, RefreshCw, TerminalSquare } from "lucide-react";
import AgentMark from "./AgentMark";
import { markStateOf } from "../lib/agentMark";
import * as ipc from "../lib/ipc";
import { t } from "../lib/i18n";

/** Capability flags worth showing, in the order the product cares about.
 *  The card counts these same six, so the two never disagree. */
export const CAP_ROWS: [keyof ipc.AgentCapabilities, string][] = [
  ["permissions", "pl_cap_perm"],
  ["cost_reporting", "pl_cap_cost"],
  ["history", "pl_cap_hist"],
  ["resume", "pl_cap_resume"],
  ["slash_commands", "pl_cap_slash"],
  ["live_list", "pl_cap_live"],
];

const TIERS = ["light", "standard", "heavy", "max"] as const;

function Sec({ label }: { label: string }) {
  return <div className="agent-sec">{label}</div>;
}

function Row({ label, children }: { label: React.ReactNode; children: React.ReactNode }) {
  return (
    <div className="agent-row">
      <div className="agent-row-label">{label}</div>
      <div className="agent-row-value">{children}</div>
    </div>
  );
}

/**
 * Everything the catalog knows about ONE agent — the part that used to
 * be crammed into every card as pills of equal weight. Reading here
 * costs nothing: only the button at the top changes which agent drives
 * the sessions.
 */
export default function AgentDetail({
  p,
  busy,
  onUse,
  onToggle,
  onRecheck,
  onEditConfig,
}: {
  p: ipc.AgentPlugin;
  busy: boolean;
  onUse: () => void;
  onToggle: (enabled: boolean) => void;
  onRecheck: () => void;
  onEditConfig?: (table: string) => void;
}) {
  const state = markStateOf(p);
  const usable = p.detected && p.enabled;
  const tiers = TIERS.filter((k) => p.models?.[k]);

  return (
    <aside className="agent-detail">
      <header className="agent-detail-head">
        <AgentMark vendor={p.vendor} id={p.id} state={state} size={42} />
        <div className="agent-detail-ident">
          <div className="agent-detail-name">{p.name}</div>
          <span className="agent-detail-crate">
            {p.cmd}
            {p.version?.installed ? ` · v${p.version.installed}` : ""}
          </span>
        </div>
        {p.selected ? (
          <span className="agent-badge on">
            <Check size={12} /> {t("pl_in_use")}
          </span>
        ) : usable ? (
          <button className="ob-btn accent" title={t("pl_use_hint")} onClick={onUse}>
            {t("pl_use")}
          </button>
        ) : !p.enabled ? (
          <button className="ob-btn" title={t("pl_enable_hint")} onClick={() => onToggle(true)}>
            {t("pl_enable")}
          </button>
        ) : (
          <span className="agent-badge warn">{t("pl_missing")}</span>
        )}
      </header>

      <Sec label={t("pl_detail_identity")} />
      <Row label={t("pl_detail_table")}>
        <code className="agent-table">[agents.{p.id}]</code>
      </Row>
      <Row label={t("pl_detail_bin")}>
        {p.detected ? (
          <code>{p.detail || p.cmd}</code>
        ) : (
          <span className="agent-warn-text">{t("pl_no_path")}</span>
        )}
      </Row>
      {(p.args ?? []).length > 0 && (
        <Row label={t("pl_detail_cmd")}>
          <code>
            {p.cmd} {p.args.join(" ")}
          </code>
        </Row>
      )}

      {!p.detected && p.enabled && p.install && (
        <div className="agent-fix">
          <div className="ob-warn">
            <TerminalSquare size={13} /> {t("pl_not_found", { name: p.name })}
          </div>
          <pre className="ob-code">{p.install}</pre>
          <button className="ob-btn" disabled={busy} onClick={onRecheck}>
            <RefreshCw size={12} /> {t("pl_recheck")}
          </button>
        </div>
      )}

      {p.version?.freshness.state === "behind" && (
        // The migration command IS the fix: nothing but an update
        // resolves a deprecation notice. Shown, never in a tooltip.
        <div className="agent-fix">
          <div className="ob-warn">
            <ArrowUpCircle size={13} />{" "}
            {t("pl_behind", {
              installed: p.version.freshness.installed,
              current: p.version.freshness.current,
            })}
          </div>
          <pre className="ob-code">{p.version.update ?? p.install}</pre>
        </div>
      )}

      {tiers.length > 0 && (
        <>
          <Sec label={t("pl_models")} />
          <div className="agent-tiers">
            {tiers.map((k) => (
              <div key={k} className="agent-tier">
                <span className="agent-tier-name">{k}</span>
                <code>{p.models[k]}</code>
              </div>
            ))}
          </div>
        </>
      )}

      {(p.env_keys ?? []).length > 0 && (
        <>
          <Sec label={t("pl_detail_env")} />
          {p.env_keys.map((key) => (
            <Row key={key} label={<code>{key}</code>}>
              <span className="agent-secret" title={t("pl_env_hint")}>
                <code>•••••••</code> {t("pl_env_in_file")}
              </span>
            </Row>
          ))}
        </>
      )}

      <Sec label={t("pl_detail_caps")} />
      {p.capabilities ? (
        <div className="agent-caps">
          {CAP_ROWS.map(([key, label]) => {
            const has = Boolean(p.capabilities![key]);
            return (
              <span key={key} className={`agent-cap ${has ? "on" : "off"}`}>
                {has ? <Check size={12} /> : <span className="agent-cap-dash" />}
                {t(label as never)}
              </span>
            );
          })}
        </div>
      ) : (
        <div className="agent-caps-unknown">{t("pl_caps_unknown")}</div>
      )}

      {p.login_hint && (
        <>
          <Sec label={t("pl_detail_login")} />
          <div className="agent-login">
            <KeyRound size={12} /> <code>{p.login_hint}</code>
          </div>
        </>
      )}

      <div className="agent-detail-foot">
        {p.enabled && !p.selected && (
          <button className="agent-off" title={t("pl_disable_hint")} onClick={() => onToggle(false)}>
            {t("pl_disable")}
          </button>
        )}
        {onEditConfig && (
          <button
            className="agent-edit"
            title={t("pl_edit_config")}
            onClick={() => onEditConfig(`agents.${p.id}`)}
          >
            <FileCode2 size={12} /> {t("pl_edit_in", { table: `agents.${p.id}` })} ›
          </button>
        )}
      </div>
    </aside>
  );
}
