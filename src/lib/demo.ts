import type { EventCallback, UnlistenFn } from "@tauri-apps/api/event";
import type {
  AgentPlugin,
  ConfigSnapshot,
  EcoOut,
  SavingsOut,
  SetupStatus,
  VerdictOut,
} from "./ipc";
import type {
  ArtifactNode,
  ArtifactPage,
  ArtifactSpace,
  ArtifactTree,
  BoardTask,
  FileChange,
  JournalTurn,
  Overview,
  Project,
  RepoState,
  SessionHit,
  SpendAgg,
  StatusLine,
  TranscriptEntry,
  UsageReport,
  VoicePlan,
  SheetView,
  DashView,
  BoardView,
} from "../types";

/**
 * Demo mode: the real window over invented data and no backend. Opened
 * with `?demo=<scene>` in a plain browser (Vite dev server), every Tauri
 * command is answered from the fixtures below and every event comes from
 * the scene's script. It exists for one reason: the screenshots in the
 * README and on the site are the real UI, not a drawing of it, and they
 * never carry a real project, session or name. Zero backend, zero tokens.
 *
 *   npm run dev → http://localhost:1420/?demo=home|task|board|docs|costs
 */

const scene = (): string | null => {
  try {
    return new URLSearchParams(window.location.search).get("demo");
  } catch {
    return null;
  }
};

export const isDemo = (): boolean => scene() != null;

// ---------- the invented world ----------

const HOME = "/Users/ana/code";
const PROJECTS: Project[] = [
  { name: "webhook-api", path: `${HOME}/webhook-api` },
  { name: "mobile-app", path: `${HOME}/mobile-app` },
  { name: "infra", path: `${HOME}/infra` },
];
const P = Object.fromEntries(PROJECTS.map((p) => [p.name, p]));

const ago = (mins: number) => new Date(Date.now() - mins * 60_000).toISOString();

const BOARD: BoardTask[] = [
  {
    title: "Billing endpoint v2",
    status: "doing",
    subtasks: [
      { text: "Map every caller of /billing/v1", done: true },
      { text: "Route v2 behind the feature flag", done: true },
      { text: "Old route answers 410 with a pointer", done: true },
      { text: "Patch the two callers in the monorepo", done: false },
      { text: "Remove the flag after a week", done: false },
    ],
    updated_at: ago(3),
    session_ids: ["s-billing"],
    pinned: true,
    workspace: P["webhook-api"].path,
  },
  { title: "Alerts cleanup and rerouting", status: "doing", updated_at: ago(41), session_ids: ["s-alerts"], pinned: false, workspace: P.infra.path },
  { title: "Retry queue fix, awaiting review", status: "waiting", updated_at: ago(190), session_ids: ["s-retry"], pinned: false, workspace: P["mobile-app"].path, note: "PR #212 open" },
  { title: "Rotate the webhook signing keys", status: "backlog", updated_at: ago(1500), session_ids: [], pinned: false, workspace: P["webhook-api"].path },
  { title: "Upgrade the queue library", status: "backlog", updated_at: ago(2900), session_ids: [], pinned: false, workspace: P["mobile-app"].path },
  { title: "Deploy pipeline hardening", status: "done", updated_at: ago(4000), session_ids: ["s-deploy", "s-deploy-2"], pinned: false, workspace: P.infra.path },
  { title: "Incident summary draft", status: "done", updated_at: ago(6100), session_ids: ["s-incident"], pinned: false, workspace: P.infra.path },
];

const SESSIONS: Record<string, SessionHit[]> = {
  "webhook-api": [
    { session_id: "s-billing", title: "Billing endpoint v2", cwd: P["webhook-api"].path, last_ts: ago(3), last_prompt: "patch the two callers too" },
    { session_id: "s-keys", title: "Webhook signing keys: rotation plan", cwd: P["webhook-api"].path, last_ts: ago(1500), last_prompt: "draft the rotation runbook" },
    { session_id: "s-410", title: "Why the old route must answer 410", cwd: P["webhook-api"].path, last_ts: ago(2600), last_prompt: "compare 301 and 410 for API clients" },
    { session_id: "s-idem", title: "Idempotency keys on retries", cwd: P["webhook-api"].path, last_ts: ago(7200), last_prompt: "add the idempotency header to the client" },
  ],
  "mobile-app": [
    { session_id: "s-retry", title: "Retry queue fix, awaiting review", cwd: P["mobile-app"].path, last_ts: ago(190), last_prompt: "address the review comments" },
    { session_id: "s-offline", title: "Offline mode for the inbox", cwd: P["mobile-app"].path, last_ts: ago(5000), last_prompt: "cache the last 50 messages" },
  ],
  infra: [
    { session_id: "s-alerts", title: "Alerts cleanup and rerouting", cwd: P.infra.path, last_ts: ago(41), last_prompt: "route the paging alerts to the new channel" },
    { session_id: "s-deploy", title: "Deploy pipeline hardening", cwd: P.infra.path, last_ts: ago(4000), last_prompt: "require two approvals on prod" },
    { session_id: "s-incident", title: "Incident summary draft", cwd: P.infra.path, last_ts: ago(6100), last_prompt: "summarize the 14:02 deploy incident" },
  ],
};
const ALL_SESSIONS = Object.values(SESSIONS).flat();
const TITLE_OF = (id: string) => ALL_SESSIONS.find((s) => s.session_id === id)?.title ?? null;

