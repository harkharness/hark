//! Which language Hark WRITES and SPEAKS in.
//!
//! Recognition is bilingual on purpose — the grammar accepts Portuguese and
//! English side by side, because people mix them mid-sentence. Output has
//! to pick one, and this is the pick: `ui_language` from the config.

/// The interface language. Portuguese is the default the app ships with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    #[default]
    Pt,
    En,
}

impl Lang {
    /// From a config code ("pt", "pt-BR", "en", "en_US"). Anything else
    /// falls back to Portuguese rather than guessing.
    pub fn from_code(code: &str) -> Self {
        match code.trim().to_lowercase().split(['-', '_']).next() {
            Some("en") => Lang::En,
            _ => Lang::Pt,
        }
    }

    /// Pick the string for this language. Reads at the call site as
    /// `lang.pick("olá", "hello")` — the Portuguese one always first.
    pub fn pick<'a>(self, pt: &'a str, en: &'a str) -> &'a str {
        match self {
            Lang::Pt => pt,
            Lang::En => en,
        }
    }
}

/// The language code handed to the speech model. `"auto"` (or an empty
/// setting) lets it detect per utterance — the only way someone who mixes
/// Portuguese and English mid-sentence gets a clean transcription, at the
/// cost of a little accuracy on very short ones. Region suffixes are
/// dropped: the model knows "pt", not "pt-BR".
pub fn stt_code(configured: &str) -> String {
    let code = configured.trim().to_lowercase();
    match code.split(['-', '_']).next() {
        None | Some("") | Some("auto") => "auto".to_string(),
        Some(base) => base.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stt_codes_drop_regions_and_pass_auto_through() {
        assert_eq!(stt_code("pt"), "pt");
        assert_eq!(stt_code("pt-BR"), "pt");
        assert_eq!(stt_code("en_US"), "en");
        assert_eq!(stt_code(" EN "), "en");
        // Empty and "auto" both mean: detect it per utterance.
        assert_eq!(stt_code("auto"), "auto");
        assert_eq!(stt_code(""), "auto");
    }

    #[test]
    fn unknown_codes_fall_back_to_portuguese() {
        assert_eq!(Lang::from_code("en"), Lang::En);
        assert_eq!(Lang::from_code("EN"), Lang::En);
        assert_eq!(Lang::from_code("en-US"), Lang::En);
        assert_eq!(Lang::from_code("en_GB"), Lang::En);
        assert_eq!(Lang::from_code("pt"), Lang::Pt);
        assert_eq!(Lang::from_code("pt-BR"), Lang::Pt);
        assert_eq!(Lang::from_code(""), Lang::Pt);
        assert_eq!(Lang::from_code("de"), Lang::Pt);
    }

    #[test]
    fn pick_returns_the_side_that_matches() {
        assert_eq!(Lang::Pt.pick("olá", "hello"), "olá");
        assert_eq!(Lang::En.pick("olá", "hello"), "hello");
    }
}
