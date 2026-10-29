import { useEffect, useRef, useState } from "react";
import { ArrowLeft, ArrowRight, ExternalLink, RotateCw } from "lucide-react";
import * as ipc from "../lib/ipc";
import { t } from "../lib/i18n";

const LAST_URL_KEY = "hark.browser.url";
const DEFAULT_URL = "https://harkharness.web.app";

/**
 * The browser panel of the rail: a real page beside the chat. The page is
 * a child webview of the window (its own origin, cookies and history; no
 * Tauri command reaches it); this component only owns the bar and keeps
 * the webview glued to the body's rectangle as the layout moves.
 */
export default function BrowserPanel() {
  const bodyRef = useRef<HTMLDivElement>(null);
  const [url, setUrl] = useState(() => {
    try {
      return window.localStorage.getItem(LAST_URL_KEY) || DEFAULT_URL;
    } catch {
      return DEFAULT_URL;
    }
  });
  const [draft, setDraft] = useState(url);
  const [error, setError] = useState<string | null>(null);
  const opened = useRef(false);

  const rect = () => {
    const r = bodyRef.current?.getBoundingClientRect();
    return r ? { x: r.left, y: r.top, w: r.width, h: r.height } : null;
  };

  // Open on mount, follow the body as it moves, close on unmount: the
  // webview lives exactly as long as this panel.
  useEffect(() => {
    const r = rect();
    if (!r) return;
    ipc
      .browserOpen(url, r.x, r.y, r.w, r.h)
      .then(() => {
        opened.current = true;
      })
      .catch((err) => setError(String(err)));
    let last = "";
    const sync = () => {
      const now = rect();
      if (!now || !opened.current) return;
      const key = `${now.x}:${now.y}:${now.w}:${now.h}`;
      if (key === last) return;
      last = key;
      void ipc.browserBounds(now.x, now.y, now.w, now.h).catch(() => {});
    };
    const ro = new ResizeObserver(sync);
    if (bodyRef.current) ro.observe(bodyRef.current);
    ro.observe(document.body);
    // Position changes without a size change (a sidebar folding) fire no
    // observer: a cheap tick covers them.
    const tick = window.setInterval(sync, 400);
    const bar = window.setInterval(() => {
      void ipc
        .browserUrlNow()
        .then((now) => {
          if (now && now !== url) {
            setUrl(now);
            setDraft((d) => (document.activeElement?.classList.contains("browser-url") ? d : now));
            try {
              window.localStorage.setItem(LAST_URL_KEY, now);
            } catch {
              /* no storage */
            }
          }
        })
        .catch(() => {});
    }, 1000);
    return () => {
      ro.disconnect();
      window.clearInterval(tick);
      window.clearInterval(bar);
      opened.current = false;
      void ipc.browserClose().catch(() => {});
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  function go(target: string) {
    setError(null);
    ipc
      .browserNavigate(target)
      .then(() => setUrl(target))
      .catch((err) => setError(String(err)));
  }

  return (
    <div className="browser-panel">
      <div className="browser-bar">
        <button className="frame-ctl" title={t("browser_back")} onClick={() => void ipc.browserHistory("back").catch(() => {})}>
          <ArrowLeft size={13} />
        </button>
        <button className="frame-ctl" title={t("browser_forward")} onClick={() => void ipc.browserHistory("forward").catch(() => {})}>
          <ArrowRight size={13} />
        </button>
        <button className="frame-ctl" title={t("browser_reload")} onClick={() => void ipc.browserHistory("reload").catch(() => {})}>
          <RotateCw size={13} />
        </button>
        <input
          className="browser-url"
          value={draft}
          spellCheck={false}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") go(draft);
            if (e.key === "Escape") setDraft(url);
          }}
          onFocus={(e) => e.currentTarget.select()}
        />
        <button className="frame-ctl" title={t("browser_external")} onClick={() => void ipc.openExternal(url).catch(() => {})}>
          <ExternalLink size={13} />
        </button>
      </div>
      {error && <div className="browser-error">{error}</div>}
      <div className="browser-body" ref={bodyRef}>
        <span className="dim">{t("browser_hint")}</span>
      </div>
    </div>
  );
}
