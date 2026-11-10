//! A dashboard page: panels over data that lives on this machine.
//!
//! The file the agent writes:
//!
//! ```json
//! { "title": "Uso do app", "refresh": "hourly",
//!   "sources": [ { "id": "db", "kind": "sqlite", "path": "data/app.db" },
//!                { "id": "ev", "kind": "csv", "path": "data/eventos.csv" },
//!                { "id": "st", "kind": "command", "command": "./scripts/stats.sh" } ],
//!   "queries": [ { "id": "por_dia", "source": "db", "sql": "SELECT dia, n FROM …", "certified": true },
//!                { "id": "retencao", "source": "ev", "columns": ["mes", "d7"], "limit": 12 } ],
//!   "panels":  [ { "title": "ativos", "query": "ativos", "chart": "number", "value": "n" },
//!                { "title": "por dia", "query": "por_dia", "chart": "bar", "x": "dia", "y": "n" } ] }
//! ```
//!
//! A source is a file in the project, a sqlite file (opened read-only)
//! or a command of the project (run only when the person asks for a
//! refresh). A query is sql for sqlite, a column pick for the rest. A
//! panel draws one query: a number, bars, a line, a table. Results are
//! cached beside the page with the instant they were taken; the cadence
//! says when they are stale. "certified" is the person's mark on a
//! query: the agent uses it, never rewrites it. Pure: running things is
//! the adapter's; this module says what to run, what to draw, and when.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Dash {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default)]
    pub refresh: Refresh,
    #[serde(default)]
    pub sources: Vec<Source>,
    #[serde(default)]
    pub queries: Vec<Query>,
    #[serde(default)]
    pub panels: Vec<Panel>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Refresh {
    #[default]
    Manual,
    Hourly,
    Daily,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Source {
    pub id: String,
    pub kind: SourceKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    Sqlite,
    Csv,
    Json,
    Command,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Query {
    pub id: String,
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sql: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub columns: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
    #[serde(default)]
    pub certified: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Chart {
    Number,
    Bar,
    Line,
    #[default]
    Table,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Panel {
    pub title: String,
    pub query: String,
    #[serde(default)]
    pub chart: Chart,
    /// The column of the labels (bars, line); the first text column when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<String>,
    /// The column of the values (bars, line); the first numeric column when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<String>,
    /// The column of a number panel; the first numeric column when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// A number panel's "vs": a column holding the previous value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous: Option<String>,
}

/// Rows and columns, as every source yields them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Table {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Value>>,
}

/// One query's cached result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Result_ {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub table: Option<Table>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// The cache beside the page: when it was taken, every query's result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Cache {
    #[serde(default)]
    pub at: String,
    #[serde(default)]
    pub results: std::collections::BTreeMap<String, Result_>,
}

/// Where a page's cache lives: `x.dash.json` → `x.dash.cache.json`
/// (not a page extension, so never a page).
pub fn cache_path(page_path: &str) -> String {
    match page_path.strip_suffix(".dash.json") {
        Some(stem) => format!("{stem}.dash.cache.json"),
        None => format!("{page_path}.cache.json"),
    }
}

/// The file as a dashboard, or the error the agent needs to see.
pub fn parse(json: &str) -> Result<Dash, String> {
    let v: Value = serde_json::from_str(json).map_err(|e| format!("não é json: {e}"))?;
    if !v.is_object() {
        return Err("o painel é um objeto com \"sources\", \"queries\" e \"panels\"".into());
    }
    let dash: Dash = serde_json::from_value(v).map_err(|e| format!("painel inválido: {e}"))?;
    for s in &dash.sources {
        match s.kind {
            SourceKind::Command if s.command.as_deref().unwrap_or("").trim().is_empty() => {
                return Err(format!("fonte {}: sem \"command\"", s.id))
            }
            SourceKind::Command => {}
            _ if s.path.as_deref().unwrap_or("").trim().is_empty() => return Err(format!("fonte {}: sem \"path\"", s.id)),
            _ => {}
        }
        if let Some(p) = &s.path {
            validate_source_path(p).map_err(|e| format!("fonte {}: {e}", s.id))?;
        }
    }
    for q in &dash.queries {
        let Some(src) = dash.sources.iter().find(|s| s.id == q.source) else {
            return Err(format!("consulta {}: fonte \"{}\" não existe", q.id, q.source));
        };
        if src.kind == SourceKind::Sqlite && q.sql.as_deref().unwrap_or("").trim().is_empty() {
            return Err(format!("consulta {}: uma fonte sqlite pede \"sql\"", q.id));
        }
    }
    for p in &dash.panels {
        if !dash.queries.iter().any(|q| q.id == p.query) {
            return Err(format!("painel \"{}\": consulta \"{}\" não existe", p.title, p.query));
        }
    }
    Ok(dash)
}

/// A source path is relative to the project and stays inside it.
pub fn validate_source_path(path: &str) -> Result<(), String> {
    if path.starts_with('/') || path.starts_with('~') || path.contains('\\') {
        return Err(format!("{path}: o caminho é relativo ao projeto"));
    }
    if path.split('/').any(|seg| seg.is_empty() || seg == "." || seg == "..") {
        return Err(format!("{path}: sem `..` nem segmentos vazios"));
    }
    Ok(())
}

/// Whether the cache is stale for the cadence. No cache is always stale.
pub fn stale(refresh: Refresh, cache_at: Option<&str>, now: &str) -> bool {
    let Some(at) = cache_at.filter(|a| !a.is_empty()) else { return true };
    let (Ok(at), Ok(now)) = (
        chrono::DateTime::parse_from_rfc3339(at),
        chrono::DateTime::parse_from_rfc3339(now),
    ) else {
        return true;
    };
    let age = now.signed_duration_since(at);
    match refresh {
        Refresh::Manual => false,
        Refresh::Hourly => age >= chrono::Duration::hours(1),
        Refresh::Daily => age >= chrono::Duration::days(1),
    }
}

/// Only SELECT (and WITH … SELECT) goes to a sqlite source: the page
/// reads data, it never changes it.
pub fn read_only_sql(sql: &str) -> Result<(), String> {
    let head = sql.trim_start().to_ascii_lowercase();
    if head.starts_with("select") || head.starts_with("with") || head.starts_with("explain") || head.starts_with("pragma table_info") {
        if head.contains(';') && head.trim_end_matches(';').contains(';') {
            return Err("uma consulta só".into());
        }
        Ok(())
    } else {
        Err("só SELECT: o painel lê, não escreve".into())
    }
}

/// CSV text as a table: the first line is the header; quoted fields with
/// commas, quotes and newlines are read the usual way; a field that
/// reads as a number becomes one.
pub fn parse_csv(text: &str) -> Table {
    let mut records: Vec<Vec<String>> = Vec::new();
    let mut record: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    let sep = if text.lines().next().is_some_and(|l| l.matches(';').count() > l.matches(',').count()) { ';' } else { ',' };
    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    field.push('"');
                    chars.next();
                } else {
                    quoted = false;
                }
            } else {
                field.push(c);
            }
        } else if c == '"' {
            quoted = true;
        } else if c == sep {
            record.push(std::mem::take(&mut field));
        } else if c == '\n' || c == '\r' {
            if c == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
            record.push(std::mem::take(&mut field));
            if record.iter().any(|f| !f.trim().is_empty()) {
                records.push(std::mem::take(&mut record));
            } else {
                record.clear();
            }
        } else {
            field.push(c);
        }
    }
    if !field.is_empty() || !record.is_empty() {
        record.push(field);
        if record.iter().any(|f| !f.trim().is_empty()) {
            records.push(record);
        }
    }
    let mut it = records.into_iter();
    let columns = it.next().map(|h| h.into_iter().map(|c| c.trim().to_string()).collect()).unwrap_or_default();
    let rows = it.map(|r| r.into_iter().map(|f| cell_value(&f)).collect()).collect();
    Table { columns, rows }
}

