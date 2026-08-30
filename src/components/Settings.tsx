import { useEffect, useRef, useState } from "react";
import { Bot, Check, Mic2, Plug, Settings2, Wrench, X } from "lucide-react";
import * as ipc from "../lib/ipc";
import PluginsPanel from "./PluginsPanel";
import { t } from "../lib/i18n";
import {
  checkForUpdate,
  currentVersion,
  lastStatus,
  type UpdateStatus,
} from "../lib/updater";

type Section = "geral" | "plugins" | "voz" | "workers" | "avancado";

const MODE_OPTIONS: [string, string][] = [
  ["", "mode_cli"],
  ["manual", "mode_manual"],
  ["acceptEdits", "mode_accept"],
  ["plan", "mode_plan"],
  ["auto", "mode_auto"],
  ["bypassPermissions", "mode_bypass"],
];

/**
 * App settings over ~/.hark/config.toml — the FILE stays the source
 * of truth (writes are surgical patches; comments survive). Every field
 * saves on change and hot-applies where possible (theme, mode, ceilings,
 * even the hotkey re-registers). Lives on the MOTHER window only: these
 * are machine-level settings, not project ones.
 */
export default function Settings({ open, onClose }: { open: boolean; onClose: () => void }) {
  const [section, setSection] = useState<Section>("geral");
  const [snap, setSnap] = useState<ipc.ConfigSnapshot | null>(null);
  const [upState, setUpState] = useState<UpdateStatus>(lastStatus());
  const [version, setVersion] = useState("?");
  const [busy, setBusy] = useState(false);
  const [voices, setVoices] = useState<[string, string][]>([]);
  const [savedKey, setSavedKey] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const savedTimer = useRef<number>(0);

  useEffect(() => {
    if (!open) return;
    ipc.configRead().then(setSnap).catch(() => {});
    ipc.ttsVoices().then(setVoices).catch(() => setVoices([]));
    currentVersion().then(setVersion);
    setUpState(lastStatus());
  }, [open]);

  if (!open || !snap) return null;
  const v = snap.values;

  /** Look for an update right now instead of waiting for the next poll. */
  async function lookNow() {
    setBusy(true);
    setUpState({ kind: "checking" });
    setUpState(await checkForUpdate());
    setBusy(false);
  }

  // One line that says where this build stands. Silence was the bug.
  const updateLine = (() => {
    switch (upState.kind) {
      case "dev":
        return t("up_dev");
      case "checking":
        return t("up_checking");
      case "ready":
        return t("up_ready", { v: upState.version });
      case "current":
        return t("up_current", { v: version });
      case "error":
        return t("up_error", { err: upState.message.slice(0, 90) });
      default:
        return t("up_idle", { v: version });
    }
  })();

  /** Save one key now; the check mark confirms, errors show inline. */
  async function save(key: string, value: string | number | boolean) {
    setError(null);
    try {
      await ipc.configWrite({ [key]: value });
      const fresh = await ipc.configRead();
      setSnap(fresh);
      setSavedKey(key);
      window.clearTimeout(savedTimer.current);
      savedTimer.current = window.setTimeout(() => setSavedKey(null), 1400);
    } catch (err) {
      setError(String(err));
    }
  }

  const saved = (key: string) =>
    savedKey === key ? <Check size={13} className="set-saved" /> : null;

  /** Text field that saves on blur/Enter (numbers coerced when asked). */
  function Field({
    label,
    hint,
    keyName,
    value,
    number,
    placeholder,
  }: {
    label: string;
    hint?: string;
    keyName: string;
    value: string | number;
    number?: boolean;
    placeholder?: string;
  }) {
    const [draft, setDraft] = useState(String(value));
    useEffect(() => setDraft(String(value)), [value]);
    const commit = () => {
      if (draft === String(value)) return;
      save(keyName, number ? Number(draft.replace(",", ".")) || 0 : draft.trim());
    };
    return (
      <div className="set-row">
        <div className="set-label">
          <b>{label} {saved(keyName)}</b>
          {hint && <span>{hint}</span>}
        </div>
        <input
          value={draft}
          placeholder={placeholder}
          inputMode={number ? "decimal" : undefined}
          onChange={(e) => setDraft(e.target.value)}
          onBlur={commit}
          onKeyDown={(e) => {
            if (e.key === "Enter") (e.target as HTMLInputElement).blur();
          }}
        />
      </div>
    );
  }

  function Select({
    label,
    hint,
    keyName,
    value,
    options,
  }: {
    label: string;
    hint?: string;
    keyName: string;
    value: string;
    options: [string, string][];
  }) {
    return (
      <div className="set-row">
        <div className="set-label">
          <b>{label} {saved(keyName)}</b>
          {hint && <span>{hint}</span>}
        </div>
        <select value={value} onChange={(e) => save(keyName, e.target.value)}>
          {options.map(([val, name]) => (
            <option key={val} value={val}>
              {name}
            </option>
          ))}
        </select>
      </div>
    );
  }

  /** Known CLI aliases + whatever the config already has + free text.
   *  The CLI has no "list models" call, so the list is the stable aliases;
   *  "outro…" opens a text field for pinned ids (claude-sonnet-5…). */
  function ModelSelect({ keyName, value }: { keyName: string; value: string }) {
    const [custom, setCustom] = useState(false);
    const known = ["sonnet", "opus", "haiku"];
    const options = known.includes(value) ? known : [value, ...known];
    if (custom) {
      return (
        <Field
          label={t("set_model")}
          hint={t("set_model_custom_hint")}
          keyName={keyName}
          value={value}
          placeholder="claude-sonnet-5"
        />
      );
    }
    return (
      <div className="set-row">
        <div className="set-label">
          <b>{t("set_model")} {saved(keyName)}</b>
          <span>{t("set_model_hint")}</span>
        </div>
        <select
          value={value}
          onChange={(e) => {
            if (e.target.value === "__custom__") setCustom(true);
            else save(keyName, e.target.value);
          }}
        >
          {options.map((m) => (
            <option key={m} value={m}>
              {m}
            </option>
          ))}
          <option value="__custom__">{t("set_model_other")}</option>
        </select>
      </div>
    );
  }

  const sections: { id: Section; name: string; icon: React.ReactNode }[] = [
    { id: "geral", name: t("set_general"), icon: <Settings2 size={14} /> },
    { id: "plugins", name: t("set_plugins"), icon: <Plug size={14} /> },
    { id: "voz", name: t("set_voice"), icon: <Mic2 size={14} /> },
    { id: "workers", name: t("set_workers"), icon: <Bot size={14} /> },
    { id: "avancado", name: t("set_advanced"), icon: <Wrench size={14} /> },
  ];

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal settings" onClick={(e) => e.stopPropagation()}>
        <nav className="set-nav">
          <h3>{t("set_title")}</h3>
          {sections.map((s) => (
            <button
              key={s.id}
              className={section === s.id ? "on" : ""}
              onClick={() => setSection(s.id)}
            >
              {s.icon} {s.name}
            </button>
          ))}
          <div className="set-path" title={snap.path}>
            {snap.path.replace(/^\/Users\/[^/]+/, "~")}
          </div>
        </nav>

        <div className="set-body">
          <button className="set-close" onClick={onClose} title={t("set_close")}>
            <X size={15} />
          </button>
          {error && <div className="gate-warning">⚠ {error}</div>}

          {section === "geral" && (
            <>
              <h2>{t("set_general")}</h2>
              <Field
                label={t("set_assistant_name")}
                hint={t("set_assistant_hint")}
                keyName="assistant_name"
                value={v.assistant_name}
                placeholder="Hark"
              />
              <div className="set-row">
                <div className="set-label">
                  <b>{t("set_persona")}</b>
                  <span>{t("set_persona_hint")}</span>
                </div>
                <button
                  className="set-open"
                  onClick={() =>
                    ipc.openExternal(`${snap.data_dir}/CLAUDE.md`).catch(() => {})
                  }
                >
                  {t("set_persona_open")}
                </button>
              </div>
              <ModelSelect keyName="model" value={v.model} />
              <Select
                label={t("set_theme")}
                hint={t("set_theme_hint")}
                keyName="theme"
                value={v.theme}
                options={[
                  ["hark", t("set_theme_default")],
                  ["dracula", "Dracula"],
                ]}
              />
              <div className="set-preview" data-theme={v.theme === "hark" ? undefined : v.theme}>
                <pre>
                  <span className="hl-kw">function</span>{" "}
                  <span className="hl-title">greet</span>(name) {"{"}
                  {"\n"}  <span className="hl-kw">return</span>{" "}
                  <span className="hl-str">`Hello, ${"{"}name{"}"}!`</span>;{"\n"}
                  {"}"}
                </pre>
              </div>
              <Select
                label={t("set_ui_lang")}
                hint={t("set_ui_lang_hint")}
                keyName="ui_language"
                value={v.ui_language}
                options={[
                  ["pt", t("lang_pt")],
                  ["en", t("lang_en")],
                ]}
              />
              <div className="set-row">
                <div className="set-label">
                  <b>{t("set_auto_update")} {saved("auto_update")}</b>
                  <span>{updateLine}</span>
                </div>
                <div className="set-update">
                  <button className="set-open" onClick={lookNow} disabled={busy}>
                    {busy ? t("up_checking") : t("up_check_now")}
                  </button>
                  <input
                    type="checkbox"
                    className="set-check"
                    checked={v.auto_update}
                    onChange={(e) => save("auto_update", e.target.checked)}
                  />
                </div>
              </div>
              <Field
                label={t("set_hotkey")}
                hint={t("set_hotkey_hint")}
                keyName="hotkey"
                value={v.hotkey}
                placeholder="cmd+shift+space"
              />
            </>
          )}

          {section === "plugins" && <PluginsPanel />}

          {section === "voz" && (
            <>
              <h2>{t("set_voice")}</h2>
              <Select
                label={t("set_tts")}
                hint={t("set_tts_hint")}
                keyName="voice"
                value={v.voice}
                options={
                  voices.length > 0
                    ? voices.map(([name, lang]) => [name, `${name} · ${lang}`])
                    : [[v.voice, v.voice]]
                }
              />
              <Select
                label={t("set_engine")}
                hint={t("set_engine_hint")}
                keyName="stt"
                value={v.stt ?? "whisper"}
                options={[
                  ["whisper", t("engine_whisper")],
                  ["external", t("engine_external")],
                ]}
              />
              <Select
                label={t("set_stt")}
                hint={t("set_stt_hint")}
                keyName="language"
                value={v.language}
                options={[
                  ["pt", t("lang_pt")],
                  ["en", t("lang_en")],
                  ["auto", t("lang_auto")],
                ]}
              />
            </>
          )}

          {section === "workers" && (
            <>
              <h2>{t("set_workers")}</h2>
              <Select
                label={t("set_mode")}
                hint={t("set_mode_hint")}
                keyName="worker_mode"
                value={v.worker_mode}
                options={MODE_OPTIONS.map(([val, k]) => [val, t(k as never)] as [string, string])}
              />
              <Field
                label={t("set_budget")}
                hint={t("set_budget_hint")}
                keyName="worker_budget_usd"
                value={v.worker_budget_usd}
                number
              />
              <Field
                label={t("set_turns")}
                hint={t("set_turns_hint")}
                keyName="worker_max_turns"
                value={v.worker_max_turns}
                number
              />
              <Field
                label={t("set_prompt")}
                hint={t("set_prompt_hint")}
                keyName="prompt_budget_chars"
                value={v.prompt_budget_chars}
                number
              />
            </>
          )}

          {section === "avancado" && (
            <>
              <h2>{t("set_advanced")}</h2>
              <Field
                label={t("set_claude")}
                hint={t("set_claude_hint", { path: snap.claude_bin_resolved })}
                keyName="claude_bin"
                value={v.claude_bin}
              />
              <Field
                label={t("set_whisper")}
                hint={t("set_whisper_hint", { path: snap.whisper_model_resolved })}
                keyName="whisper_model"
                value={v.whisper_model}
                placeholder="~/.local/share/hark/models/ggml-small.bin"
              />
              <div className="set-row">
                <div className="set-label">
                  <b>{t("set_data")}</b>
                  <span>{t("set_data_hint")}</span>
                </div>
                <code className="set-ro">{snap.data_dir}</code>
              </div>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
