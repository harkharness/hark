//! Pure context resolution: which workspace(s) a question is about.

/// A named focus area, kubectl-context style. Paths arrive here already
/// expanded (no `~`); expansion is the config adapter's job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextDef {
    pub name: String,
    /// A session belongs to this context when its cwd starts with any prefix.
    pub match_cwd: Vec<String>,
    /// Repositories collected into the snapshot for this context.
    pub repos: Vec<String>,
}

impl ContextDef {
    pub fn matches(&self, cwd: Option<&str>) -> bool {
        cwd.is_some_and(|c| self.match_cwd.iter().any(|prefix| c.starts_with(prefix)))
    }
}

/// Context named in the question (word match), for one-turn overrides.
pub fn hint(question: &str, names: &[String]) -> Option<String> {
    let words: Vec<String> = question
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
        .map(String::from)
        .collect();
    names
        .iter()
        .find(|name| words.iter().any(|w| w == &name.to_lowercase()))
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contexts() -> Vec<ContextDef> {
        vec![
            ContextDef {
                name: "alpha".into(),
                match_cwd: vec!["/home/dev/workspace-alpha".into(), "/home/dev/alpha-".into()],
                repos: vec!["/home/dev/workspace-alpha".into()],
            },
            ContextDef {
                name: "nu".into(),
                match_cwd: vec!["/home/dev/workspace-lab".into()],
                repos: vec![],
            },
        ]
    }

    #[test]
    fn matches_cwd_by_prefix() {
        let ctx = &contexts()[0];
        assert!(ctx.matches(Some("/home/dev/workspace-alpha")));
        assert!(ctx.matches(Some("/home/dev/workspace-alpha/repositories/alpha")));
        assert!(ctx.matches(Some("/home/dev/alpha-gladius-fix")));
        assert!(!ctx.matches(Some("/home/dev/workspace-lab")));
        assert!(!ctx.matches(None));
    }

    #[test]
    fn resolves_hint_from_question() {
        let names: Vec<String> = contexts().iter().map(|c| c.name.clone()).collect();
        assert_eq!(
            hint("no workspace nu, o que ficou pendente?", &names),
            Some("nu".to_string())
        );
        assert_eq!(
            hint("como está a migração no alpha?", &names),
            Some("alpha".to_string())
        );
        assert_eq!(hint("o que ficou pendente hoje?", &names), None);
        // Substring inside a larger word must NOT match ("continue" contains "nu"... no: word boundary).
        assert_eq!(hint("continua a tarefa", &names), None);
    }
}
