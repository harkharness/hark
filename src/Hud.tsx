import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import * as ipc from "./lib/ipc";
import { setLang, setSpeechLang } from "./lib/i18n";
import { useVoiceRound } from "./hooks/useVoiceRound";
import VoiceRound from "./components/VoiceRound";
import type { HarkEvent } from "./types";

/**
 * The floating voice HUD: one global ear over every window, used when the
 * main window is not the one in front (phase 10.6: in front, the composer
 * runs the same round in place). The round itself lives in
 * `useVoiceRound`; this window is its face and its hide.
 */
export default function Hud() {
  const round = useVoiceRound({
    owner: "hud",
    autoStart: true,
    onHide: () => {
      ipc.hudHide().catch(() => {});
    },
  });
  const start = round.start;

  // Every hotkey press re-arms the HUD; the first mount starts by itself.
  useEffect(() => {
    // UI language for this window (stage changes repaint with it).
    ipc
      .configRead()
      .then((s) => {
        setLang(s.values.ui_language);
        setSpeechLang(s.values.language);
      })
      .catch(() => {});
    const un = listen<HarkEvent>("hark", (e) => {
      if (e.payload.kind === "hud_listen") start();
    });
    return () => {
      un.then((f) => f());
    };
  }, [start]);

  return (
    <div className="hud">
      <VoiceRound stage={round.stage} onPick={round.pick} onTyped={round.typed} onEscape={round.escape} />
    </div>
  );
}
