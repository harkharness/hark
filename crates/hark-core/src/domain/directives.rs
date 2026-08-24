//! Pure extraction of session directives from what the user said.
//! "planeja isso" -> plan mode, "capricha" -> max effort, "usa o opus" -> model.
//! Everything maps to real `claude` CLI flags.

use serde::{Deserialize, Serialize};

/// Permission mode, mirroring `claude --permission-mode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    Manual,
    AcceptEdits,
    Plan,
    Auto,
    Bypass,
}

impl Mode {
    pub fn as_flag(self) -> &'static str {
        match self {
            Mode::Manual => "manual",
            Mode::AcceptEdits => "acceptEdits",
            Mode::Plan => "plan",
            Mode::Auto => "auto",
            Mode::Bypass => "bypassPermissions",
        }
    }

    /// Inverse of `as_flag`, for config files and the UI mode selector.
    pub fn from_flag(flag: &str) -> Option<Self> {
        match flag {
            "manual" => Some(Mode::Manual),
            "acceptEdits" => Some(Mode::AcceptEdits),
            "plan" => Some(Mode::Plan),
            "auto" => Some(Mode::Auto),
            "bypass" | "bypassPermissions" => Some(Mode::Bypass),
            _ => None,
        }
    }

    /// Short label for the footer.
    pub fn label(self) -> &'static str {
        match self {
            Mode::Manual => "manual",
            Mode::AcceptEdits => "edições ok",
            Mode::Plan => "plano",
            Mode::Auto => "auto",
            Mode::Bypass => "sem trava",
        }
    }
}

/// Reasoning effort, mirroring `claude --effort`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Effort {
    Low,
    Medium,
    High,
    XHigh,
    Max,
}

impl Effort {
    pub fn as_flag(self) -> &'static str {
        match self {
            Effort::Low => "low",
            Effort::Medium => "medium",
            Effort::High => "high",
            Effort::XHigh => "xhigh",
            Effort::Max => "max",
        }
    }
}

/// What the user asked for, beyond the task itself.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Directives {
    pub mode: Option<Mode>,
    pub effort: Option<Effort>,
    pub model: Option<String>,
}

impl Directives {
    pub fn any(&self) -> bool {
        self.mode.is_some() || self.effort.is_some() || self.model.is_some()
    }
}

const MODE_PHRASES: &[(&[&str], Mode)] = &[
    // Bypass first and only via unambiguous phrases: it disables every guard.
    (
        &[
            "ignora as permiss", "ignorar as permiss", "sem pedir permiss", "sem trava",
            "without asking permission", "without permission", "no permission prompts",
            "skip the permissions", "bypass",
        ],
        Mode::Bypass,
    ),
    (
        &[
            "planeja", "plano antes", "faz um plano", "monta um plano", "modo plano",
            "plan first", "make a plan", "plan it out", "plan mode",
        ],
        Mode::Plan,
    ),
    (
        &[
            "aceita as edi", "aceitar as edi", "pode editar direto", "aplica direto",
            "accept the edit", "accept edits", "edit directly", "apply directly",
        ],
        Mode::AcceptEdits,
    ),
    (
        &[
            "pergunta sempre", "me pergunta antes", "modo manual",
            "ask me first", "ask me before", "always ask", "manual mode",
        ],
        Mode::Manual,
    ),
    (
        &["automático", "automatico", "modo auto", "auto mode", "automatic mode"],
        Mode::Auto,
    ),
];

const EFFORT_PHRASES: &[(&[&str], Effort)] = &[
    (
        &[
            "capricha", "esforço máximo", "esforco maximo", "mais inteligente",
            "max effort", "maximum effort", "smartest model", "best model",
        ],
        Effort::Max,
    ),
    (
        &[
            "pensa bem", "com calma", "caprichado", "bem detalhado",
            "take your time", "think it through", "think hard", "be thorough",
            "carefully",
        ],
        Effort::High,
    ),
    (
        &[
            "rápido", "rapido", "rapidinho", "mais rápido", "sem enrolar",
            "quick", "fast pass", "no need to overthink",
        ],
        Effort::Low,
    ),
];

