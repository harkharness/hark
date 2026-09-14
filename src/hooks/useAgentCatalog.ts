import { useCallback, useEffect, useState } from "react";
import * as ipc from "../lib/ipc";
import { keepIfSame } from "../lib/settle";
import { toCatalog, type Catalog } from "../lib/support";

/**
 * The agent catalog as the window sees it: every registry entry's name
 * and capability sheet, plus which one opens new chats. Fetched once and
 * refreshed on a slow poll (`which` on a handful of binaries), settled
 * through `keepIfSame` so the tree does not re-render for an identical
 * answer — the poll rule from docs/FRONTEND.md.
 */
export function useAgentCatalog(): { catalog: Catalog; selected?: string; refresh: () => void } {
  const [plugins, setPlugins] = useState<ipc.AgentPlugin[]>([]);
  const refresh = useCallback(() => {
    ipc
      .agentPlugins()
      .then((list) => setPlugins(keepIfSame(list)))
      .catch(() => {});
  }, []);
  useEffect(() => {
    refresh();
    const id = setInterval(refresh, 30_000);
    return () => clearInterval(id);
  }, [refresh]);
  const [catalog, setCatalog] = useState<Catalog>({});
  useEffect(() => setCatalog(toCatalog(plugins)), [plugins]);
  return { catalog, selected: plugins.find((p) => p.selected)?.id, refresh };
}
