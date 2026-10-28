//! `hark://` links: what a copied link says, and what an opened one asks
//! for. Pure; the shell registers the scheme and acts on the target.

/// What an opened link asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// `hark://chat/<session>`: the chat of that session, where it lives.
    Chat { session: String },
    /// `hark://artifacts/<space>/<id>`: that page in the artifacts view.
    Page { space: String, id: String },
}

pub const SCHEME: &str = "hark://";

/// Percent-encode what is not safe in a path segment (spaces, slashes,
/// percent signs, anything outside printable ASCII).
fn encode(seg: &str) -> String {
    let mut out = String::with_capacity(seg.len());
    for b in seg.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn decode(seg: &str) -> Option<String> {
    let bytes = seg.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = seg.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

pub fn chat_link(session: &str) -> String {
    format!("{SCHEME}chat/{}", encode(session))
}

pub fn page_link(space: &str, id: &str) -> String {
    format!("{SCHEME}artifacts/{}/{}", encode(space), encode(id))
}

/// A segment that is a real name: not empty, not a climb, no slash.
fn clean(seg: &str) -> Option<String> {
    let s = decode(seg)?;
    (!s.is_empty() && s != "." && s != ".." && !s.contains('/')).then_some(s)
}

/// The target of a link, or None for anything that is not one of ours.
pub fn parse(url: &str) -> Option<Target> {
    let rest = url.strip_prefix(SCHEME)?;
    let rest = rest.trim_end_matches('/');
    let mut parts = rest.split('/');
    match (parts.next()?, parts.next(), parts.next(), parts.next()) {
        ("chat", Some(session), None, None) => Some(Target::Chat { session: clean(session)? }),
        ("artifacts", Some(space), Some(id), None) => Some(Target::Page { space: clean(space)?, id: clean(id)? }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chat_link_carries_its_session_and_comes_back_as_one() {
        assert_eq!(chat_link("0c4f91ab"), "hark://chat/0c4f91ab");
        assert_eq!(parse("hark://chat/0c4f91ab"), Some(Target::Chat { session: "0c4f91ab".into() }));
    }

    #[test]
    fn a_page_link_names_its_space_and_page_with_spaces_escaped() {
        assert_eq!(page_link("hark", "adr-004"), "hark://artifacts/hark/adr-004");
        assert_eq!(page_link("meu projeto", "a"), "hark://artifacts/meu%20projeto/a");
        assert_eq!(parse("hark://artifacts/meu%20projeto/a"), Some(Target::Page { space: "meu projeto".into(), id: "a".into() }));
        assert_eq!(parse("hark://artifacts/hark/"), None, "a space alone is not a page");
    }

    #[test]
    fn anything_else_is_not_a_target() {
        assert_eq!(parse("https://example.com/chat/x"), None);
        assert_eq!(parse("hark://nope/x"), None);
        assert_eq!(parse("hark://chat/"), None);
        assert_eq!(parse("hark://artifacts/../x/y"), None, "no climbing through a link");
    }
}
