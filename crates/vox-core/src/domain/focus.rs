//! Which window is the spoken word about? A pure ledger of reporting
//! windows and their focus order. Only windows that REPORT context
//! participate — the mother and the HUD focusing must never erase what
//! the user was just looking at (that bug sent work to the global ask).

/// Focus ledger over per-window context payloads.
#[derive(Debug, Clone, Default)]
pub struct FocusLedger<C> {
    /// (window label, its last reported context).
    reports: Vec<(String, C)>,
    /// Focus order, most recent LAST. Only reporting labels appear.
    order: Vec<String>,
}

impl<C: Clone> FocusLedger<C> {
    /// A window reported its context (on focus or when its task changed).
    /// Updates in place; only the very first report grabs focus (a task
    /// change in a background window must not steal it).
    pub fn report(&mut self, label: &str, ctx: C) {
        match self.reports.iter_mut().find(|(l, _)| l == label) {
            Some((_, existing)) => *existing = ctx,
            None => self.reports.push((label.to_string(), ctx)),
        }
        if self.order.is_empty() {
            self.order.push(label.to_string());
        }
    }

    /// The OS focused a window. Unknown labels (mother, hud, anything
    /// that never reported) are a no-op.
    pub fn focused(&mut self, label: &str) {
        if !self.reports.iter().any(|(l, _)| l == label) {
            return;
        }
        self.order.retain(|l| l != label);
        self.order.push(label.to_string());
    }

    /// A window closed: drop it and fall back to the previous one.
    pub fn destroyed(&mut self, label: &str) {
        self.reports.retain(|(l, _)| l != label);
        self.order.retain(|l| l != label);
    }

    /// Context of the window the user is (or was last) working in.
    pub fn current(&self) -> Option<&C> {
        let label = self.order.last()?;
        self.reports.iter().find(|(l, _)| l == label).map(|(_, c)| c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ledger() -> FocusLedger<&'static str> {
        let mut l = FocusLedger::default();
        l.report("proj-a", "ctx-a");
        l.report("proj-b", "ctx-b");
        l
    }

    #[test]
    fn last_focused_reporting_window_wins() {
        let mut l = ledger();
        l.focused("proj-a");
        assert_eq!(l.current(), Some(&"ctx-a"));
        l.focused("proj-b");
        assert_eq!(l.current(), Some(&"ctx-b"));
    }

    #[test]
    fn mother_or_hud_focus_never_clears_context() {
        let mut l = ledger();
        l.focused("proj-a");
        l.focused("main");
        l.focused("hud");
        assert_eq!(l.current(), Some(&"ctx-a"));
    }

    #[test]
    fn destroyed_window_falls_back_to_previous() {
        let mut l = ledger();
        l.focused("proj-a");
        l.focused("proj-b");
        l.destroyed("proj-b");
        assert_eq!(l.current(), Some(&"ctx-a"));
    }

    #[test]
    fn destroying_the_last_window_clears() {
        let mut l = FocusLedger::default();
        l.report("proj-a", "ctx-a");
        l.destroyed("proj-a");
        assert_eq!(l.current(), None);
    }

    #[test]
    fn task_update_on_refocus_overwrites_stale_task() {
        let mut l = ledger();
        l.focused("proj-a");
        l.focused("proj-b");
        // proj-a re-reports (its focused task changed) WITHOUT regaining
        // OS focus: payload updates, focus order stays with proj-b.
        l.report("proj-a", "ctx-a2");
        assert_eq!(l.current(), Some(&"ctx-b"));
        l.focused("proj-a");
        assert_eq!(l.current(), Some(&"ctx-a2"));
    }
}