const TRANSCRIPTS: Record<string, TranscriptEntry[]> = {
  "s-billing": [
    { ts: ago(26), role: "user", text: "migrate the billing endpoint to v2, keep the old route answering 410", is_error: false },
    { ts: ago(25), role: "tool_use", tool: "Bash", text: JSON.stringify({ command: "grep -rn \"billing/v1\" src apps | cut -d: -f1 | sort -u", description: "Find every caller of the v1 route" }), is_error: false },
    { ts: ago(25), role: "tool_result", text: "src/routes/billing.ts\napps/dashboard/src/api/billing.ts\napps/worker/src/jobs/invoice.ts", is_error: false },
    { ts: ago(24), role: "tool_use", tool: "Edit", text: JSON.stringify({ file_path: "src/routes/billing.ts", old_string: "router.post(\"/billing/v1/charge\", charge);", new_string: "router.post(\"/billing/v2/charge\", chargeV2);\nrouter.post(\"/billing/v1/charge\", gone410(\"/billing/v2/charge\"));" }), is_error: false },
    { ts: ago(24), role: "tool_result", text: "The file src/routes/billing.ts has been updated.", is_error: false },
    { ts: ago(23), role: "tool_use", tool: "Bash", text: JSON.stringify({ command: "npm test -- billing", description: "Run the billing tests" }), is_error: false },
    { ts: ago(22), role: "tool_result", text: "PASS src/routes/billing.test.ts\n  ✓ v2 charges with the new payload (18 ms)\n  ✓ v1 answers 410 with Link to v2 (4 ms)\n\nTests: 2 passed, 2 total", is_error: false },
    { ts: ago(22), role: "assistant", text: "Route migrated and the old path now returns **410** with a `Link` header pointing at v2. Tests green.\n\nTwo callers in the monorepo still hit v1:\n\n- `apps/dashboard/src/api/billing.ts`\n- `apps/worker/src/jobs/invoice.ts`\n\nWant me to patch them too, or leave them to their own teams?", is_error: false },
    { ts: ago(4), role: "user", text: "patch the two callers too", is_error: false },
    { ts: ago(3), role: "tool_use", tool: "Edit", text: JSON.stringify({ file_path: "apps/dashboard/src/api/billing.ts", old_string: "/billing/v1/charge", new_string: "/billing/v2/charge" }), is_error: false },
    { ts: ago(3), role: "tool_result", text: "The file apps/dashboard/src/api/billing.ts has been updated.", is_error: false },
    { ts: ago(3), role: "tool_use", tool: "Edit", text: JSON.stringify({ file_path: "apps/worker/src/jobs/invoice.ts", old_string: "/billing/v1/charge", new_string: "/billing/v2/charge" }), is_error: false },
    { ts: ago(3), role: "tool_result", text: "The file apps/worker/src/jobs/invoice.ts has been updated.", is_error: false },
    { ts: ago(3), role: "assistant", text: "Both callers now use v2; the worker's retry path keeps its idempotency key. Ready to push the branch `billing-v2` so the dashboard team can review.", is_error: false },
  ],
  "s-alerts": [
    { ts: ago(60), role: "user", text: "route the paging alerts to the new channel and drop the duplicates", is_error: false },
    { ts: ago(41), role: "assistant", text: "Routed 14 alerts; 3 were duplicates of the deploy alert and are folded into it. The on-call rotation file is untouched.", is_error: false },
  ],
};

/** Behind the bell: two unread, one read a moment ago. */
const NOTIFS: import("../types").Notification[] = [
  { id: "turn:chat:Rotate the webhook secret:1", kind: "turn_ended", target: { kind: "chat", title: "Rotate the webhook secret", session_id: "s-rotate" }, project: P["webhook-api"].path, text: "finished its turn · runbook drafted, two commands to confirm", at: ago(3) },
  { id: "page:webhook-api/incident-summary:1", kind: "artifact_ready", target: { kind: "page", space: "webhook-api", id: "incident-summary" }, project: "webhook-api", text: "Incident summary · deploy 14:02 ready for review", at: ago(45) },
  { id: "perm:chat:Billing endpoint v2:1", kind: "permission_asked", target: { kind: "chat", title: "Billing endpoint v2", session_id: "s-billing" }, project: P["webhook-api"].path, text: "asked for permission: Bash", at: ago(9), read_at: ago(2) },
];

/** The list of the day, as Início › Demandas shows it. */
const DEMANDS: import("../types").Demand[] = [
  { id: "task:Billing endpoint v2", source: "board", project: P["webhook-api"].path, title: "Billing endpoint v2", why: "stopped at a permission", urgency: "needs_you", link: null, at: ago(6) },
  { id: "page:webhook-api/incident-summary", source: "artifact", project: "webhook-api", title: "Incident summary · deploy 14:02", why: "ready for your review", urgency: "review", link: "hark://artifacts/webhook-api/incident-summary", at: ago(45) },
  { id: "task:Offline mode for the queue", source: "board", project: P["mobile-app"].path, title: "Offline mode for the queue", why: "stalled for 4 days", urgency: "stalled", link: null, at: ago(4 * 24 * 60) },
  { id: "req:r1", source: "request", project: null, title: "answer the accountant", why: "you asked yesterday, 18:10", urgency: "reminder", link: null, at: ago(16 * 60) },
];

const JOURNAL: JournalTurn[] = [
  // The days before today: rows in the Início sidebar, read back on click.
  { ts: ago(3 * 24 * 60 + 90), question: "what's blocked across the projects?", fala: "Two things: the retry queue waits on a review, and the offline mode has no owner yet.", body: "" },
  { ts: ago(24 * 60 + 150), question: "sum up yesterday", fala: "Billing v2 shipped to staging; the alerts cleanup ran its first batch; nothing failed.", body: "" },
  { ts: ago(24 * 60 + 40), question: "what did the dashboard team answer?", fala: "They took the v2 route on their side; the worker's patch is theirs to review.", body: "" },
  { ts: ago(120), question: "how much did I spend today?", fala: "$0.42 today, across two chats. Nothing failed.", body: "" },
  { ts: ago(118), question: "what's running?", fala: "Two workers: Billing endpoint v2 in webhook-api, Alerts cleanup in infra. Both on their last steps.", body: "" },
  { ts: ago(45), question: "read the alerts thread and draft the incident summary", fala: "Draft ready. Three alerts correlate with the deploy at 14:02; the summary is on the board as a page.", body: "" },
];