fn cell_value(f: &str) -> Value {
    let t = f.trim();
    if t.is_empty() {
        return Value::Null;
    }
    // "01", "007": a code or a label, not a number.
    let digits = t.trim_start_matches('-');
    if digits.len() > 1 && digits.starts_with('0') && !digits.starts_with("0.") && !digits.starts_with("0,") {
        return Value::String(t.to_string());
    }
    if let Ok(n) = t.parse::<i64>() {
        return Value::from(n);
    }
    if let Ok(n) = t.replace(',', ".").parse::<f64>() {
        if t.chars().any(|c| c.is_ascii_digit()) {
            return Value::from(n);
        }
    }
    Value::String(t.to_string())
}

/// JSON text as a table: an array of objects (columns from the first),
/// an array of arrays (columns A, B, …), or `{columns, rows}`.
pub fn parse_json_table(text: &str) -> Result<Table, String> {
    let v: Value = serde_json::from_str(text).map_err(|e| format!("não é json: {e}"))?;
    if let Some(obj) = v.as_object() {
        if let (Some(cols), Some(rows)) = (obj.get("columns"), obj.get("rows")) {
            let t: Table = serde_json::from_value(serde_json::json!({ "columns": cols, "rows": rows })).map_err(|e| e.to_string())?;
            return Ok(t);
        }
        return Err("um objeto precisa de \"columns\" e \"rows\"".into());
    }
    let Some(items) = v.as_array() else { return Err("esperava uma lista".into()) };
    if items.is_empty() {
        return Ok(Table::default());
    }
    if let Some(first) = items[0].as_object() {
        let columns: Vec<String> = first.keys().cloned().collect();
        let rows = items
            .iter()
            .map(|it| columns.iter().map(|c| it.get(c).cloned().unwrap_or(Value::Null)).collect())
            .collect();
        return Ok(Table { columns, rows });
    }
    if items[0].is_array() {
        let width = items.iter().filter_map(|r| r.as_array()).map(Vec::len).max().unwrap_or(0);
        let columns = (0..width).map(super::sheet::column_name).collect();
        let rows = items.iter().filter_map(|r| r.as_array().cloned()).collect();
        return Ok(Table { columns, rows });
    }
    Err("esperava objetos ou listas".into())
}

