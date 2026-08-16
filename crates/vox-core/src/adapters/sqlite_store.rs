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
}