/// Extract every directive present in the utterance.
pub fn parse(utterance: &str) -> Directives {
    let lower = utterance.to_lowercase();
    let first_match = |table: &[(&[&str], Mode)]| {
        table
            .iter()
            .find(|(needles, _)| needles.iter().any(|n| lower.contains(n)))
            .map(|(_, value)| *value)
    };
    let effort = EFFORT_PHRASES
        .iter()
        .find(|(needles, _)| needles.iter().any(|n| lower.contains(n)))
        .map(|(_, value)| *value);

    // The model router already owns explicit model requests; it returns the
    // standard tier when nothing was asked, so compare against that.
    let models = crate::domain::intent::Models::default();
    let routed = crate::domain::intent::model_for(utterance, &models);
    let explicit = (routed != models.standard || lower.contains("sonnet")).then_some(routed);

    Directives {
        mode: first_match(MODE_PHRASES),
        effort,
        model: explicit,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_round_trips_through_its_flag() {
        // Config files and the UI selector speak in flag strings; every
        // mode must come back from its own flag, and junk must not parse.
        for mode in [Mode::Manual, Mode::AcceptEdits, Mode::Plan, Mode::Auto, Mode::Bypass] {
            assert_eq!(Mode::from_flag(mode.as_flag()), Some(mode));
        }
        assert_eq!(Mode::from_flag("bypass"), Some(Mode::Bypass)); // short form
        assert_eq!(Mode::from_flag(""), None);
        assert_eq!(Mode::from_flag("turbo"), None);
    }

    #[test]
    fn detects_permission_modes() {
        let mode = |s: &str| parse(s).mode;
        assert_eq!(mode("planeja isso antes de mexer"), Some(Mode::Plan));
        assert_eq!(mode("faz um plano da migração"), Some(Mode::Plan));
        assert_eq!(mode("pode aceitar as edições"), Some(Mode::AcceptEdits));
        assert_eq!(mode("me pergunta sempre antes"), Some(Mode::Manual));
        assert_eq!(mode("modo automático"), Some(Mode::Auto));
        assert_eq!(mode("continua a migração"), None);
    }

    #[test]
    fn bypass_needs_the_explicit_dangerous_phrase() {
        assert_eq!(parse("ignora as permissões").mode, Some(Mode::Bypass));
        // Near-misses must NOT unlock it.
        assert_eq!(parse("ignora esse arquivo").mode, None);
        assert_eq!(parse("sem parar pra perguntar do lint").mode, None);
    }

    #[test]
    fn detects_effort() {
        let effort = |s: &str| parse(s).effort;
        assert_eq!(effort("resolve isso rápido"), Some(Effort::Low));
        assert_eq!(effort("capricha nessa análise"), Some(Effort::Max));
        assert_eq!(effort("pensa bem antes"), Some(Effort::High));
        assert_eq!(effort("abre o PR"), None);
    }

    #[test]
    fn reuses_the_model_router_for_explicit_models() {
        assert_eq!(parse("usa o opus e planeja").model.as_deref(), Some("opus"));
        assert_eq!(parse("com o melhor modelo").model.as_deref(), Some("fable"));
        assert_eq!(parse("abre o PR").model, None);
    }

    #[test]
    fn cli_flags_match_the_claude_interface() {
        assert_eq!(Mode::Plan.as_flag(), "plan");
        assert_eq!(Mode::AcceptEdits.as_flag(), "acceptEdits");
        assert_eq!(Mode::Bypass.as_flag(), "bypassPermissions");
        assert_eq!(Effort::XHigh.as_flag(), "xhigh");
    }

    #[test]
    fn reports_whether_anything_was_said() {
        assert!(!parse("continua a migração").any());
        assert!(parse("planeja isso").any());
    }
}

#[cfg(test)]
mod bilingual {
    use super::*;

    #[test]
    fn english_effort_directives() {
        assert_eq!(parse("run the tests quickly").effort, Some(Effort::Low));
        assert_eq!(parse("take your time on this one").effort, Some(Effort::High));
        assert_eq!(parse("max effort here").effort, Some(Effort::Max));
    }

    #[test]
    fn english_mode_directives() {
        assert_eq!(parse("do it without asking permission").mode, Some(Mode::Bypass));
        assert_eq!(parse("plan first, then implement").mode, Some(Mode::Plan));
        assert_eq!(parse("accept the edits"). mode, Some(Mode::AcceptEdits));
        assert_eq!(parse("ask me first").mode, Some(Mode::Manual));
    }

    #[test]
    fn plain_english_work_carries_no_directive() {
        assert!(!parse("update the readme").any());
    }
}