/// A table-source query: the columns picked (all when none), the first
/// `limit` rows. A column that is not there is an error, named.
pub fn pick(table: &Table, query: &Query) -> Result<Table, String> {
    let idx: Vec<usize> = if query.columns.is_empty() {
        (0..table.columns.len()).collect()
    } else {
        query
            .columns
            .iter()
            .map(|c| table.columns.iter().position(|t| t.eq_ignore_ascii_case(c)).ok_or_else(|| format!("coluna \"{c}\" não existe")))
            .collect::<Result<_, _>>()?
    };
    let columns = idx.iter().map(|&i| table.columns[i].clone()).collect();
    let rows = table
        .rows
        .iter()
        .take(query.limit.unwrap_or(usize::MAX))
        .map(|r| idx.iter().map(|&i| r.get(i).cloned().unwrap_or(Value::Null)).collect())
        .collect();
    Ok(Table { columns, rows })
}

/// A panel as the viewer draws it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PanelView {
    pub title: String,
    pub query: String,
    pub chart: Chart,
    pub certified: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// number: the value; bar/line: unused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number: Option<f64>,
    /// number: the change against `previous`, as a fraction (0.062 = +6.2%).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delta: Option<f64>,
    /// bar/line: the labels and values, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub values: Vec<f64>,
    /// table: the rows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub table: Option<Table>,
}

fn as_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().replace(',', ".").parse().ok(),
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        _ => None,
    }
}

fn as_label(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        Value::Number(n) => n.to_string(),
        other => other.to_string(),
    }
}

/// The column a panel names, or the first that fits (numeric for values,
/// non-numeric for labels), or an error that names what is missing.
fn column(table: &Table, named: Option<&str>, numeric: bool) -> Result<usize, String> {
    if let Some(name) = named {
        return table.columns.iter().position(|c| c.eq_ignore_ascii_case(name)).ok_or_else(|| format!("coluna \"{name}\" não existe"));
    }
    let first_row = table.rows.first();
    let is_numeric = |i: usize| first_row.and_then(|r| r.get(i)).and_then(as_f64).is_some();
    (0..table.columns.len())
        .find(|&i| is_numeric(i) == numeric)
        .ok_or_else(|| if numeric { "nenhuma coluna numérica".to_string() } else { "nenhuma coluna de rótulo".to_string() })
}