const SPACES: ArtifactSpace[] = [
  { name: "webhook-api", project: P["webhook-api"].path, pages: 6 },
  { name: "infra", project: P.infra.path, pages: 2 },
  { name: "mobile-app", project: P["mobile-app"].path, pages: 0 },
  { name: "pessoal", project: null, pages: 1 },
];
const node = (id: string, title: string, path: string, kind: ArtifactNode["kind"], parent: string | null, mins: number, origin?: ArtifactNode["origin"]): ArtifactNode => ({
  id, title, path, kind, parent, updated_at: ago(mins), origin: origin ?? null,
});
const TREES: Record<string, ArtifactTree> = {
  "webhook-api": {
    nodes: [
      node("overview", "Overview", "overview.md", "doc", null, 3000),
      node("decisions", "Decisions", "decisions.md", "doc", null, 2900),
      node("adr-003-spend-ceiling", "ADR 003: one ceiling per turn", "decisions/adr-003-spend-ceiling.md", "doc", "decisions", 2000, { session_id: "s-idem", task: "Idempotency keys on retries" }),
      node("adr-004-one-window", "ADR 004: one window, every project", "decisions/adr-004-one-window.md", "doc", "decisions", 12, { session_id: "s-billing", task: "Billing endpoint v2" }),
      node("billing-v2-rollout", "Billing v2 · rollout deck", "billing-v2-rollout.html", "slides", null, 70, { session_id: "s-billing", task: "Billing endpoint v2" }),
      node("cells-design", "Cells · how a region is wired", "cells-design.html", "design", null, 240, { session_id: "s-keys", task: "Webhook signing keys: rotation plan" }),
      node("infra-costs", "Infra costs by month", "infra-costs.sheet.json", "sheet", null, 25, { session_id: "s-billing", task: "Billing endpoint v2" }),
      node("usage", "Dashboard: webhook usage", "usage.dash.json", "data", null, 40, { session_id: "s-billing", task: "Billing endpoint v2" }),
      node("release-plan", "Release 0.11 plan", "release-plan.board.json", "board", null, 90),
    ],
    order: { "": ["overview", "decisions", "billing-v2-rollout", "cells-design", "infra-costs", "usage", "release-plan"], decisions: ["adr-003-spend-ceiling", "adr-004-one-window"] },
  },
  infra: {
    nodes: [
      node("runbooks", "Runbooks", "runbooks.md", "doc", null, 4000),
      node("incident-14-02", "Incident: the 14:02 deploy", "runbooks/incident-14-02.md", "doc", "runbooks", 6000, { session_id: "s-incident", task: "Incident summary draft" }),
    ],
    order: { "": ["runbooks"], runbooks: ["incident-14-02"] },
  },
  "mobile-app": { nodes: [], order: {} },
  pessoal: { nodes: [node("reading-list", "Reading list", "reading-list.md", "doc", null, 9000)], order: { "": ["reading-list"] } },
};
const PAGE_TEXT: Record<string, string> = {
  "adr-004-one-window": `# ADR 004: one window, every project

Status: **accepted** · 2026-10-27 · supersedes ADR 002

## Context

Three windows per project did not survive a day of real use: the one you
needed was always behind the one you had. The board, the costs and the
documentation are views of *everything*, not of one project.

## Decision

One main window holds every project. The sidebar is the map; a chat opens in
place and keeps its own panels beside it (terminal, files, diff, browser). A
chat detaches into a window of its own only when the person asks for it.

## Consequences

- Panels belong to the chat they were opened in, and come back with it.
- The board and the artifacts are views: no composer, no scope, no panels.
- A voice command never switches what you are looking at; the chat it landed
  in pulses in the sidebar instead.

## Next steps

- [ ] Fold the project window into a detached chat (\`hark://chat/<session>\`)
- [x] Move the costs under settings
`,
  overview: `# webhook-api

The service every product calls to charge, refund and reconcile. Node 22,
Postgres, one queue. Owned by the payments team; paged through infra.

## Where to start

- [Decisions](hark://artifacts/webhook-api/decisions): the ADRs, newest first
- [Billing v2 · rollout deck](hark://artifacts/webhook-api/billing-v2-rollout)
`,
  decisions: `# Decisions\n\nArchitecture decision records of webhook-api, one page each.\n`,
  "adr-003-spend-ceiling": `# ADR 003: one ceiling per turn\n\nStatus: accepted. Every agent turn runs under a dollar ceiling held by the harness, not by the agent.\n`,
  runbooks: `# Runbooks\n\nWhat to do at 3 a.m., one page per failure mode.\n`,
  "incident-14-02": `# Incident: the 14:02 deploy\n\nThree alerts correlated with the deploy at 14:02. Rolled back in 11 minutes; no customer impact beyond retries.\n`,
  "reading-list": `# Reading list\n\n- The Mythical Man-Month, again\n- Designing Data-Intensive Applications, ch. 11\n`,
  "cells-design": `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="hark-kind" content="design"><meta name="hark-canvas" content="1600x1000"><title>Cells</title>
<style>body{margin:0;width:1600px;height:1000px;background:#fff;color:#1f2430;font:16px/1.4 -apple-system,"Segoe UI",sans-serif}.cell{position:absolute;width:300px;height:170px;border:2px solid #2f5fd1;border-radius:14px;padding:18px;box-sizing:border-box;background:#f4f7ff}.cell h3{margin:0 0 6px;font-size:18px}.cell small{color:#5b6475}.edge{position:absolute;top:120px;left:120px;width:1360px;height:90px;border:2px dashed #e0af68;border-radius:14px;padding:16px;box-sizing:border-box;background:#fff8ea}h1{position:absolute;left:120px;top:40px;margin:0;font-size:32px}</style></head>
<body><h1>Cells: how a region is wired</h1><div class="edge"><b>Edge</b> · DNS → load balancer → gateway. One hostname grammar: &lt;service&gt;.&lt;cell&gt;.example</div>
<div class="cell" style="left:120px;top:300px"><h3>us · global core</h3><small>shared control plane</small><p>api-east, api-west; the only writer.</p></div>
<div class="cell" style="left:470px;top:300px"><h3>ph · Philippines</h3><small>regional cell</small><p>Reads locally, writes through the core.</p></div>
<div class="cell" style="left:820px;top:300px"><h3>mx · Mexico</h3><small>regional cell</small><p>Same shape as ph.</p></div>
<div class="cell" style="left:1170px;top:300px"><h3>eu · Europe</h3><small>dedicated cell</small><p>Own writer, own keys.</p></div>
<div class="cell" style="left:120px;top:560px;width:1350px;height:300px;background:#f6f6f6;border-color:#9aa3b2"><h3>What every cell carries</h3><p>Gateway, queue, a Postgres with its own keys, the metrics agent. Nothing crosses a cell but the control plane's replication.</p></div>
</body></html>`,
  "billing-v2-rollout": `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>Billing v2 · rollout</title>
<style>:root{color-scheme:light dark;--ink:#1f2430;--paper:#fff;--accent:#2f5fd1}@media(prefers-color-scheme:dark){:root{--ink:#d6dae3;--paper:#12151c;--accent:#7aa2f7}}body{margin:0;font:18px/1.5 -apple-system,"Segoe UI",sans-serif;color:var(--ink);background:var(--paper)}section{min-height:100vh;padding:8vh 10vw;box-sizing:border-box;border-bottom:1px solid rgba(128,128,128,.25)}h1{font-size:44px;margin:0 0 16px;letter-spacing:-.02em}h2{font-size:32px;margin:0 0 12px}.kicker{color:var(--accent);font-size:14px;letter-spacing:.08em;text-transform:uppercase}</style></head>
<body><section><div class="kicker">Rollout</div><h1>Billing v2</h1><p>One week behind a flag, then the old route answers 410.</p></section><section><h2>Who calls v1 today</h2><p>Dashboard, worker, and nobody else.</p></section></body></html>`,
};
const findNode = (space: string, id: string) => TREES[space]?.nodes.find((n) => n.id === id);
const crumbs = (space: string, id: string): string[] => {
  const out: string[] = [];
  let cur = findNode(space, id);
  while (cur) {
    out.unshift(cur.title);
    cur = cur.parent ? findNode(space, cur.parent) : undefined;
  }
  return out;
};

