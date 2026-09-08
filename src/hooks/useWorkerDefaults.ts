import { useEffect, useRef, useState } from "react";
import * as ipc from "../lib/ipc";
import type { Overview } from "../types";
import { DEFAULT_TIERS, type ModelTiers } from "../components/ModelSelect";

/**
 * The three composer pills' WINDOW defaults — mode, model, effort.
 *
 * They are preferences, not session state: seeded from config.toml once
 * the overview lands, and written back the moment one is picked. Shared
 * so the mother's expanded chat and the project chats cannot drift into
 * remembering different things.
 *
 * Applying a pick to a LIVE task is deliberately NOT here: that is one
 * task's directive, it dies with the task, and the two screens do
 * genuinely different work around it (the project window can emulate a
 * relaxed permission mode for a turn already in flight; the mother has
 * no such turn to rescue).
 */
export function useWorkerDefaults(overview: Overview | null) {
  const [mode, setMode] = useState<string | null>(null);
  // The tier table the model pill offers, from config [models]. Read here
  // so both windows offer the same four models.
  const [tiers, setTiers] = useState<ModelTiers>(DEFAULT_TIERS);
  useEffect(() => {
    ipc
      .configRead()
      .then((snap) => {
        const v = snap.values as { models?: Partial<ModelTiers>; model?: string };
        const m = v.models ?? {};
        setTiers({
          light: m.light || DEFAULT_TIERS.light,
          standard: m.standard || v.model || DEFAULT_TIERS.standard,
          heavy: m.heavy || DEFAULT_TIERS.heavy,
          max: m.max || DEFAULT_TIERS.max,
        });
      })
      .catch(() => {});
  }, []);
  const [model, setModel] = useState("");
  const [effort, setEffort] = useState("");

  // Once, on the first overview. A fallback on every read would have
  // resurrected the stored value the moment someone deliberately picked
  // "auto" or "padrão" — both of which are the empty string.
  const seeded = useRef(false);
  useEffect(() => {
    if (seeded.current || !overview) return;
    seeded.current = true;
    setModel(overview.default_model ?? "");
    setEffort(overview.default_effort ?? "");
  }, [overview]);

  const remember = (key: string, value: string) => {
    ipc.configWrite({ [key]: value }).catch(() => {});
  };

  return {
    /** The raw pick, or null when nobody touched the pill. Dispatches pass
     *  this: an empty config mode means "the CLI's own default", and
     *  sending a resolved "manual" instead would quietly override it. */
    mode,
    /** What the pill DISPLAYS: the pick, else config, else manual. */
    modeInForce: mode ?? overview?.default_mode ?? "manual",
    model,
    effort,
    tiers,
    setModeDefault(flag: string) {
      setMode(flag);
      remember("worker_mode", flag);
    },
    setModelDefault(value: string) {
      setModel(value);
      remember("worker_model", value);
    },
    setEffortDefault(value: string) {
      setEffort(value);
      remember("worker_effort", value);
    },
  };
}
