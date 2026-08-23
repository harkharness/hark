import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Check, ChevronLeft, Download, Mic, TerminalSquare } from "lucide-react";
import * as ipc from "../lib/ipc";
import { setLang, setSpeechLang, t } from "../lib/i18n";

type DlState =
  | { kind: "idle" }
  | { kind: "downloading"; key: string; pct: number }
  | { kind: "done" }
  | { kind: "error"; err: string };

/**
 * First-run wizard on the mother window: identity → claude CLI check →
 * whisper model download (verified) → mic test. Every step is skippable;
 * finishing (or skipping) marks the machine onboarded.
 */
export default function Onboarding({
  status,
  onClose,
}: {
  status: ipc.SetupStatus;
  onClose: () => void;
}) {
  const [step, setStep] = useState(0);
  const [lang, setLangState] = useState(status.language || "pt");
  const [name, setName] = useState(status.assistant_name || "Hark");
  const [hotkey, setHotkey] = useState(status.hotkey || "cmd+shift+space");
  const [claude, setClaude] = useState({ ok: status.claude_ok, bin: status.claude_bin });
  const [whisperOk, setWhisperOk] = useState(status.whisper_ok);
  const [dl, setDl] = useState<DlState>({ kind: "idle" });
  const [mic, setMic] = useState<{ kind: "idle" | "listening" | "heard" | "fail"; text?: string }>(
    { kind: "idle" },
  );

  useEffect(() => {
    const un = listen<{ key: string; pct?: number; done?: boolean; error?: string }>(
      "hark-setup",
      (ev) => {
        const p = ev.payload;
        if (p.error) setDl({ kind: "error", err: p.error });
        else if (p.done) {
          setDl({ kind: "done" });
          setWhisperOk(true);
        } else if (typeof p.pct === "number")
          setDl({ kind: "downloading", key: p.key, pct: p.pct });
      },
    );
    return () => {
      un.then((f) => f());
    };
  }, []);

  const saveIdentity = async () => {
    await ipc.configWrite({
      language: lang,
      ui_language: lang,
      assistant_name: name.trim() || "Hark",
      hotkey: hotkey.trim() || "cmd+shift+space",
    });
    setLang(lang);
    setSpeechLang(lang);
  };

  const recheckClaude = async () => {
    const fresh = await ipc.setupStatus();
    setClaude({ ok: fresh.claude_ok, bin: fresh.claude_bin });
  };

  const testMic = async () => {
    setMic({ kind: "listening" });
    try {
      const text = await ipc.hearOnce("setup");
      setMic(text ? { kind: "heard", text } : { kind: "fail", text: "—" });
    } catch (err) {
      setMic({ kind: "fail", text: String(err) });
    }
  };

  const finish = async () => {
    await ipc.setupMarkDone();
    onClose();
  };

  const steps = [t("ob_s1_title"), t("ob_s2_title"), t("ob_s3_title"), t("ob_s4_title")];
  const last = step === steps.length - 1;

  return (
    <div className="ob-backdrop">
      <div className="ob-card">
        <header className="ob-head">
          <div className="ob-title">{t("ob_title")}</div>
          <div className="ob-sub">{t("ob_sub")}</div>
          <div className="ob-dots">
            {steps.map((label, i) => (
              <span key={label} className={`ob-dot ${i === step ? "on" : i < step ? "past" : ""}`}>
                {label}
              </span>
            ))}
          </div>
        </header>

        {step === 0 && (
          <section className="ob-body">
            <label className="ob-field">
              <span>{t("ob_s1_lang")}</span>
              <select value={lang} onChange={(e) => setLangState(e.target.value)}>
                <option value="pt">português</option>
                <option value="en">english</option>
              </select>
            </label>
            <label className="ob-field">
              <span>{t("ob_s1_name")}</span>
              <input value={name} onChange={(e) => setName(e.target.value)} />
            </label>
            <label className="ob-field">
              <span>{t("ob_s1_hotkey")}</span>
              <input value={hotkey} onChange={(e) => setHotkey(e.target.value)} />
            </label>
          </section>
        )}

        {step === 1 && (
          <section className="ob-body">
            {claude.ok ? (
              <div className="ob-ok">
                <Check size={14} /> {t("ob_s2_ok", { path: claude.bin })}
              </div>
            ) : (
              <>
                <div className="ob-warn">
                  <TerminalSquare size={14} /> {t("ob_s2_missing")}
                </div>
                <div className="ob-hint">{t("ob_s2_how")}</div>
                <pre className="ob-code">
                  curl -fsSL https://claude.ai/install.sh | bash{"\n"}claude
                </pre>
                <button className="ob-btn" onClick={recheckClaude}>
                  {t("ob_s2_recheck")}
                </button>
              </>
            )}
          </section>
        )}

        {step === 2 && (
          <section className="ob-body">
            <div className="ob-hint">{t("ob_s3_sub")}</div>
            {whisperOk && dl.kind !== "downloading" ? (
              <div className="ob-ok">
                <Check size={14} />{" "}
                {dl.kind === "done"
                  ? t("ob_s3_verified")
                  : t("ob_s3_have", { path: status.whisper_path })}
              </div>
            ) : (
              <div className="ob-models">
                {status.models.map((m) => (
                  <button
                    key={m.key}
                    className="ob-model"
                    disabled={dl.kind === "downloading"}
                    onClick={() => {
                      setDl({ kind: "downloading", key: m.key, pct: 0 });
                      void ipc.setupDownloadModel(m.key);
                    }}
                  >
                    <span className="ob-model-name">
                      <Download size={13} /> {m.key}
                    </span>
                    <span className="ob-model-sub">
                      {m.key === "small" ? t("ob_s3_small") : t("ob_s3_turbo")} · {m.size_label}
                    </span>
                  </button>
                ))}
              </div>
            )}
            {dl.kind === "downloading" && (
              <div className="ob-progress">
                <div className="ob-bar">
                  <div className="ob-fill" style={{ width: `${dl.pct}%` }} />
                </div>
                <span>{t("ob_s3_downloading", { pct: dl.pct })}</span>
              </div>
            )}
            {dl.kind === "error" && (
              <div className="ob-warn">{t("ob_s3_error", { err: dl.err })}</div>
            )}
          </section>
        )}

        {step === 3 && (
          <section className="ob-body">
            <div className="ob-hint">{t("ob_s4_sub")}</div>
            <button className="ob-btn accent" onClick={testMic} disabled={mic.kind === "listening"}>
              <Mic size={13} />{" "}
              {mic.kind === "listening" ? t("ob_s4_listening") : t("ob_s4_test")}
            </button>
            {mic.kind === "heard" && (
              <div className="ob-ok">
                <Check size={14} /> {t("ob_s4_heard", { text: mic.text ?? "" })}
              </div>
            )}
            {mic.kind === "fail" && (
              <div className="ob-warn">{t("ob_s4_fail", { err: mic.text ?? "" })}</div>
            )}
          </section>
        )}

        <footer className="ob-foot">
          <button className="ob-ghost" onClick={finish}>
            {t("ob_skip")}
          </button>
          <div className="ob-nav">
            {step > 0 && (
              <button className="ob-ghost" onClick={() => setStep(step - 1)}>
                <ChevronLeft size={13} /> {t("ob_back")}
              </button>
            )}
            <button
              className="ob-btn accent"
              onClick={async () => {
                if (step === 0) await saveIdentity();
                if (last) await finish();
                else setStep(step + 1);
              }}
            >
              {last ? t("ob_done") : t("ob_next")}
            </button>
          </div>
        </footer>
      </div>
    </div>
  );
}
