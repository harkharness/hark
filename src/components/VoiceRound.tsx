import { CircleQuestionMark, FolderPlus, Mic, ShieldCheck, Target } from "lucide-react";
import { t } from "../lib/i18n";
import { confirmSpeed } from "../lib/voice";
import { CONFIRM_MS, type Stage } from "../hooks/useVoiceRound";

/** The listening waveform. */
export function Waveform() {
  return (
    <span className="hud-wave">
      <span />
      <span />
      <span />
      <span />
      <span />
    </span>
  );
}

/**
 * One voice round on screen: the same rows whether they float in the HUD
 * window or sit in the composer. The stage says what to draw; the
 * callbacks are the hook's.
 */
export default function VoiceRound({
  stage,
  onPick,
  onTyped,
  onEscape,
}: {
  stage: Stage;
  onPick: (instruction: string, cand: import("../types").VoiceCandidate) => void;
  onTyped: (text: string) => void;
  onEscape: () => void;
}) {
  return (
    <>
      {stage.s === "listening" && (
        <div className="hud-row">
          <Waveform />
          <span className="hud-live">{t("hud_listening")}</span>
        </div>
      )}
      {stage.s === "loading" && (
        <div className="hud-row">
          <Waveform />
          <span className="hud-text warn">
            {stage.size ? t("mic_loading_sized", { size: stage.size, secs: stage.secs }) : t("mic_loading_plain", { secs: stage.secs })}
          </span>
        </div>
      )}
      {stage.s === "transcribing" && (
        <div className="hud-row">
          <Waveform />
          <span className="hud-live">{t("hud_transcribing", { secs: stage.secs })}</span>
        </div>
      )}
      {stage.s === "thinking" && (
        <div className="hud-row">
          <Mic size={14} className="hud-icon" />
          <span className="hud-text">“{stage.text}”</span>
          <span className="hud-sub">{t("hud_routing")}</span>
        </div>
      )}
      {stage.s === "confirm" && (
        <>
          <div className="hud-row">
            <Mic size={14} className="hud-icon" />
            <span className="hud-text">“{stage.text}”</span>
          </div>
          <div className="hud-row hud-dest">
            <span className="hud-sub">{t("hud_goes_to")}</span>
            <span className={`hud-chip ${stage.plan.new_task ? "new" : ""}`}>
              {stage.plan.new_task ? <FolderPlus size={12} /> : <Target size={12} />}
              {stage.plan.task_title ?? t("hud_new_chat_in", { name: stage.plan.project_name ?? "?" })}
              {stage.plan.project_name && stage.plan.task_title ? ` · ${stage.plan.project_name}` : ""}
            </span>
            <span className="hud-keys">{confirmSpeed(stage.plan) === "fast" ? t("hud_keys_fast") : t("hud_keys_verdict")}</span>
          </div>
          {stage.plan.warnings.map((w) => (
            <div key={w} className="hud-row hud-warning">
              ⚠ {w} {t("hud_warn_hint")}
            </div>
          ))}
          {confirmSpeed(stage.plan) === "fast" && <div className="hud-progress" style={{ animationDuration: `${CONFIRM_MS}ms` }} />}
        </>
      )}
      {stage.s === "candidates" && (
        <>
          <div className="hud-row">
            <Mic size={14} className="hud-icon" />
            <span className="hud-text">“{stage.text}”</span>
            <span className="hud-sub">{t("hud_which")}</span>
          </div>
          <div className="hud-cands">
            {stage.options.map((o, i) => (
              <button key={i} onClick={() => onPick(stage.instruction, o)}>
                <i>{i + 1}</i>
                <span className="hud-cand-title">{o.title}</span>
                {o.project_name && <span className="hud-cand-proj">{o.project_name}</span>}
              </button>
            ))}
          </div>
        </>
      )}
      {stage.s === "running" && (
        <div className="hud-row">
          <Waveform />
          <span className="hud-text">→ {stage.target}</span>
          <span className="hud-sub">{t("hud_dispatching")}</span>
        </div>
      )}
      {stage.s === "asking" && (
        <div className="hud-row">
          <CircleQuestionMark size={14} className="hud-icon" />
          <span className="hud-text">“{stage.text}”</span>
          <span className="hud-sub">{t("hud_asking")}</span>
        </div>
      )}
      {stage.s === "answer" && (
        <div className="hud-row">
          <span className="hud-text hud-answer">{stage.text}</span>
        </div>
      )}
      {stage.s === "type" && (
        <input
          className="hud-type"
          autoFocus
          placeholder={t("hud_type_placeholder")}
          onKeyDown={(e) => {
            if (e.key === "Escape") onEscape();
            if (e.key === "Enter") {
              const text = (e.target as HTMLInputElement).value.trim();
              if (text) onTyped(text);
            }
          }}
        />
      )}
      {stage.s === "note" && (
        <div className="hud-row">
          <span className={`hud-text ${stage.tone}`}>
            {stage.text.startsWith("✓") && <ShieldCheck size={13} className="hud-icon" />}
            {stage.text}
          </span>
        </div>
      )}
    </>
  );
}