/// What one panel shows, from its query's cached result.
pub fn panel_view(dash: &Dash, panel: &Panel, result: Option<&Result_>) -> PanelView {
    let certified = dash.queries.iter().any(|q| q.id == panel.query && q.certified);
    let base = PanelView {
        title: panel.title.clone(),
        query: panel.query.clone(),
        chart: panel.chart,
        certified,
        error: None,
        number: None,
        delta: None,
        labels: vec![],
        values: vec![],
        table: None,
    };
    let err = |e: String| PanelView { error: Some(e), ..base.clone() };
    let Some(result) = result else { return err("sem resultado ainda: atualize o painel".into()) };
    if let Some(e) = &result.error {
        return err(e.clone());
    }
    let Some(table) = &result.table else { return err("sem resultado".into()) };
    match panel.chart {
        Chart::Table => PanelView { table: Some(table.clone()), ..base },
        Chart::Number => {
            let col = match column(table, panel.value.as_deref(), true) {
                Ok(c) => c,
                Err(e) => return err(e),
            };
            let Some(row) = table.rows.first() else { return err("sem linhas".into()) };
            let number = row.get(col).and_then(as_f64);
            let delta = panel.previous.as_deref().and_then(|p| {
                let pi = table.columns.iter().position(|c| c.eq_ignore_ascii_case(p))?;
                let prev = row.get(pi).and_then(as_f64)?;
                let cur = number?;
                (prev != 0.0).then(|| (cur - prev) / prev.abs())
            });
            PanelView { number, delta, ..base }
        }
        Chart::Bar | Chart::Line => {
            let y = match column(table, panel.y.as_deref(), true) {
                Ok(c) => c,
                Err(e) => return err(e),
            };
            let x = column(table, panel.x.as_deref(), false).ok();
            let labels = table.rows.iter().enumerate().map(|(i, r)| x.and_then(|x| r.get(x)).map(as_label).unwrap_or_else(|| (i + 1).to_string())).collect();
            let values = table.rows.iter().map(|r| r.get(y).and_then(as_f64).unwrap_or(0.0)).collect();
            PanelView { labels, values, ..base }
        }
    }
}

/// A query the agent changed or added since the baseline file: the
/// person's mark ("certified") is kept only when the sql is the same.
pub fn changed_queries(before: &Dash, after: &Dash) -> Vec<String> {
    after
        .queries
        .iter()
        .filter(|q| {
            before.queries.iter().find(|b| b.id == q.id).map(|b| b.sql != q.sql || b.columns != q.columns || b.source != q.source).unwrap_or(true)
        })
        .map(|q| q.id.clone())
        .collect()
}

/// The person certifies a query, or takes the mark back.
pub fn set_certified(dash: &mut Dash, id: &str, on: bool) -> Result<(), String> {
    let q = dash.queries.iter_mut().find(|q| q.id == id).ok_or_else(|| format!("consulta \"{id}\" não existe"))?;
    q.certified = on;
    Ok(())
}