const REPO: RepoState = { root: P["webhook-api"].path, branch: "billing-v2", upstream: "origin/billing-v2", ahead: 2, behind: 0, dirty: 3, added: 61, removed: 9 };
const CHANGES: FileChange[] = [
  { path: "src/routes/billing.ts", added: 24, removed: 6, untracked: false, binary: false },
  { path: "apps/dashboard/src/api/billing.ts", added: 2, removed: 2, untracked: false, binary: false },
  { path: "apps/worker/src/jobs/invoice.ts", added: 2, removed: 1, untracked: false, binary: false },
  { path: "docs/billing-v2.md", added: 33, removed: 0, untracked: true, binary: false },
];
const DIFF = `diff --git a/src/routes/billing.ts b/src/routes/billing.ts
--- a/src/routes/billing.ts
+++ b/src/routes/billing.ts
@@ -8,9 +8,27 @@ import { charge } from "../billing/charge";
+import { chargeV2 } from "../billing/charge-v2";
+import { gone410 } from "../http/gone";

 const router = Router();

-router.post("/billing/v1/charge", charge);
+router.post("/billing/v2/charge", chargeV2);
+// The old route stays for a week and says where to go.
+router.post("/billing/v1/charge", gone410("/billing/v2/charge"));
+
+/** 410 with a Link header: clients that follow it never see a 404. */
+export function gone410(next: string) {
+  return (_req: Request, res: Response) => {
+    res.set("Link", \`<\${next}>; rel="successor-version"\`);
+    res.status(410).json({ error: "gone", next });
+  };
+}
`;
const FILES = [
  "src/routes/billing.ts", "src/routes/index.ts", "src/billing/charge.ts", "src/billing/charge-v2.ts", "src/http/gone.ts",
  "src/queue/retry.ts", "src/queue/idempotency.ts", "apps/dashboard/src/api/billing.ts", "apps/worker/src/jobs/invoice.ts",
  "docs/billing-v2.md", "package.json", "README.md",
];
const FILE_TEXT: Record<string, string> = {
  "apps/dashboard/src/api/billing.ts": `import { api } from "./client";

export type ChargeInput = { customerId: string; amountCents: number; idempotencyKey: string };

/** Charges through the billing service; v2 since the rollout of 2026-10. */
export async function charge(input: ChargeInput) {
  return api.post("/billing/v2/charge", input, {
    headers: { "Idempotency-Key": input.idempotencyKey },
  });
}

export async function refund(chargeId: string, amountCents?: number) {
  return api.post(\`/billing/v2/charges/\${chargeId}/refund\`, { amountCents });
}
`,
  "src/routes/billing.ts": `import { Router, type Request, type Response } from "express";
import { charge } from "../billing/charge";
import { chargeV2 } from "../billing/charge-v2";

const router = Router();

router.post("/billing/v2/charge", chargeV2);
// The old route stays for a week and says where to go.
router.post("/billing/v1/charge", gone410("/billing/v2/charge"));

/** 410 with a Link header: clients that follow it never see a 404. */
export function gone410(next: string) {
  return (_req: Request, res: Response) => {
    res.set("Link", \`<\${next}>; rel="successor-version"\`);
    res.status(410).json({ error: "gone", next });
  };
}

export default router;
`,
};

const SPEND = (group: string): SpendAgg[] => {
  const row = (key: string, cost: number, turns: number, input = 0, output = 0, cr = 0, cc = 0, errors = 0): SpendAgg => ({
    key, cost_usd: cost, priced_turns: turns, turns, input, output, cache_read: cr, cache_created: cc, errors,
  });
  switch (group) {
    case "kind": return [row("worker", 3.84, 31, 41_200, 18_900, 612_000, 88_000, 1), row("gate", 0.21, 19, 9_100, 1_100), row("ask", 0.09, 12, 6_300, 2_200), row("title", 0.02, 4, 900, 60)];
    case "model": return [row("claude-opus-5-5", 3.12, 18, 30_000, 14_000, 500_000, 70_000), row("claude-sonnet-5-5", 0.92, 29, 20_000, 7_000, 110_000, 18_000, 1), row("claude-haiku-5-5", 0.12, 19, 7_500, 1_260)];
    case "workspace": return [row(P["webhook-api"].path, 2.41, 29, 28_000, 12_000, 400_000, 60_000), row(P.infra.path, 1.33, 22, 18_000, 7_000, 180_000, 20_000, 1), row(P["mobile-app"].path, 0.42, 15, 11_500, 3_160, 32_000, 6_000)];
    case "day": return [row("2026-10-28", 0.74, 9, 8_000, 3_000, 90_000, 12_000), row("2026-10-29", 1.12, 14, 12_000, 5_000, 160_000, 20_000), row("2026-10-30", 1.48, 21, 20_000, 8_000, 240_000, 30_000, 1), row("2026-10-31", 0.82, 22, 17_500, 6_160, 122_000, 24_000)];
    case "session":
    case "task": return [row("Billing endpoint v2", 1.96, 14, 22_000, 9_000, 330_000, 48_000), row("Alerts cleanup and rerouting", 1.02, 11, 11_000, 5_000, 140_000, 18_000, 1), row("Retry queue fix, awaiting review", 0.61, 9, 9_000, 3_000, 60_000, 9_000), row("Incident summary draft", 0.38, 6, 6_000, 2_000, 40_000, 6_000)];
    default: return [row("hark", 4.16, 66, 57_500, 22_160, 612_000, 88_000, 1)];
  }
};
const LIMITS: StatusLine = {
  context_used: 0.18, context_tokens: 36_000, context_window: 200_000, model: "claude-opus-5-5", session_id: "s-billing",
  limits: [
    { key: "5h", used: 0.38, resets_at: ago(-130) },
    { key: "7d", used: 0.61, resets_at: ago(-4300) },
    { key: "7d_opus", used: 0.44, resets_at: ago(-4300) },
  ],
};
const USAGE: UsageReport = {
  session: {
    models: [{ model: "claude-opus-5-5", input: 22_000, output: 9_000, cache_read: 330_000, cache_created: 48_000, cost_usd: 1.96, turns: 14 }],
    turns: 14, cost_usd: 1.96, duration_ms: 48 * 60_000, cache_hit: 0.87,
    context: { total: 36_000, window: 200_000, pct: 0.18 },
  },
  day: { total_usd: 0.42, week_usd: 4.16, kinds: SPEND("kind"), top: SPEND("task") },
  machine: { models: SPEND("model").map((r) => ({ model: r.key, input: r.input, output: r.output, cache_read: r.cache_read, cache_created: r.cache_created, cost_usd: r.cost_usd, turns: r.turns })), turns: 66, workspaces: SPEND("workspace") },
  limits: LIMITS,
};

