//! What leaves the machine in a bug report, scrubbed.
//!
//! A report is pasted into a PUBLIC GitHub issue by the person filing it.
//! Whatever reaches this function may end up there, so it strips the
//! things that identify a person or unlock an account: home directories,
//! tokens, e-mails, credentials inside URLs, and the values of settings
//! whose names say they are secret. Dependency-free on purpose: it runs
//! on every log line as it is written, not only on the day of the report.

/// Scrub one text. Idempotent: scrubbing twice changes nothing.
pub fn redact(text: &str) -> String {
    text.split('\n').map(redact_line).collect::<Vec<_>>().join("\n")
}

fn redact_line(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut redact_next = false;
    for (i, word) in line.split(' ').enumerate() {
        if i > 0 {
            out.push(' ');
        }
        if redact_next {
            // "Authorization: Bearer <token>": the scheme word is kept,
            // the credential after it goes.
            if word == "Bearer" || word == "bearer" {
                out.push_str(word);
                continue;
            }
            redact_next = false;
            out.push_str(&keep_trailing_punct(word, "<redacted>"));
            continue;
        }
        // "Bearer <token>" and "api_key: <value>" span two words.
        if word == "Bearer" || word == "bearer" || (word.ends_with(':') && secret_key(&word[..word.len() - 1])) {
            redact_next = true;
            out.push_str(word);
            continue;
        }
        out.push_str(&redact_word(word));
    }
    out
}

fn redact_word(word: &str) -> String {
    // `KEY=value` and `KEY: value` glued together: the value goes when the
    // key says it is a secret, or a gateway address.
    for sep in ['=', ':'] {
        if let Some((key, value)) = word.split_once(sep) {
            if key.is_empty() || key.contains("//") || value.is_empty() || value.starts_with("//") {
                continue;
            }
            if secret_key(key) {
                return format!("{key}{sep}{}", keep_trailing_punct(value, "<redacted>"));
            }
            if key.to_ascii_uppercase().ends_with("_BASE_URL") {
                return format!("{key}{sep}{}", keep_trailing_punct(value, "<url>"));
            }
        }
    }
    let bare = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '/' && c != '~' && c != '@' && c != '.');
    if bare.is_empty() {
        return word.to_string();
    }
    if looks_like_token(bare) {
        return keep_trailing_punct(word, "<redacted>");
    }
    if bare.contains('@') && bare.contains('.') && !bare.contains('/') && !bare.starts_with('@') {
        return keep_trailing_punct(word, "<email>");
    }
    let mut w = strip_userinfo(word);
    w = fold_home(&w);
    w
}

/// Well-known secret shapes: Anthropic and GitHub tokens, long opaque
/// strings after `sk-`.
fn looks_like_token(word: &str) -> bool {
    let w = word.trim_end_matches(|c: char| !c.is_alphanumeric());
    if w.starts_with("sk-ant-") {
        return true;
    }
    if w.starts_with("sk-") && w.len() >= 24 {
        return true;
    }
    const GH: [&str; 6] = ["ghp_", "gho_", "ghu_", "ghs_", "ghr_", "github_pat_"];
    GH.iter().any(|p| w.starts_with(p) && w.len() > p.len() + 12)
}

fn secret_key(key: &str) -> bool {
    let k = key.trim_matches(|c: char| !c.is_alphanumeric() && c != '_').to_ascii_uppercase();
    ["TOKEN", "SECRET", "PASSWORD", "PASSWD", "API_KEY", "APIKEY", "AUTHORIZATION", "COOKIE"]
        .iter()
        .any(|s| k.ends_with(s) || k == *s)
        || k == "KEY"
}

/// `scheme://user:pass@host/...` → `scheme://host/...`
fn strip_userinfo(word: &str) -> String {
    let Some(scheme_end) = word.find("://") else { return word.to_string() };
    let rest = &word[scheme_end + 3..];
    let host_end = rest.find('/').unwrap_or(rest.len());
    let authority = &rest[..host_end];
    match authority.rfind('@') {
        Some(at) => format!("{}{}{}", &word[..scheme_end + 3], &authority[at + 1..], &rest[host_end..]),
        None => word.to_string(),
    }
}

/// `/Users/<login>/x` and `/home/<login>/x` → `~/x`.
fn fold_home(word: &str) -> String {
    let mut out = word.to_string();
    for prefix in ["/Users/", "/home/"] {
        while let Some(start) = out.find(prefix) {
            let after = start + prefix.len();
            let login_len = out[after..].find(['/', ' ', '"', '\'']).unwrap_or(out.len() - after);
            if login_len == 0 {
                break;
            }
            out.replace_range(start..after + login_len, "~");
        }
    }
    out
}

fn keep_trailing_punct(word: &str, replacement: &str) -> String {
    let trailing: String = word.chars().rev().take_while(|c| matches!(c, ',' | ';' | ')' | ']' | '}' | '"' | '\'' | '.')).collect::<Vec<_>>().into_iter().rev().collect();
    format!("{replacement}{trailing}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_home_directory_is_a_tilde() {
        assert_eq!(redact("cwd /Users/maria/Projects/app ok"), "cwd ~/Projects/app ok");
        assert_eq!(redact("file:///home/jo/x.txt"), "file://~/x.txt");
        assert_eq!(redact("~/Projects/app stays"), "~/Projects/app stays");
    }

    #[test]
    fn tokens_go_whatever_carries_them() {
        assert_eq!(redact("key sk-ant-api03-abcdefghijklmnopqrstuvwxyz0123"), "key <redacted>");
        assert_eq!(redact("Authorization: Bearer eyJhbGciOiJIUzI1NiJ9.abc"), "Authorization: Bearer <redacted>");
        assert_eq!(redact("token ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789,"), "token <redacted>,");
        assert_eq!(redact("ANTHROPIC_AUTH_TOKEN=abc123 ok"), "ANTHROPIC_AUTH_TOKEN=<redacted> ok");
        assert_eq!(redact("api_key: 12345"), "api_key: <redacted>");
        assert_eq!(redact("password:hunter2;"), "password:<redacted>;");
    }

    #[test]
    fn a_gateway_address_is_not_published() {
        assert_eq!(redact("ANTHROPIC_BASE_URL=https://llm.example.corp/v1"), "ANTHROPIC_BASE_URL=<url>");
        assert_eq!(redact("https://user:pw@host.example/path"), "https://host.example/path");
    }

    #[test]
    fn an_email_is_an_email() {
        assert_eq!(redact("by dev@acme.example."), "by <email>.");
        assert_eq!(redact("@mention stays"), "@mention stays");
    }

    #[test]
    fn ordinary_text_is_untouched_and_scrubbing_is_idempotent() {
        let line = "12:00 worker_turn task=t-1 error_code=budget cost=0.42 model=claude-opus-5";
        assert_eq!(redact(line), line);
        let once = redact("Bearer abc /Users/x/y a@b.co");
        assert_eq!(redact(&once), once);
    }
}
