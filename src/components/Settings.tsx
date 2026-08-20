import { useEffect, useRef, useState } from "react";
import { Bot, Check, Mic2, Settings2, Wrench, X } from "lucide-react";
import * as ipc from "../lib/ipc";

type Section = "geral" | "voz" | "workers" | "avancado";

const MODE_OPTIONS: [string, string][] = [
  ["", "padrão do CLI (pergunta tudo)"],
  ["manual", "Manual"],
  ["acceptEdits", "Aceitar edições"],
  ["plan", "Planejar"],
  ["auto", "Automático"],
  ["bypassPermissions", "Ignorar permissões"],
];

/**
 * App settings over ~/.config/vox/config.toml — the FILE stays the source
 * of truth (writes are surgical patches; comments survive). Every field
 * saves on change and hot-applies where possible (theme, mode, ceilings,
 * even the hotkey re-registers). Lives on the MOTHER window only: these
 * are machine-level settings, not project ones.
 */
export default function Settings({ open, onClose }: { open: boolean; onClose: () => void }) {
  const [section, setSection] = useState<Section>("geral");
  const [snap, setSnap] = useState<ipc.ConfigSnapshot | null>(null);
  const [voices, setVoices] = useState<[string, string][]>([]);
  const [savedKey, setSavedKey] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const savedTimer = useRef<number>(0);

  useEffect(() => {
    if (!open) return;
    ipc.configRead().then(setSnap).catch(() => {});
    ipc.ttsVoices().then(setVoices).catch(() => setVoices([]));
  }, [open]);

  if (!open || !snap) return null;
  const v = snap.values;

  /** Save one key now; the check mark confirms, errors show inline. */
  async function save(key: string, value: string | number) {
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
          label="Modelo padrão"
          hint="id completo ou alias — Enter salva"
          keyName={keyName}
          value={value}
          placeholder="claude-sonnet-5"
        />
      );
    }
    return (
      <div className="set-row">
        <div className="set-label">
          <b>Modelo padrão {saved(keyName)}</b>
          <span>alias passado ao claude --model</span>
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
          <option value="__custom__">outro…</option>
        </select>
      </div>
    );
  }

  const sections: { id: Section; name: string; icon: React.ReactNode }[] = [
    { id: "geral", name: "Geral", icon: <Settings2 size={14} /> },
    { id: "voz", name: "Voz", icon: <Mic2 size={14} /> },
    { id: "workers", name: "Workers", icon: <Bot size={14} /> },
    { id: "avancado", name: "Avançado", icon: <Wrench size={14} /> },
  ];

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal settings" onClick={(e) => e.stopPropagation()}>
        <nav className="set-nav">
          <h3>Configurações</h3>
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
          <button className="set-close" onClick={onClose} title="fechar (Esc)">
            <X size={15} />
          </button>
          {error && <div className="gate-warning">⚠ {error}</div>}

          {section === "geral" && (
            <>
              <h2>Geral</h2>
              <ModelSelect keyName="model" value={v.model} />
              <Select
                label="Tema do código"
                hint="blocos do chat, editor e terminal"
                keyName="theme"
                value={v.theme}
                options={[
                  ["vox", "Vox (padrão)"],
                  ["dracula", "Dracula"],
                ]}
              />
              <div className="set-preview" data-theme={v.theme === "vox" ? undefined : v.theme}>
                <pre>
                  <span className="hl-kw">function</span>{" "}
                  <span className="hl-title">greet</span>(name) {"{"}
                  {"\n"}  <span className="hl-kw">return</span>{" "}
                  <span className="hl-str">`Hello, ${"{"}name{"}"}!`</span>;{"\n"}
                  {"}"}
                </pre>
              </div>
              <Field
                label="Atalho global"
                hint="abre o HUD de voz de qualquer app — aplica na hora"
                keyName="hotkey"
                value={v.hotkey}
                placeholder="cmd+shift+space"
              />
            </>
          )}

          {section === "voz" && (
            <>
              <h2>Voz</h2>
              <Select
                label="Voz das respostas"
                hint="vozes do sistema (say); pt primeiro"
                keyName="voice"
                value={v.voice}
                options={
                  voices.length > 0
                    ? voices.map(([name, lang]) => [name, `${name} · ${lang}`])
                    : [[v.voice, v.voice]]
                }
              />
              <Select
                label="Idioma da transcrição"
                hint="o que o whisper espera ouvir"
                keyName="language"
                value={v.language}
                options={[
                  ["pt", "Português (Brasil)"],
                  ["en", "Inglês"],
                ]}
              />
            </>
          )}

          {section === "workers" && (
            <>
              <h2>Workers</h2>
              <Select
                label="Modo de permissão padrão"
                hint="novas tasks nascem neste modo (falado > seletor > isto)"
                keyName="worker_mode"
                value={v.worker_mode}
                options={MODE_OPTIONS}
              />
              <Field
                label="Teto por worker (USD)"
                hint="--max-budget-usd; 0 desliga — a trava pós-incidente"
                keyName="worker_budget_usd"
                value={v.worker_budget_usd}
                number
              />
              <Field
                label="Máximo de turnos"
                hint="0 = sem limite"
                keyName="worker_max_turns"
                value={v.worker_max_turns}
                number
              />
              <Field
                label="Orçamento do prompt (chars)"
                hint="tamanho do contexto montado pro ask (~chars/4 tokens)"
                keyName="prompt_budget_chars"
                value={v.prompt_budget_chars}
                number
              />
            </>
          )}

          {section === "avancado" && (
            <>
              <h2>Avançado</h2>
              <Field
                label="Binário do claude"
                hint={`resolvido: ${snap.claude_bin_resolved}`}
                keyName="claude_bin"
                value={v.claude_bin}
              />
              <Field
                label="Modelo do whisper"
                hint={`em uso: ${snap.whisper_model_resolved} · muda no próximo boot`}
                keyName="whisper_model"
                value={v.whisper_model}
                placeholder="~/.local/share/vox/models/ggml-small.bin"
              />
              <div className="set-row">
                <div className="set-label">
                  <b>Dados locais</b>
                  <span>índice, ledger e modelos ficam aqui — nada sai da máquina</span>
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