const OVERVIEW: Overview = {
  contexts: ["webhook-api", "infra"],
  active: "webhook-api",
  workers: [
    { task_id: "t-billing", status: "running", workspace: P["webhook-api"].path, summary: "Both callers now use v2; ready to push the branch billing-v2.", session_id: "s-billing" },
    { task_id: "t-alerts", status: "running", workspace: P.infra.path, summary: "Routed 14 alerts; 3 duplicates folded.", session_id: "s-alerts" },
  ],
  board: BOARD,
  projects: PROJECTS,
  theme: "hark",
  default_mode: "manual",
  default_model: "",
  default_effort: "",
  ui_language: "en",
  language: "en",
  assistant_name: "Hark",
  user_name: "Ana",
  in_flight: [],
};

const CONFIG: ConfigSnapshot = {
  values: {
    claude_bin: "claude", model: "", projects_dir: HOME, language: "en", ui_language: "en", voice: "Samantha",
    whisper_model: "small", stt: "whisper", theme: "hark", prompt_budget_chars: 12_000, hotkey: "cmd+shift+space",
    worker_budget_usd: 5, worker_max_turns: 60, worker_mode: "manual", worker_model: "", worker_effort: "",
    assistant_name: "Hark", auto_update: true, models: { light: "haiku", standard: "sonnet", heavy: "opus", max: "opus" },
  },
  path: "/Users/ana/.hark/config.toml",
  claude_bin_resolved: "/Users/ana/.local/bin/claude",
  whisper_model_resolved: "/Users/ana/.hark/models/ggml-small.bin",
  data_dir: "/Users/ana/.hark",
};

const CAPS = { resume: true, permissions: true, structured_output: true, cost_reporting: true, history: true, live_list: true, slash_commands: true, memory_file: "CLAUDE.md", shell_tools: ["Bash"], fork: true, directive_mode: true, directive_model: true, directive_effort: true };
const AGENTS: AgentPlugin[] = [
  { id: "claude", name: "Claude Code", plugin: "claude", cmd: "claude", vendor: "Anthropic", status: "available", detected: true, enabled: true, detail: "The claude CLI with your full settings, MCP servers and memory file.", install: "npm install -g @anthropic-ai/claude-code", selected: true, memory_file: "CLAUDE.md", login_hint: "claude", args: [], env_keys: [], models: { light: "haiku", standard: "sonnet", heavy: "opus", max: "opus" }, version: { installed: "2.1.4", current: "2.1.4", freshness: { state: "current" }, update: null, checked_at: ago(30) }, capabilities: CAPS },
  { id: "gemini", name: "Gemini CLI", plugin: "acp", cmd: "gemini", vendor: "Google", status: "available", detected: true, enabled: true, detail: "Gemini CLI over the Agent Client Protocol.", install: "npm install -g @google/gemini-cli", selected: false, memory_file: "GEMINI.md", login_hint: "gemini", args: ["--acp"], env_keys: [], models: { light: "gemini-2.5-flash-lite", standard: "gemini-2.5-flash", heavy: "gemini-2.5-pro", max: "gemini-2.5-pro" }, version: { installed: "0.46.0", current: "0.59.0", freshness: { state: "behind", installed: "0.46.0", current: "0.59.0" }, update: "npm install -g @google/gemini-cli@0.59.0", checked_at: ago(30) }, capabilities: { ...CAPS, cost_reporting: false, history: false, fork: false, memory_file: "GEMINI.md" } },
  { id: "codex", name: "Codex", plugin: "acp", cmd: "codex-acp", vendor: "OpenAI", status: "available", detected: false, enabled: true, detail: "Codex over the Agent Client Protocol.", install: "npm install -g @agentclientprotocol/codex-acp", selected: false, memory_file: "AGENTS.md", login_hint: "codex login", args: [], env_keys: [], models: {}, version: { installed: null, current: "0.9.2", freshness: { state: "unknown" }, update: null, checked_at: ago(30) }, capabilities: null },
];

const SETUP: SetupStatus = { onboarded: true, config_exists: true, whisper_ok: true, whisper_path: CONFIG.whisper_model_resolved, claude_bin: "claude", claude_ok: true, agent_ok: true, projects_dir_ok: true, models: [], stt_reco_reason: null, language: "en", assistant_name: "Hark", hotkey: "cmd+shift+space" };
const SAVINGS: SavingsOut = { avoided_gate_usd: 2.31, avoided_local_usd: 1.08, avoided_cache_usd: 11.4, total_usd: 14.79, methodology: ["gate: dispatches the evaluator refused, priced at a worker turn", "local: questions answered from the index, priced at a lean ask", "cache: cache-read tokens priced at the uncached input rate"] };
const ECO: EcoOut = { status: { rtk: true, ponytail: false, caveman: false, tokensave: false }, envs: [["RTK_ENABLED", "1"]], fingerprint: "rtk" };

// ---------- the event bus ----------

type Handler = (e: { event: string; id: number; payload: unknown }) => void;
const handlers = new Map<string, Set<Handler>>();
let seq = 0;

export function demoListen<T>(name: string, cb: EventCallback<T>): Promise<UnlistenFn> {
  const set = handlers.get(name) ?? new Set<Handler>();
  set.add(cb as unknown as Handler);
  handlers.set(name, set);
  return Promise.resolve(() => {
    set.delete(cb as unknown as Handler);
  });
}

/** Deliver an event as the Rust shell would. Exposed on `window.harkDemo`. */
export function demoEmit(name: string, payload: unknown): void {
  for (const h of handlers.get(name) ?? []) h({ event: name, id: ++seq, payload });
}

const sleep = (ms: number) => new Promise<void>((r) => window.setTimeout(r, ms));
const never = <T,>() => new Promise<T>(() => {});

// ---------- the commands ----------

