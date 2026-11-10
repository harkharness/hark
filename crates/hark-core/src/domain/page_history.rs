//! The versions a page keeps: the text as it was before each change.
//!
//! Hark cannot see a file before an agent overwrites it, so versions are
//! what the PERSON saw and accepted: the first opening takes the
//! baseline, "accept" takes the current text, "go back" keeps the
//! current text before restoring an old one, and an edit through Hark
//! keeps the text before it. The viewer marks what differs from the
//! newest version. Pure: names and pruning; the store keeps the files.

/// How many versions a page keeps; the oldest beyond this are dropped.
pub const KEEP: usize = 30;

/// A version's file name from the ISO instant it was taken: colons are
/// not welcome in every file system.
pub fn file_name(at: &str) -> String {
    at.replace(':', "-")
}

/// The ISO instant back from a version's file name.
pub fn instant(name: &str) -> String {
    // 2026-10-28T10-05-00Z → 2026-10-28T10:05:00Z: only the time part had
    // colons, after the "T".
    match name.split_once('T') {
        Some((day, time)) => format!("{day}T{}", time.replacen('-', ":", 2)),
        None => name.to_string(),
    }
}

/// The instants to drop so that at most `KEEP` remain: the oldest first.
/// `instants` must be sorted oldest first.
pub fn to_prune(instants: &[String]) -> Vec<String> {
    if instants.len() <= KEEP {
        return vec![];
    }
    instants[..instants.len() - KEEP].to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_version_file_name_is_the_instant_without_colons_and_back() {
        assert_eq!(file_name("2026-10-28T10:05:00Z"), "2026-10-28T10-05-00Z");
        assert_eq!(instant("2026-10-28T10-05-00Z"), "2026-10-28T10:05:00Z");
        assert_eq!(instant("2026-10-28T10-05-00.123Z"), "2026-10-28T10:05:00.123Z");
        assert_eq!(instant("odd"), "odd");
    }

    #[test]
    fn pruning_keeps_the_newest_thirty() {
        let list: Vec<String> = (0..KEEP + 3).map(|i| format!("2026-10-28T11:{i:02}:00Z")).collect();
        assert_eq!(to_prune(&list), list[..3].to_vec());
        assert!(to_prune(&list[..KEEP]).is_empty());
    }
}
