//! Pure fuzzy scoring for file paths: powers the @mention autocomplete,
//! the Cmd+P quick-open and spoken file references ("abre o arquivo readme
//! do projeto vox"). No I/O here; callers bring the path list.

/// Score a path against a query. `None` means "does not match at all".
/// Higher is better. Every query token must appear somewhere in the path;
/// hits on the basename beat hits buried in directories, and shorter paths
/// win ties.
pub fn score(query: &str, path: &str) -> Option<i64> {
    let path_lower = path.to_lowercase();
    let base = path_lower.rsplit('/').next().unwrap_or(&path_lower);
    let stem = base.rsplit_once('.').map(|(s, _)| s).unwrap_or(base);
    let mut total = 0i64;
    let mut any = false;
    for token in query
        .to_lowercase()
        .split(|c: char| c.is_whitespace() || c == '/')
        .filter(|t| !t.is_empty())
    {
        any = true;
        total += if base == token || stem == token {
            80
        } else if base.starts_with(token) {
            40
        } else if base.contains(token) {
            25
        } else if path_lower
            .split('/')
            .any(|seg| seg.starts_with(token))
        {
            12
        } else if path_lower.contains(token) {
            5
        } else {
            return None;
        };
    }
    if !any {
        return None;
    }
    // Prefer shallow, short paths when token scores tie.
    Some(total - path_lower.len() as i64 / 16 - 2 * path_lower.matches('/').count() as i64)
}

/// Rank `paths` against `query`, best first, capped at `limit`.
/// An empty query returns the first `limit` paths untouched (browse mode).
pub fn search<'a>(
    query: &str,
    paths: impl IntoIterator<Item = &'a str>,
    limit: usize,
) -> Vec<String> {
    if query.trim().is_empty() {
        return paths.into_iter().take(limit).map(String::from).collect();
    }
    let mut hits: Vec<(i64, &str)> = paths
        .into_iter()
        .filter_map(|p| score(query, p).map(|s| (s, p)))
        .collect();
    hits.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.len().cmp(&b.1.len())));
    hits.into_iter()
        .take(limit)
        .map(|(_, p)| p.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basename_hits_beat_directory_hits() {
        let paths = [
            "src/main.rs",
            "README.md",
            "docs/readme-notes/extra.txt",
        ];
        let hits = search("readme", paths, 10);
        assert_eq!(hits[0], "README.md");
        assert!(hits.contains(&"docs/readme-notes/extra.txt".to_string()));
        assert!(!hits.contains(&"src/main.rs".to_string()));
    }

    #[test]
    fn every_token_must_match_somewhere() {
        assert!(score("gate rs", "crates/vox-core/src/domain/gate.rs").is_some());
        assert_eq!(score("gate py", "crates/vox-core/src/domain/gate.rs"), None);
    }

    #[test]
    fn queries_are_case_insensitive_and_accept_slashes() {
        assert!(score("Domain/Gate", "crates/vox-core/src/domain/gate.rs").is_some());
        assert!(score("APP TSX", "src/App.tsx").is_some());
    }

    #[test]
    fn exact_basename_beats_prefix_matches() {
        let paths = ["src/board_utils.rs", "src/board.rs"];
        assert_eq!(search("board", paths, 10)[0], "src/board.rs");
    }

    #[test]
    fn empty_query_browses_and_limit_caps() {
        let paths = ["a.rs", "b.rs", "c.rs"];
        assert_eq!(search("", paths, 2), vec!["a.rs", "b.rs"]);
        assert_eq!(search("rs", paths, 2).len(), 2);
    }
}