type Args = Record<string, unknown> | undefined;
const C: Record<string, (a: Args) => unknown> = {
  overview: () => OVERVIEW,
  config_read: () => CONFIG,
  config_raw_read: () => ({ text: `# Hark\nlanguage = "en"\nui_language = "en"\nassistant_name = "Hark"\nhotkey = "cmd+shift+space"\nworker_budget_usd = 5\n`, path: CONFIG.path }),
  setup_status: () => SETUP,
  hark_chat_status: () => ({ alive: true, session_id: "s-hark" }),
  // `?demo=inicio`: a day with no chat yet, so Hoje opens with the brief.
  journal_recent: () => (scene() === "inicio" ? JOURNAL.filter((t) => new Date(t.ts).toDateString() !== new Date().toDateString()) : JOURNAL),
  demands: () => DEMANDS,
  request_add: () => null,
  request_done: () => undefined,
  attention_command: () => null,
  cli_folders: () => [
    { path: "/Users/ana/code/tulipafin", sessions: 7, last_ts: ago(26 * 60) },
    { path: "/Users/ana/code/scratch", sessions: 2, last_ts: ago(9 * 24 * 60) },
  ],
  notifications: () => ({ rows: NOTIFS, groups: [] }),
  notifications_history: () => [],
  notifications_read: () => undefined,
  notifications_read_chat: () => undefined,
  notifications_read_all: () => undefined,
  session_owners: () => [],
  project_sessions: (a) => SESSIONS[PROJECTS.find((p) => p.path === a?.path)?.name ?? ""] ?? [],
  session_candidates: (a) => ALL_SESSIONS.filter((s) => s.title.toLowerCase().includes(String(a?.query ?? "").toLowerCase())).slice(0, Number(a?.limit ?? 8)),
  find_session: (a) => ALL_SESSIONS.find((s) => s.title.toLowerCase().includes(String(a?.query ?? "").toLowerCase())) ?? null,
  read_transcript: (a) => {
    const id = String(a?.sessionId ?? "");
    const entries = TRANSCRIPTS[id] ?? (id === "s-hark" ? [] : [{ ts: ago(100), role: "user", text: TITLE_OF(id) ?? "…", is_error: false } as TranscriptEntry]);
    return { session_title: TITLE_OF(id), entries, offset: 4096 };
  },
  transcript_since: () => ({ entries: [], offset: 4096 }),
  session_stats: (a) => ({ title: TITLE_OF(String(a?.sessionId)), size_mb: 0.4, entries: 38, last_ts: ago(3) }),
  session_context_weight: () => ({ last_total_tokens: 36_000, cache_read: 30_000, cache_created: 4_000, input: 1_500, output: 500, context_window: 200_000, pct: 0.18 }),
  chat_from_session: (a) => ({ title: TITLE_OF(String(a?.sessionId)) ?? "chat", session_id: String(a?.sessionId) }),
  usage_report: () => USAGE,
  spend_summary: (a) => SPEND(String(a?.group ?? "kind")),
  spend_top_sessions: () => SPEND("session"),
  subscription_limits: () => LIMITS,
  statusline_bridge_status: () => ({ installed: true, age_secs: 40, payload_path: "/Users/ana/.hark/statusline.json" }),
  statusline_bridge_install: () => null,
  statusline_bridge_uninstall: () => undefined,
  savings_summary: () => SAVINGS,
  eco_status: () => ECO,
  agent_plugins: () => AGENTS,
  agent_registry_refresh: () => AGENTS,
  agent_plugin_select: () => undefined,
  agent_plugin_enable: () => undefined,
  agent_for_session: () => "claude",
  slash_commands: () => ["compact", "clear", "review"],
  repo_states: (a) => Object.fromEntries(((a?.paths as string[]) ?? []).map((p) => [p, p === P["webhook-api"].path ? REPO : { ...REPO, root: p, branch: "main", upstream: "origin/main", ahead: 0, dirty: 0, added: 0, removed: 0 }])),
  repo_changes: () => ({ root: P["webhook-api"].path, changes: CHANGES }),
  repo_diff: () => DIFF,
  project_files: (a) => FILES.filter((f) => f.toLowerCase().includes(String(a?.query ?? "").toLowerCase())).slice(0, Number(a?.limit ?? 10)),
  file_read: (a) => {
    const abs = String(a?.path ?? "");
    const rel = Object.keys(FILE_TEXT).find((k) => abs.endsWith(k));
    return { content: rel ? FILE_TEXT[rel] : `// ${abs}\n`, truncated: false };
  },
  file_save: () => undefined,
  artifacts_spaces: () => SPACES,
  artifacts_tree: (a) => TREES[String(a?.space)] ?? { nodes: [], order: {} },
  artifact_read: (a): ArtifactPage => {
    const space = String(a?.space);
    const id = String(a?.id);
    const n = findNode(space, id);
    if (!n) throw new Error(`${id}: not found`);
    return { node: n, text: PAGE_TEXT[id] ?? `# ${n.title}\n`, abs: `/Users/ana/.hark/artifacts/${space}/${n.path}`, breadcrumb: crumbs(space, id) };
  },
  artifact_cites: () => null,
  artifact_from_brief: () => TREES["webhook-api"].nodes[0],
  artifact_write: () => undefined,
  artifact_create: (a) => node(String(a?.title).toLowerCase().replace(/\W+/g, "-"), String(a?.title), `${String(a?.title).toLowerCase().replace(/\W+/g, "-")}.md`, "doc", (a?.parent as string) ?? null, 0),
  artifact_move: () => undefined,
  artifact_rename: () => undefined,
  artifact_set_kind: () => undefined,
  artifact_versions: () => [{ at: ago(60), bytes: 420 }, { at: ago(25), bytes: 468 }],
  artifact_accept: () => undefined,
  artifact_restore: () => undefined,
  sheet_view: (): SheetView => ({
    computed: {
      title: "Infra costs by month",
      tabs: [
        {
          name: "Monthly",
          columns: ["month", "infra", "db", "total"],
          cells: [
            [{ raw: "sep/26", text: "sep/26", formula: false }, { raw: "412", text: "412", formula: false, number: 412 }, { raw: "96", text: "96", formula: false, number: 96 }, { raw: "=B2+C2", text: "508", formula: true, number: 508 }],
            [{ raw: "oct/26", text: "oct/26", formula: false }, { raw: "438", text: "438", formula: false, number: 438 }, { raw: "101", text: "101", formula: false, number: 101 }, { raw: "=B3+C3", text: "539", formula: true, number: 539 }],
            [{ raw: "nov/26", text: "nov/26", formula: false }, { raw: "=B3*1.03", text: "451.14", formula: true, number: 451.14 }, { raw: "=ARRED(C3*1.03;0)", text: "104", formula: true, number: 104 }, { raw: "=B4+C4", text: "555.14", formula: true, number: 555.14 }],
            [{ raw: "average", text: "average", formula: false }, { raw: "=MÉDIA(B2:B4)", text: "433.71", formula: true, number: 433.71 }, { raw: "=MÉDIA(C2:C4)", text: "100.33", formula: true, number: 100.33 }, { raw: "=FOO(D2)", text: "#NOME?", formula: true, error: "#NOME?" }],
          ],
        },
        { name: "By service", columns: ["service", "usd"], cells: [[{ raw: "api", text: "api", formula: false }, { raw: "260", text: "260", formula: false, number: 260 }]] },
      ],
    },
    changed: [{ tab: 0, row: 2, col: 0 }, { tab: 0, row: 2, col: 1 }, { tab: 0, row: 2, col: 2 }, { tab: 0, row: 2, col: 3 }],
    versions: 2,
    raw: "{}",
  }),
  sheet_set_cell: () => undefined,
  sheet_edit: () => undefined,
  dash_view: (): DashView => ({
    title: "Dashboard: webhook usage",
    refresh: "hourly",
    sources: [{ id: "db", kind: "sqlite", path: "data/app.db" }, { id: "ev", kind: "csv", path: "data/events.csv" }, { id: "st", kind: "command", command: "./scripts/stats.sh" }],
    queries: [
      { id: "deliveries_day", source: "db", sql: "SELECT date(created_at) AS day, count(*) AS n\nFROM deliveries\nWHERE created_at >= date('now', '-28 days')\nGROUP BY day ORDER BY day;", columns: [], certified: true, changed: false, preview: { columns: ["day", "n"], rows: [["11/03", 3412], ["11/04", 3508], ["11/05", 3470]] } },
      { id: "active_7d", source: "st", sql: null, columns: [], certified: true, changed: false, preview: { columns: ["n", "before"], rows: [[1284, 1209]] } },
      { id: "retention_d7", source: "ev", sql: null, columns: ["month", "d7"], certified: false, changed: true, preview: { columns: ["month", "d7"], rows: [["sep", 38], ["oct", 41], ["nov", 52]] } },
      { id: "ttfd", source: "db", sql: "SELECT avg(seconds) AS s FROM first_delivery", columns: [], certified: false, changed: true, preview: null, error: "no such table: first_delivery" },
    ],
    panels: [
      { title: "active users · 7 days", query: "active_7d", chart: "number", certified: true, number: 1284, delta: 0.062 },
      { title: "deliveries per day", query: "deliveries_day", chart: "number", certified: true, number: 3910, delta: 0.11 },
      { title: "time to first delivery", query: "ttfd", chart: "number", certified: false, error: "no such table: first_delivery" },
      { title: "deliveries per day · 4 weeks", query: "deliveries_day", chart: "bar", certified: true, labels: ["13/10", "14", "15", "16", "17", "20", "21", "22", "23", "24", "27", "28", "29", "30", "31", "03/11", "04", "05", "06", "07"], values: [30, 34, 28, 40, 44, 41, 38, 52, 55, 49, 58, 62, 60, 71, 76, 73, 80, 85, 82, 88] },
      { title: "retention D7 · %", query: "retention_d7", chart: "line", certified: false, labels: ["sep", "oct", "nov"], values: [38, 41, 52] },
      { title: "last deliveries", query: "deliveries_day", chart: "table", certified: true, table: { columns: ["day", "n"], rows: [["11/03", 3412], ["11/04", 3508], ["11/05", 3470]] } },
    ],
    cache_at: ago(20),
    stale: false,
    changed: ["retention_d7", "ttfd"],
    versions: 3,
    raw: "{}",
  }),
  dash_certify: () => undefined,
  dash_edit: () => undefined,
  dash_story: () => "Active users grew 6% in a week and deliveries per day are at their highest since the quick-add beta; time to first delivery still misses the 30 s goal.",
  board_view: (): BoardView => ({
    board: {
      title: "Release 0.11 plan",
      groups: [
        { id: "g1", title: "Quick-add", x: 24, y: 24, w: 338, h: 290 },
        { id: "g2", title: "Reports", x: 390, y: 24, w: 338, h: 290 },
        { id: "g3", title: "Open questions", x: 756, y: 24, w: 338, h: 290 },
      ],
      notes: [
        { id: "n1", text: "One tap on the phone: amount, category, done.", x: 48, y: 60, color: "yellow", owner: "Ana", date: "fri", group: "g1" },
        { id: "n2", text: "Recognise “50 at the market” by voice (local STT).", x: 212, y: 60, color: "yellow", owner: "Ana", date: "fri", group: "g1" },
        { id: "n3", text: "Offline: local queue, sync later.", x: 48, y: 170, color: "yellow", owner: "agent", date: "18:31", group: "g1" },
        { id: "n4", text: "Spend by category per month, with a goal.", x: 414, y: 60, color: "blue", owner: "Bruno", date: "fri", group: "g2" },
        { id: "n5", text: "Export the month as CSV for the accountant.", x: 578, y: 60, color: "blue", owner: "Bruno", date: "fri", group: "g2" },
        { id: "n6", text: "Hark's dashboard reads the same SQLite.", x: 414, y: 170, color: "blue", owner: "agent", date: "18:31", group: "g2" },
        { id: "n7", text: "Quick-add in 0.11 or 0.12?", x: 780, y: 60, color: "pink", owner: "Ana", date: "fri", group: "g3" },
        { id: "n8", text: "Who validates pt-BR STT with accents?", x: 780, y: 170, color: "pink", owner: "Bruno", date: "fri", group: "g3" },
      ],
      timeline: [
        { date: "2026-11-12", title: "quick-add beta", owner: "Ana" },
        { date: "2026-11-19", title: "reports", owner: "Bruno" },
        { date: "2026-11-26", title: "voice STT", owner: "Ana" },
        { date: "2026-12-03", title: "release 0.11", owner: "team" },
      ],
      links: [
        { from: "n2", to: "n8", label: "needs" },
        { from: "n6", to: "n4" },
      ],
    },
    changed: ["n3", "n6"],
    versions: 2,
    raw: "{}",
  }),
  board_edit: () => undefined,
  board_selection: (a) => (Array.isArray(a?.ids) ? (a.ids as string[]).map((id) => `- note ${id}`).join("\n") + "\n" : ""),
  artifact_strip_pager: () => true,
  artifact_remove: () => [],
  export_file: (a) => `/Users/ana/.hark/exports/${String(a?.name)}`,
  reveal_path: () => undefined,
  term_open: (a) => {
    const id = String(a?.id);
    void (async () => {
      await sleep(250);
      const lines = [
        "\x1b[32mana@studio\x1b[0m \x1b[34m~/code/webhook-api\x1b[0m \x1b[33m(billing-v2)\x1b[0m $ npm test -- billing\r\n",
        "\r\n> webhook-api@2.4.0 test\r\n> vitest run billing\r\n\r\n",
        " \x1b[32m✓\x1b[0m src/routes/billing.test.ts (2 tests) 22ms\r\n",
        "   \x1b[32m✓\x1b[0m v2 charges with the new payload\r\n",
        "   \x1b[32m✓\x1b[0m v1 answers 410 with Link to v2\r\n\r\n",
        " Test Files  \x1b[32m1 passed\x1b[0m (1)\r\n      Tests  \x1b[32m2 passed\x1b[0m (2)\r\n\r\n",
        "\x1b[32mana@studio\x1b[0m \x1b[34m~/code/webhook-api\x1b[0m \x1b[33m(billing-v2)\x1b[0m $ ",
      ];
      for (const data of lines) {
        demoEmit("term-out", { id, data, exit: false });
        await sleep(40);
      }
    })();
  },
  term_write: () => undefined,
  term_resize: () => undefined,
  term_close: () => undefined,
  // The voice round: what was heard, where it goes. The verdict listen
  // never answers, so the "confirm?" face stays on screen.
  hear_once: (a) => {
    if (String(a?.owner) === "main" && !heard) {
      heard = true;
      demoEmit("hark", { kind: "mic", phase: "capturing", owner: "main" });
      return sleep(700).then(() => {
        demoEmit("hark", { kind: "mic", phase: "transcribing", owner: "main" });
        return sleep(400).then(() => "continue the billing endpoint migration and patch the two callers");
      });
    }
    return never<string>();
  },
  hear_stop: () => undefined,
  hear_abort: () => undefined,
  plan_utterance: (a): VoicePlan => ({
    kind: "work", instruction: String(a?.text ?? ""), task_title: "Billing endpoint v2", session_id: "s-billing",
    workspace: P["webhook-api"].path, project_name: "webhook-api", new_task: false, confidence: "low", warnings: [],
  }),
  interpret_verdict: (): VerdictOut => ({ kind: "unknown" }),
  voice_execute: () => undefined,
  dispatch_prechecks: () => [],
  classify_utterance: () => ({ kind: "question", options: [] }),
  interpret_followup: () => ({ kind: "stay" }),
  ask_lane: () => "lean",
  ask_text: () => ({ fala: "Two chats working, $0.42 today.", detalhes: "", itens: [] }),
  route_text: () => "lean",
  task_command: () => null,
  evaluate: () => ({ acao: "continuar_task", confianca: 0.9, motivo: "same thread", needs_confirmation: false }),
  crossref_context: () => null,
  speak: () => undefined,
  speak_stop: () => undefined,
  use_context: () => undefined,
  worker_start: () => ({ status: "started", task_id: "t-billing", directives: {} }),
  worker_send: () => ({ directives: {}, queued: null }),
  worker_queued_now: () => undefined,
  worker_queued_drop: () => undefined,
  worker_stop: () => undefined,
  worker_restart_light: () => undefined,
  worker_handoff: () => undefined,
  worker_set_mode: () => ({ directives: { mode: "manual" }, restarted: false }),
  worker_set_model: () => ({ directives: {}, restarted: false }),
  worker_set_effort: () => ({ directives: {}, restarted: false }),
  worker_interrupt: () => undefined,
  worker_lift_cap: () => undefined,
  hark_chat_send: () => ({ task_id: "hark-chat", resumed: true, queued: null }),
  chat_start: () => ({ status: "started", task_id: "t-new", directives: {} }),
  board_subtask_toggle: () => undefined,
  board_move: () => undefined,
  board_rename: () => undefined,
  chat_rename: (a) => String(a?.title ?? ""),
  board_pin: () => undefined,
  board_archive: () => undefined,
  board_archive_stale: () => 0,
  board_create: (a) => ({ title: String(a?.title ?? "task"), status: "doing", workspace: a?.workspace ?? null, session_ids: a?.sessionId ? [String(a.sessionId)] : [], updated_at: new Date().toISOString(), pinned: false, subtasks: [] }),
  approve: () => undefined,
  project_add: (a) => ({ name: String(a?.path).split("/").pop() ?? "project", path: String(a?.path) }),
  project_remove: () => undefined,
  open_task: () => undefined,
  open_project_window: () => undefined,
  focus_main: () => undefined,
  open_external: () => undefined,
  set_active_context: () => undefined,
  hud_hide: () => undefined,
  hud_show: () => undefined,
  setup_download_model: () => undefined,
  setup_mark_done: () => undefined,
  config_write: () => undefined,
  config_raw_write: () => undefined,
  tts_voices: () => [["Samantha", "en_US"], ["Luciana", "pt_BR"]],
  report_bundle: () => ({ markdown: "", version: "0.9.0", os: "macOS 15.6", arch: "aarch64", agents: "claude 2.1.4", log_lines: 0 }),
  report_issue_url: () => ({ url: "https://github.com/harkharness/hark/issues/new", logs_in_url: true, logs: null }),
  report_save: () => "/Users/ana/.hark/reports/report.md",
  report_gh_status: () => false,
  report_gh_create: () => "",
  browser_open: () => undefined,
  browser_bounds: () => undefined,
  browser_navigate: () => undefined,
  browser_history: () => undefined,
  browser_url_now: () => null,
  browser_close: () => undefined,
};
let heard = false;

