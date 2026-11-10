//! What a dashboard's sources need, run on this machine and inside one
//! project folder: a read-only query over a sqlite file, a file's text,
//! a command's output. Nothing here reaches outside the project — a
//! path that climbs or links out is refused before anything is read —
//! and sqlite is opened read-only, so a query cannot change the data.

use crate::domain::dash::{read_only_sql, Table};
use crate::ports::DataRunner;
use std::path::{Path, PathBuf};

pub struct LocalRunner;

/// At most this many rows come back from a query: a dashboard draws a
/// summary, not a dump.
const MAX_ROWS: usize = 5_000;
/// A command gets this long; a dashboard is not a batch job.
const COMMAND_SECS: u64 = 30;
/// A file read whole is at most this big.
const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;

/// The real path of a source inside the project, or why not.
fn inside(project: &Path, path: &str) -> anyhow::Result<PathBuf> {
    crate::domain::dash::validate_source_path(path).map_err(anyhow::Error::msg)?;
    let root = std::fs::canonicalize(project).map_err(|e| anyhow::anyhow!("projeto {}: {e}", project.display()))?;
    let file = root.join(path);
    let real = std::fs::canonicalize(&file).map_err(|e| anyhow::anyhow!("{path}: {e}"))?;
    if !real.starts_with(&root) {
        anyhow::bail!("{path}: fora do projeto");
    }
    if !real.is_file() {
        anyhow::bail!("{path}: não é um arquivo");
    }
    Ok(real)
}

impl DataRunner for LocalRunner {
    fn sql(&self, project: &Path, path: &str, sql: &str) -> anyhow::Result<Table> {
        read_only_sql(sql).map_err(anyhow::Error::msg)?;
        let file = inside(project, path)?;
        let flags = rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let conn = rusqlite::Connection::open_with_flags(&file, flags).map_err(|e| anyhow::anyhow!("{path}: {e}"))?;
        let mut stmt = conn.prepare(sql).map_err(|e| anyhow::anyhow!("sql: {e}"))?;
        let columns: Vec<String> = stmt.column_names().iter().map(|c| c.to_string()).collect();
        let width = columns.len();
        let mut rows = Vec::new();
        let mut it = stmt.query([]).map_err(|e| anyhow::anyhow!("sql: {e}"))?;
        while let Some(row) = it.next().map_err(|e| anyhow::anyhow!("sql: {e}"))? {
            let mut out = Vec::with_capacity(width);
            for i in 0..width {
                let v = match row.get_ref(i)? {
                    rusqlite::types::ValueRef::Null => serde_json::Value::Null,
                    rusqlite::types::ValueRef::Integer(n) => serde_json::Value::from(n),
                    rusqlite::types::ValueRef::Real(f) => serde_json::Value::from(f),
                    rusqlite::types::ValueRef::Text(t) => serde_json::Value::from(String::from_utf8_lossy(t).to_string()),
                    rusqlite::types::ValueRef::Blob(b) => serde_json::Value::from(format!("<blob {} bytes>", b.len())),
                };
                out.push(v);
            }
            rows.push(out);
            if rows.len() >= MAX_ROWS {
                break;
            }
        }
        Ok(Table { columns, rows })
    }

    fn file(&self, project: &Path, path: &str) -> anyhow::Result<String> {
        let file = inside(project, path)?;
        let len = std::fs::metadata(&file)?.len();
        if len > MAX_FILE_BYTES {
            anyhow::bail!("{path}: {} MB é grande demais para um painel", len / (1024 * 1024));
        }
        Ok(std::fs::read_to_string(file)?)
    }

    fn command(&self, project: &Path, command: &str) -> anyhow::Result<String> {
        if !project.is_dir() {
            anyhow::bail!("projeto {}: não é uma pasta", project.display());
        }
        let mut child = std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg(command)
            .current_dir(project)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| anyhow::anyhow!("{command}: {e}"))?;
        let started = std::time::Instant::now();
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }
            if started.elapsed() > std::time::Duration::from_secs(COMMAND_SECS) {
                let _ = child.kill();
                anyhow::bail!("{command}: passou de {COMMAND_SECS} s e foi parado");
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        };
        let out = child.wait_with_output()?;
        if !status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            anyhow::bail!("{command}: saiu com {} {}", status.code().unwrap_or(-1), err.trim());
        }
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sqlite_source_answers_a_select_and_refuses_a_write() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("app.db");
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute_batch("CREATE TABLE lancamentos(dia TEXT, n INTEGER, v REAL); INSERT INTO lancamentos VALUES ('01', 3, 1.5), ('02', 5, NULL);").unwrap();
        drop(conn);
        let t = LocalRunner.sql(dir.path(), "app.db", "SELECT dia, n, v FROM lancamentos ORDER BY dia").unwrap();
        assert_eq!(t.columns, vec!["dia", "n", "v"]);
        assert_eq!(t.rows[0], vec![serde_json::json!("01"), serde_json::json!(3), serde_json::json!(1.5)]);
        assert_eq!(t.rows[1][2], serde_json::Value::Null);
        assert!(LocalRunner.sql(dir.path(), "app.db", "DELETE FROM lancamentos").is_err());
        assert!(LocalRunner.sql(dir.path(), "app.db", "SELECT * FROM nope").unwrap_err().to_string().contains("nope"));
        assert!(LocalRunner.sql(dir.path(), "../app.db", "SELECT 1").is_err());
    }

    #[test]
    fn a_file_source_stays_inside_the_project() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("data")).unwrap();
        std::fs::write(dir.path().join("data/ev.csv"), "a,b\n1,2\n").unwrap();
        assert_eq!(LocalRunner.file(dir.path(), "data/ev.csv").unwrap(), "a,b\n1,2\n");
        assert!(LocalRunner.file(dir.path(), "data/../../etc/passwd").is_err());
        assert!(LocalRunner.file(dir.path(), "/etc/passwd").is_err());
        assert!(LocalRunner.file(dir.path(), "data").is_err(), "a folder is not a file");
        #[cfg(unix)]
        {
            let outside = dir.path().parent().unwrap().join(format!("hark-outside-{}.csv", std::process::id()));
            std::fs::write(&outside, "x").unwrap();
            std::os::unix::fs::symlink(&outside, dir.path().join("data/link.csv")).unwrap();
            assert!(LocalRunner.file(dir.path(), "data/link.csv").is_err(), "a link out of the project is refused");
            let _ = std::fs::remove_file(outside);
        }
    }

    #[test]
    fn a_command_source_runs_in_the_project_and_reports_a_failure() {
        let dir = tempfile::tempdir().unwrap();
        let out = LocalRunner.command(dir.path(), "printf 'a,b\\n1,2\\n'").unwrap();
        assert_eq!(out, "a,b\n1,2\n");
        let err = LocalRunner.command(dir.path(), "echo nope >&2; exit 3").unwrap_err().to_string();
        assert!(err.contains("3") && err.contains("nope"), "{err}");
        assert_eq!(LocalRunner.command(dir.path(), "pwd").unwrap().trim(), std::fs::canonicalize(dir.path()).unwrap().to_string_lossy());
    }
}
