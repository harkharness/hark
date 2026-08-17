//! SQLite-backed session index (rusqlite, bundled). Stores per-file fold
//! state so re-indexing only parses appended bytes.

use crate::domain::snapshot::{RecentPrompt, SessionSummary};
use crate::ports::SessionStore;
use rusqlite::{params, Connection, OptionalExtension};

pub struct SqliteStore {
    conn: Connection,
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS files (
    path        TEXT PRIMARY KEY,
    session_id  TEXT NOT NULL,
    cwd         TEXT,
    git_branch  TEXT,
    title       TEXT,
    last_prompt TEXT,
    last_ts     TEXT,
    byte_offset INTEGER NOT NULL DEFAULT 0,
    mtime       INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS prompts (
    path TEXT NOT NULL,
    ts   TEXT NOT NULL,
    text TEXT NOT NULL,
    PRIMARY KEY (path, ts, text)
);
CREATE INDEX IF NOT EXISTS idx_files_last_ts ON files(last_ts);
CREATE TABLE IF NOT EXISTS board (
    title       TEXT PRIMARY KEY,
    task        TEXT NOT NULL  -- full Task as JSON
);
CREATE TABLE IF NOT EXISTS spend (
    id                   INTEGER PRIMARY KEY AUTOINCREMENT,
    ts                   TEXT NOT NULL,
    kind                 TEXT NOT NULL,   -- ask|gate|worker|dispatch|session
    source               TEXT NOT NULL,   -- live|jsonl (NEVER aggregate across)
    task_id              TEXT,
    label                TEXT,
    session_id           TEXT,
    workspace            TEXT,
    model                TEXT NOT NULL,
    input_tokens         INTEGER NOT NULL DEFAULT 0,
    output_tokens        INTEGER NOT NULL DEFAULT 0,
    cache_read_tokens    INTEGER NOT NULL DEFAULT 0,
    cache_created_tokens INTEGER NOT NULL DEFAULT 0,
    cost_usd             REAL,            -- NULL on jsonl rows (no USD on disk)
    duration_ms          INTEGER,
    is_error             INTEGER NOT NULL DEFAULT 0,
    is_sidechain         INTEGER NOT NULL DEFAULT 0,
    context_window       INTEGER,
    request_id           TEXT,
    outcome              TEXT
);
CREATE INDEX IF NOT EXISTS idx_spend_ts ON spend(ts);
CREATE INDEX IF NOT EXISTS idx_spend_session ON spend(session_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_spend_request
    ON spend(request_id, model) WHERE request_id IS NOT NULL;
";

impl SqliteStore {
    pub fn open(path: &std::path::Path) -> anyhow::Result<Self> {
        Self::from_conn(Connection::open(path)?)
    }

    pub fn in_memory() -> anyhow::Result<Self> {
        Self::from_conn(Connection::open_in_memory()?)
    }

    fn from_conn(conn: Connection) -> anyhow::Result<Self> {
        conn.execute_batch(SCHEMA)?;
        Ok(Self { conn })
    }

    fn row_to_summary(row: &rusqlite::Row) -> rusqlite::Result<SessionSummary> {
        Ok(SessionSummary {
            session_id: row.get("session_id")?,
            cwd: row.get("cwd")?,
            git_branch: row.get("git_branch")?,
            title: row.get("title")?,
            last_prompt: row.get("last_prompt")?,
            last_ts: row.get("last_ts")?,
            recent_prompts: Vec::new(), // filled separately
        })
    }

    /// Load the whole board (small by nature).
    fn board_impl(&self) -> anyhow::Result<Vec<crate::domain::board::Task>> {
        let mut stmt = self.conn.prepare("SELECT task FROM board")?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows
            .iter()
            .filter_map(|json| serde_json::from_str(json).ok())
            .collect())
    }

    /// Replace the whole board (updates arrive already merged).
    fn save_board_impl(&mut self, tasks: &[crate::domain::board::Task]) -> anyhow::Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM board", [])?;
        for task in tasks {
            tx.execute(
                "INSERT INTO board (title, task) VALUES (?1, ?2)",
                params![task.title, serde_json::to_string(task)?],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// (input+cache_read+cache_created, context_window) of the newest live
    /// non-sidechain row of a session: the weight the next turn drags.
    pub fn last_context_weight(
        &self,
        session_id: &str,
    ) -> anyhow::Result<Option<(u64, Option<u64>)>> {
        let row = self
            .conn
            .query_row(
                "SELECT input_tokens + cache_read_tokens + cache_created_tokens, context_window
                 FROM spend
                 WHERE session_id = ?1 AND source = 'live' AND is_sidechain = 0
                 ORDER BY ts DESC, id DESC LIMIT 1",
                params![session_id],
                |r| {
                    Ok((
                        r.get::<_, i64>(0)? as u64,
                        r.get::<_, Option<i64>>(1)?.map(|w| w as u64),
                    ))
                },
            )
            .optional()?;
        Ok(row)
    }

    fn row_to_agg(row: &rusqlite::Row) -> rusqlite::Result<crate::ports::SpendAgg> {
        Ok(crate::ports::SpendAgg {
            key: row.get(0)?,
            cost_usd: row.get(1)?,
            usage: crate::domain::claude_event::TokenUsage {
                input: row.get::<_, i64>(2)? as u64,
                output: row.get::<_, i64>(3)? as u64,
                cache_read: row.get::<_, i64>(4)? as u64,
                cache_created: row.get::<_, i64>(5)? as u64,
            },
            turns: row.get::<_, i64>(6)? as u64,
            errors: row.get::<_, i64>(7)? as u64,
        })
    }

    fn prompts_for(&self, path: &str) -> anyhow::Result<Vec<RecentPrompt>> {
        let mut stmt = self
            .conn
            .prepare("SELECT ts, text FROM prompts WHERE path = ?1 ORDER BY ts ASC")?;
        let rows = stmt
            .query_map(params![path], |r| {
                Ok(RecentPrompt {
                    ts: r.get(0)?,
                    text: r.get(1)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

impl crate::ports::SpendLedger for SqliteStore {
    fn record_spend(&mut self, rows: &[crate::domain::spend::SpendRow]) -> anyhow::Result<()> {
        let tx = self.conn.transaction()?;
        for r in rows {
            // OR IGNORE + unique(request_id, model): jsonl rebuilds are idempotent.
            tx.execute(
                "INSERT OR IGNORE INTO spend (ts, kind, source, task_id, label, session_id,
                    workspace, model, input_tokens, output_tokens, cache_read_tokens,
                    cache_created_tokens, cost_usd, duration_ms, is_error, is_sidechain,
                    context_window, request_id, outcome)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19)",
                params![
                    r.ts,
                    r.kind.as_str(),
                    r.source.as_str(),
                    r.task_id,
                    r.label,
                    r.session_id,
                    r.workspace,
                    r.model,
                    r.usage.input as i64,
                    r.usage.output as i64,
                    r.usage.cache_read as i64,
                    r.usage.cache_created as i64,
                    r.cost_usd,
                    r.duration_ms.map(|d| d as i64),
                    r.is_error as i64,
                    r.is_sidechain as i64,
                    r.context_window.map(|w| w as i64),
                    r.request_id,
                    r.outcome,
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    fn spend_summary(
        &self,
        query: &crate::ports::SpendQuery,
    ) -> anyhow::Result<Vec<crate::ports::SpendAgg>> {
        use crate::ports::SpendGroup;
        let group = match query.group {
            SpendGroup::Kind => "kind",
            SpendGroup::Model => "model",
            SpendGroup::Label => "COALESCE(label, '—')",
            SpendGroup::Workspace => "COALESCE(workspace, '—')",
            SpendGroup::Day => "substr(ts, 1, 10)",
            SpendGroup::Session => "COALESCE(session_id, '—')",
        };
        let sql = format!(
            "SELECT {group} AS k,
                    COALESCE(SUM(cost_usd), 0.0),
                    COALESCE(SUM(input_tokens), 0), COALESCE(SUM(output_tokens), 0),
                    COALESCE(SUM(cache_read_tokens), 0), COALESCE(SUM(cache_created_tokens), 0),
                    COUNT(DISTINCT ts),
                    COUNT(DISTINCT CASE WHEN is_error = 1 THEN ts END)
             FROM spend
             WHERE source = ?1 AND (?2 IS NULL OR ts >= ?2)
             GROUP BY k
             ORDER BY 2 DESC, 5 DESC"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt
            .query_map(params![query.source.as_str(), query.since], Self::row_to_agg)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn spend_top_sessions(
        &self,
        since: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<crate::ports::SpendAgg>> {
        let mut stmt = self.conn.prepare(
            "SELECT COALESCE(session_id, '—') AS k,
                    COALESCE(SUM(cost_usd), 0.0),
                    COALESCE(SUM(input_tokens), 0), COALESCE(SUM(output_tokens), 0),
                    COALESCE(SUM(cache_read_tokens), 0), COALESCE(SUM(cache_created_tokens), 0),
                    COUNT(DISTINCT ts),
                    COUNT(DISTINCT CASE WHEN is_error = 1 THEN ts END)
             FROM spend
             WHERE source = 'live' AND ts >= ?1 AND session_id IS NOT NULL
             GROUP BY k ORDER BY 2 DESC LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![since, limit as i64], Self::row_to_agg)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

impl SessionStore for SqliteStore {
    fn session_path(&self, session_id: &str) -> anyhow::Result<Option<String>> {
        Ok(self
            .conn
            .query_row(
                "SELECT path FROM files WHERE session_id = ?1 LIMIT 1",
                params![session_id],
                |r| r.get::<_, String>(0),
            )
            .optional()?)
    }

    fn board(&self) -> anyhow::Result<Vec<crate::domain::board::Task>> {
        self.board_impl()
    }

    fn save_board(&mut self, tasks: &[crate::domain::board::Task]) -> anyhow::Result<()> {
        self.save_board_impl(tasks)
    }

    fn file_state(&self, path: &str) -> anyhow::Result<Option<(SessionSummary, u64, i64)>> {
        let row = self
            .conn
            .query_row(
                "SELECT session_id, cwd, git_branch, title, last_prompt, last_ts,
                        byte_offset, mtime
                 FROM files WHERE path = ?1",
                params![path],
                |row| {
                    Ok((
                        Self::row_to_summary(row)?,
                        row.get::<_, u64>("byte_offset")?,
                        row.get::<_, i64>("mtime")?,
                    ))
                },
            )
            .optional()?;
        match row {
            None => Ok(None),
            Some((mut summary, offset, mtime)) => {
                summary.recent_prompts = self.prompts_for(path)?;
                Ok(Some((summary, offset, mtime)))
            }
        }
    }

    fn save_file_state(
        &mut self,
        path: &str,
        summary: &SessionSummary,
        offset: u64,
        mtime: i64,
    ) -> anyhow::Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO files
               (path, session_id, cwd, git_branch, title, last_prompt, last_ts,
                byte_offset, mtime)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)
             ON CONFLICT(path) DO UPDATE SET
               session_id=?2, cwd=?3, git_branch=?4, title=?5, last_prompt=?6,
               last_ts=?7, byte_offset=?8, mtime=?9",
            params![
                path,
                summary.session_id,
                summary.cwd,
                summary.git_branch,
                summary.title,
                summary.last_prompt,
                summary.last_ts,
                offset,
                mtime,
            ],
        )?;
        tx.execute("DELETE FROM prompts WHERE path = ?1", params![path])?;
        for p in &summary.recent_prompts {
            tx.execute(
                "INSERT OR IGNORE INTO prompts (path, ts, text) VALUES (?1,?2,?3)",
                params![path, p.ts, p.text],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    fn search_sessions(&self, terms: &[String], limit: usize) -> anyhow::Result<Vec<SessionSummary>> {
        if terms.is_empty() {
            return Ok(Vec::new());
        }
        // Small index (a few thousand prompts): LIKE scan is instant and
        // avoids an FTS5 dependency. Score = prompt-hit frequency plus a
        // strong title bonus; a session titled after the topic with dozens
        // of mentions must beat a session that mentions it in passing.
        let mut scored: std::collections::HashMap<String, (i64, Option<String>)> =
            std::collections::HashMap::new();
        for term in terms {
            let pattern = format!("%{}%", term.to_lowercase());
            let mut stmt = self.conn.prepare(
                "SELECT f.path, f.last_ts,
                        (CASE WHEN lower(coalesce(f.title,'')) LIKE ?1 THEN 10 ELSE 0 END)
                        + (SELECT COUNT(*) FROM prompts p
                           WHERE p.path = f.path AND lower(p.text) LIKE ?1) AS score
                 FROM files f",
            )?;
            let rows = stmt
                .query_map(params![pattern], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, Option<String>>(1)?,
                        r.get::<_, i64>(2)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            for (path, last_ts, score) in rows.into_iter().filter(|(_, _, s)| *s > 0) {
                let entry = scored.entry(path).or_insert((0, last_ts));
                entry.0 += score;
            }
        }
        let mut ranked: Vec<(String, i64, Option<String>)> = scored
            .into_iter()
            .map(|(path, (hits, ts))| (path, hits, ts))
            .collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then(b.2.cmp(&a.2)));
        ranked
            .into_iter()
            .take(limit)
            .map(|(path, _, _)| {
                let (summary, _, _) = self
                    .file_state(&path)?
                    .ok_or_else(|| anyhow::anyhow!("row vanished: {path}"))?;
                Ok(summary)
            })
            .collect()
    }

    fn sessions_since(&self, iso_ts: &str) -> anyhow::Result<Vec<SessionSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT path, session_id, cwd, git_branch, title, last_prompt, last_ts
             FROM files WHERE last_ts >= ?1 ORDER BY last_ts DESC",
        )?;
        let rows = stmt
            .query_map(params![iso_ts], |row| {
                Ok((row.get::<_, String>("path")?, Self::row_to_summary(row)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|(path, mut summary)| {
                summary.recent_prompts = self.prompts_for(&path)?;
                Ok(summary)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::snapshot::{RecentPrompt, SessionSummary};
    use crate::ports::SessionStore;

    fn summary(id: &str, ts: &str) -> SessionSummary {
        SessionSummary {
            session_id: id.into(),
            cwd: Some("/home/dev/proj".into()),
            git_branch: Some("main".into()),
            title: Some("Some work".into()),
            last_prompt: Some("do the thing".into()),
            last_ts: Some(ts.into()),
            recent_prompts: vec![RecentPrompt {
                ts: ts.into(),
                text: "do the thing".into(),
            }],
        }
    }

    #[test]
    fn roundtrips_file_state() {
        let mut store = SqliteStore::in_memory().unwrap();
        let s = summary("abc", "2026-08-14T10:00:00.000Z");

        assert!(store.file_state("/logs/abc.jsonl").unwrap().is_none());
        store.save_file_state("/logs/abc.jsonl", &s, 4096, 1755).unwrap();

        let (loaded, offset, mtime) = store.file_state("/logs/abc.jsonl").unwrap().unwrap();
        assert_eq!(loaded, s);
        assert_eq!(offset, 4096);
        assert_eq!(mtime, 1755);
    }

    #[test]
    fn save_is_upsert() {
        let mut store = SqliteStore::in_memory().unwrap();
        store
            .save_file_state("/logs/abc.jsonl", &summary("abc", "2026-08-14T10:00:00.000Z"), 10, 1)
            .unwrap();
        store
            .save_file_state("/logs/abc.jsonl", &summary("abc", "2026-08-14T11:00:00.000Z"), 20, 2)
            .unwrap();

        let (loaded, offset, _) = store.file_state("/logs/abc.jsonl").unwrap().unwrap();
        assert_eq!(loaded.last_ts.as_deref(), Some("2026-08-14T11:00:00.000Z"));
        assert_eq!(offset, 20);
    }

    #[test]
    fn search_finds_old_sessions_by_topic_terms() {
        let mut store = SqliteStore::in_memory().unwrap();
        let mut webhook = summary("webhook-old", "2026-07-20T10:00:00.000Z");
        webhook.title = Some("Migração webhook UAT".into());
        webhook.recent_prompts = vec![RecentPrompt {
            ts: "2026-07-20T10:00:00.000Z".into(),
            text: "valida o webhook em uat e prepara o decom".into(),
        }];
        store.save_file_state("/logs/webhook.jsonl", &webhook, 1, 1).unwrap();
        store
            .save_file_state("/logs/other.jsonl", &summary("other", "2026-08-14T10:00:00.000Z"), 1, 1)
            .unwrap();

        let hits = store
            .search_sessions(&["migração".into(), "webhook".into()], 5)
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].session_id, "webhook-old");

        assert!(store.search_sessions(&["kafka".into()], 5).unwrap().is_empty());
    }

    #[test]
    fn search_ranks_frequency_and_title_over_passing_mentions() {
        let mut store = SqliteStore::in_memory().unwrap();

        // The real topic session: titled after it, many mentions, OLD.
        let mut real = summary("webhook-real", "2026-07-20T10:00:00.000Z");
        real.title = Some("Carteira webhook migration".into());
        real.recent_prompts = (0..8)
            .map(|i| RecentPrompt {
                ts: format!("2026-07-20T10:0{i}:00.000Z"),
                text: format!("passo {i} da migração do webhook em uat"),
            })
            .collect();
        store.save_file_state("/logs/real.jsonl", &real, 1, 1).unwrap();

        // A NEWER session that mentions the topic once, in passing.
        let mut passing = summary("passing", "2026-08-15T10:00:00.000Z");
        passing.title = Some("Outro assunto".into());
        passing.recent_prompts = vec![RecentPrompt {
            ts: "2026-08-15T10:00:00.000Z".into(),
            text: "aproveita e olha o webhook depois, e roda a migração do banco atualmente parada".into(),
        }];
        store.save_file_state("/logs/passing.jsonl", &passing, 1, 1).unwrap();

        let hits = store
            .search_sessions(
                &["migração".into(), "webhook".into(), "atualmente".into()],
                5,
            )
            .unwrap();
        assert_eq!(hits[0].session_id, "webhook-real", "frequency+title must win");
    }

    #[test]
    fn sessions_since_filters_and_sorts_newest_first() {
        let mut store = SqliteStore::in_memory().unwrap();
        store
            .save_file_state("/logs/old.jsonl", &summary("old", "2026-08-10T10:00:00.000Z"), 1, 1)
            .unwrap();
        store
            .save_file_state("/logs/a.jsonl", &summary("a", "2026-08-14T09:00:00.000Z"), 1, 1)
            .unwrap();
        store
            .save_file_state("/logs/b.jsonl", &summary("b", "2026-08-14T11:00:00.000Z"), 1, 1)
            .unwrap();

        let recent = store.sessions_since("2026-08-14T00:00:00.000Z").unwrap();
        let ids: Vec<_> = recent.iter().map(|s| s.session_id.as_str()).collect();
        assert_eq!(ids, vec!["b", "a"]);
    }

    #[test]
    fn spend_ledger_records_aggregates_and_never_mixes_sources() {
        use crate::domain::claude_event::TokenUsage;
        use crate::domain::spend::{SpendKind, SpendRow, SpendSource};
        use crate::ports::{SpendGroup, SpendLedger, SpendQuery};

        let mut store = SqliteStore::in_memory().unwrap();
        let row = |ts: &str, kind: SpendKind, source: SpendSource, cost: Option<f64>,
                   request: Option<&str>, error: bool| SpendRow {
            ts: ts.into(),
            kind,
            source,
            task_id: None,
            label: Some("migração".into()),
            session_id: Some("s-1".into()),
            workspace: Some("/p/vox".into()),
            model: "claude-sonnet-5".into(),
            usage: TokenUsage { input: 10, output: 20, cache_read: 100, cache_created: 5 },
            cost_usd: cost,
            duration_ms: Some(900),
            is_error: error,
            is_sidechain: false,
            context_window: Some(200_000),
            request_id: request.map(String::from),
            outcome: None,
        };

        store
            .record_spend(&[
                row("2026-08-17T10:00:00Z", SpendKind::Worker, SpendSource::Live, Some(0.05), None, false),
                row("2026-08-17T11:00:00Z", SpendKind::Ask, SpendSource::Live, Some(0.01), None, true),
                row("2026-08-17T10:00:01Z", SpendKind::Session, SpendSource::Jsonl, None, Some("req-1"), false),
            ])
            .unwrap();
        // Rebuild replays the same jsonl row: unique(request_id, model) ignores it.
        store
            .record_spend(&[row(
                "2026-08-17T10:00:01Z", SpendKind::Session, SpendSource::Jsonl, None, Some("req-1"), false,
            )])
            .unwrap();

        let live = store
            .spend_summary(&SpendQuery {
                since: None,
                group: SpendGroup::Kind,
                source: SpendSource::Live,
            })
            .unwrap();
        let total: f64 = live.iter().map(|a| a.cost_usd).sum();
        assert!((total - 0.06).abs() < 1e-9, "USD only from live rows");
        assert_eq!(live.iter().map(|a| a.turns).sum::<u64>(), 2);
        assert_eq!(live.iter().map(|a| a.errors).sum::<u64>(), 1);

        let jsonl = store
            .spend_summary(&SpendQuery {
                since: None,
                group: SpendGroup::Model,
                source: SpendSource::Jsonl,
            })
            .unwrap();
        assert_eq!(jsonl.len(), 1);
        assert_eq!(jsonl[0].usage.input, 10, "duplicate request_id ignored");
        assert_eq!(jsonl[0].cost_usd, 0.0, "jsonl rows carry no USD");

        let top = store.spend_top_sessions("2026-08-17T00:00:00Z", 5).unwrap();
        assert_eq!(top[0].key, "s-1");
        assert!((top[0].cost_usd - 0.06).abs() < 1e-9);

        // Window filter respects `since`.
        let later = store
            .spend_summary(&SpendQuery {
                since: Some("2026-08-17T10:30:00Z".into()),
                group: SpendGroup::Kind,
                source: SpendSource::Live,
            })
            .unwrap();
        assert_eq!(later.len(), 1);
        assert_eq!(later[0].key, "ask");
    }
}