/** The command names demo mode answers (the test checks the seam against it). */
export const DEMO_COMMANDS: ReadonlySet<string> = new Set(Object.keys(C));

export function demoInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const fn = C[cmd];
  if (!fn) {
    console.warn(`demo: no answer for ${cmd}`);
    return Promise.resolve(undefined as T);
  }
  try {
    return Promise.resolve(fn(args) as T);
  } catch (err) {
    return Promise.reject(err);
  }
}

// ---------- the scenes ----------

/** What a scene needs in place before React mounts (the rail is read once). */
export function seedDemo(): void {
  const s = scene();
  if (!s) return;
  document.documentElement.classList.add("demo");
  try {
    if (s === "task") localStorage.setItem("hark.rail.v2", JSON.stringify({ "Billing endpoint v2": ["arquivos", "diff"] }));
    localStorage.setItem("hark.artifacts.open", JSON.stringify(["decisions", "runbooks"]));
  } catch {
    /* no storage */
  }
  (window as unknown as { harkDemo: unknown }).harkDemo = { emit: demoEmit, scene: s };
}

/** After the first render: the events that put the scene on screen. */
export async function startDemo(): Promise<void> {
  const s = scene();
  if (!s) return;
  await sleep(500);
  switch (s) {
    case "home":
      await sleep(300);
      demoEmit("hark", { kind: "hud_listen" });
      break;
    case "task":
      demoEmit("hark", { kind: "focus_task", title: "Billing endpoint v2", session_id: "s-billing", project: P["webhook-api"] });
      await sleep(900);
      // The chat's own ledger: what this session has cost so far.
      demoEmit("hark", { kind: "worker_turn", task_id: "Billing endpoint v2", label: "Billing endpoint v2", session_id: "s-billing", agent: "claude", text: "", cost_usd: 1.96, model: "claude-opus-5-5", is_error: false, context_pct: 0.18, context_window: 200_000 });
      await sleep(200);
      // task_id = the label: the demo has no live worker to map an id through.
      demoEmit("hark-permission", { request_id: "req-1", task_id: "Billing endpoint v2", label: "Billing endpoint v2", tool_name: "Bash", input: JSON.stringify({ command: "git push -u origin billing-v2", description: "Push the branch for review" }) });
      break;
    case "board":
      demoEmit("hark", { kind: "main_tab", tab: "board" });
      break;
    case "docs":
      demoEmit("hark", { kind: "open_page", space: "webhook-api", id: "adr-004-one-window" });
      break;
    case "costs":
      demoEmit("hark", { kind: "main_tab", tab: "custos" });
      break;
    default:
      break;
  }
}
