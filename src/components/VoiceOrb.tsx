import { t } from "../lib/i18n";

export type OrbMode = "idle" | "listening" | "speaking" | "busy";

/**
 * Tiny corner waveform that follows what the voice loop is doing:
 * listening (mic hot), speaking (TTS running) or thinking (busy). Pure
 * CSS animation keyed by state — cheap on purpose, no audio analysis.
 */
export default function VoiceOrb({ mode }: { mode: OrbMode }) {
  return (
    <span className={`orb ${mode}`} title={t(ORB_TITLE[mode])}>
      {[0, 1, 2, 3, 4].map((i) => (
        <span key={i} className="orb-bar" style={{ animationDelay: `${i * 0.12}s` }} />
      ))}
    </span>
  );
}

const ORB_TITLE = {
  idle: "orb_idle",
  listening: "orb_listening",
  speaking: "orb_speaking",
  busy: "orb_busy",
} as const;
