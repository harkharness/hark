import { Minus, Square, X } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { t } from "../lib/i18n";

/** The strip a desktop without an overlay title bar gets: a drag region
 *  across the top with the window's own three buttons at the right. The
 *  Mac paints its traffic lights over the page instead, so it never
 *  renders this (App decides by data-os). */
export default function WindowControls() {
  const win = () => getCurrentWindow();
  return (
    <div className="win-bar" data-tauri-drag-region>
      <button className="win-btn" title={t("win_min")} aria-label={t("win_min")} onClick={() => void win().minimize()}>
        <Minus size={13} />
      </button>
      <button className="win-btn" title={t("win_max")} aria-label={t("win_max")} onClick={() => void win().toggleMaximize()}>
        <Square size={10} />
      </button>
      <button className="win-btn close" title={t("win_close")} aria-label={t("win_close")} onClick={() => void win().close()}>
        <X size={13} />
      </button>
    </div>
  );
}