/// The dashboard back as the file.
pub fn to_json(dash: &Dash) -> String {
    let mut s = serde_json::to_string_pretty(dash).unwrap_or_else(|_| "{}".into());
    s.push('\n');
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Dash {
        parse(
            r#"{ "title": "Uso", "refresh": "hourly",
              "sources": [ {"id": "db", "kind": "sqlite", "path": "data/app.db"},
                           {"id": "ev", "kind": "csv", "path": "data/eventos.csv"},
                           {"id": "st", "kind": "command", "command": "./scripts/stats.sh"} ],
              "queries": [ {"id": "por_dia", "source": "db", "sql": "SELECT dia, n FROM x", "certified": true},
                           {"id": "retencao", "source": "ev", "columns": ["mes", "d7"], "limit": 2},
                           {"id": "ativos", "source": "st"} ],
              "panels": [ {"title": "ativos", "query": "ativos", "chart": "number", "value": "n", "previous": "antes"},
                          {"title": "por dia", "query": "por_dia", "chart": "bar", "x": "dia", "y": "n"},
                          {"title": "retenção", "query": "retencao", "chart": "line"},
                          {"title": "tudo", "query": "por_dia"} ] }"#,
        )
        .unwrap()
    }

    #[test]
    fn a_dashboard_is_parsed_and_its_loose_ends_are_named() {
        let d = sample();
        assert_eq!(d.refresh, Refresh::Hourly);
        assert_eq!(d.sources.len(), 3);
        assert_eq!(d.panels[3].chart, Chart::Table, "a panel without a chart is a table");
        assert!(parse("nope").unwrap_err().contains("json"));
        assert!(parse(r#"{"sources":[{"id":"a","kind":"csv"}]}"#).unwrap_err().contains("path"));
        assert!(parse(r#"{"sources":[{"id":"a","kind":"command"}]}"#).unwrap_err().contains("command"));
        assert!(parse(r#"{"sources":[{"id":"a","kind":"csv","path":"../x.csv"}]}"#).unwrap_err().contains(".."));
        assert!(parse(r#"{"sources":[{"id":"a","kind":"csv","path":"/etc/x.csv"}]}"#).unwrap_err().contains("relativo"));
        assert!(parse(r#"{"queries":[{"id":"q","source":"zz"}]}"#).unwrap_err().contains("zz"));
        assert!(parse(r#"{"sources":[{"id":"db","kind":"sqlite","path":"a.db"}],"queries":[{"id":"q","source":"db"}]}"#).unwrap_err().contains("sql"));
        assert!(parse(r#"{"panels":[{"title":"p","query":"q"}]}"#).unwrap_err().contains("q"));
        assert_eq!(cache_path("uso.dash.json"), "uso.dash.cache.json");
    }

    #[test]
    fn the_cache_is_stale_by_the_cadence_and_manual_never_is() {
        let at = Some("2026-11-10T10:00:00Z");
        assert!(stale(Refresh::Hourly, None, "2026-11-10T10:30:00Z"), "no cache is stale");
        assert!(!stale(Refresh::Hourly, at, "2026-11-10T10:30:00Z"));
        assert!(stale(Refresh::Hourly, at, "2026-11-10T11:00:00Z"));
        assert!(!stale(Refresh::Daily, at, "2026-11-11T09:59:00Z"));
        assert!(stale(Refresh::Daily, at, "2026-11-11T10:00:00Z"));
        assert!(!stale(Refresh::Manual, at, "2027-01-01T00:00:00Z"));
        assert!(stale(Refresh::Manual, Some("garbage"), "2026-11-10T10:30:00Z"));
    }

    #[test]
    fn only_a_select_reaches_a_sqlite_source() {
        assert!(read_only_sql("SELECT 1").is_ok());
        assert!(read_only_sql("  with a as (select 1) select * from a;").is_ok());
        assert!(read_only_sql("DELETE FROM x").is_err());
        assert!(read_only_sql("select 1; drop table x").is_err());
        assert!(read_only_sql("PRAGMA table_info(x)").is_ok());
    }

    #[test]
    fn csv_and_json_become_tables_with_numbers_read_as_numbers() {
        let t = parse_csv("mes,d7,nota\nset,38,\"a, b\"\nout,41.5,\"diz \"\"oi\"\"\"\n\nnov,,x\n");
        assert_eq!(t.columns, vec!["mes", "d7", "nota"]);
        assert_eq!(t.rows[0], vec![Value::from("set"), Value::from(38), Value::from("a, b")]);
        assert_eq!(t.rows[1][1], Value::from(41.5));
        assert_eq!(t.rows[1][2], Value::from("diz \"oi\""));
        assert_eq!(t.rows[2][1], Value::Null);
        let semi = parse_csv("a;b\r\n1;2\r\n");
        assert_eq!(semi.rows[0], vec![Value::from(1), Value::from(2)], "a semicolon csv is read too");
        let j = parse_json_table(r#"[{"dia":"01","n":3},{"dia":"02","n":5}]"#).unwrap();
        assert_eq!(j.columns, vec!["dia", "n"]);
        assert_eq!(j.rows[1][1], Value::from(5));
        let a = parse_json_table("[[1,2],[3]]").unwrap();
        assert_eq!(a.columns, vec!["A", "B"]);
        assert_eq!(a.rows[1], vec![Value::from(3)]);
        let o = parse_json_table(r#"{"columns":["x"],"rows":[[1]]}"#).unwrap();
        assert_eq!(o.rows[0][0], Value::from(1));
        assert!(parse_json_table("{}").is_err());
        assert!(parse_json_table("3").is_err());
    }

    #[test]
    fn a_table_query_picks_columns_and_rows_and_names_a_missing_column() {
        let t = parse_csv("mes,d7,x\nset,38,1\nout,41,2\nnov,52,3\n");
        let q = &sample().queries[1];
        let out = pick(&t, q).unwrap();
        assert_eq!(out.columns, vec!["mes", "d7"]);
        assert_eq!(out.rows.len(), 2, "limit 2");
        let all = pick(&t, &Query { id: "a".into(), source: "ev".into(), sql: None, columns: vec![], limit: None, certified: false }).unwrap();
        assert_eq!(all.columns.len(), 3);
        let bad = Query { columns: vec!["nope".into()], ..q.clone() };
        assert!(pick(&t, &bad).unwrap_err().contains("nope"));
    }

    #[test]
    fn panels_draw_their_query_number_bars_line_or_table_and_say_what_is_missing() {
        let d = sample();
        let mut cache = Cache::default();
        cache.results.insert("ativos".into(), Result_ { table: Some(parse_csv("n,antes\n1284,1209\n")), error: None });
        cache.results.insert("por_dia".into(), Result_ { table: Some(parse_csv("dia,n\n01,3\n02,5\n")), error: None });
        cache.results.insert("retencao".into(), Result_ { table: Some(parse_csv("mes,d7\nset,38\nout,41\n")), error: None });
        let number = panel_view(&d, &d.panels[0], cache.results.get("ativos"));
        assert_eq!(number.number, Some(1284.0));
        assert!((number.delta.unwrap() - 0.062).abs() < 0.001);
        assert!(!number.certified);
        let bars = panel_view(&d, &d.panels[1], cache.results.get("por_dia"));
        assert_eq!(bars.labels, vec!["01", "02"]);
        assert_eq!(bars.values, vec![3.0, 5.0]);
        assert!(bars.certified);
        let line = panel_view(&d, &d.panels[2], cache.results.get("retencao"));
        assert_eq!(line.labels, vec!["set", "out"], "x is the first text column when not named");
        assert_eq!(line.values, vec![38.0, 41.0]);
        let table = panel_view(&d, &d.panels[3], cache.results.get("por_dia"));
        assert_eq!(table.table.as_ref().unwrap().rows.len(), 2);
        let none = panel_view(&d, &d.panels[0], None);
        assert!(none.error.as_deref().unwrap().contains("atualize"));
        let failed = panel_view(&d, &d.panels[0], Some(&Result_ { table: None, error: Some("no such table".into()) }));
        assert_eq!(failed.error.as_deref(), Some("no such table"));
        let wrong = Panel { value: Some("zzz".into()), ..d.panels[0].clone() };
        assert!(panel_view(&d, &wrong, cache.results.get("ativos")).error.unwrap().contains("zzz"));
    }

    #[test]
    fn the_queries_the_agent_changed_are_named_and_certifying_is_the_persons() {
        let before = sample();
        let mut after = sample();
        after.queries[0].sql = Some("SELECT dia, n FROM y".into());
        after.queries.push(Query { id: "nova".into(), source: "db".into(), sql: Some("SELECT 1".into()), columns: vec![], limit: None, certified: false });
        assert_eq!(changed_queries(&before, &after), vec!["por_dia", "nova"]);
        assert!(changed_queries(&before, &before).is_empty());
        set_certified(&mut after, "nova", true).unwrap();
        assert!(after.queries[3].certified);
        assert!(set_certified(&mut after, "zzz", true).is_err());
        assert_eq!(parse(&to_json(&after)).unwrap(), after);
    }
}
