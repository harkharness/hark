//! Hark's directives, said in the agent's own vocabulary.
//!
//! A `session/new` answer offers what the agent can be told: permission
//! MODES (`modes.availableModes`) and CONFIG OPTIONS (`configOptions`,
//! categorised `model` / `thought_level`). Nothing here guesses beyond
//! that list — a directive with no offered counterpart is simply not
//! applied, and the capability sheet says so, so the pill is off instead
//! of pretending.
//!
//! Pure functions; the wire lives in `session.rs`.

use hark_core::domain::directives::{Effort, Mode};
use serde_json::Value;

/// One offered value: an id the agent understands and a label for people.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Named {
    pub id: String,
    pub name: String,
}

/// The agent's permission modes and the one in force.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Modes {
    pub current: String,
    pub available: Vec<Named>,
}

/// One `configOptions` selector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigOption {
    pub id: String,
    pub category: Option<String>,
    pub current: Option<String>,
    pub values: Vec<Named>,
}

/// Everything a `session/new` (or `session/load`) answer offers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Offer {
    pub modes: Option<Modes>,
    pub options: Vec<ConfigOption>,
}

fn s(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

/// Select values may come flat or in named groups; both flatten to ids.
fn flatten_values(items: &[Value], out: &mut Vec<Named>) {
    for item in items {
        if let Some(group) = item.get("options").and_then(Value::as_array) {
            flatten_values(group, out);
            continue;
        }
        if let Some(id) = s(item, "value") {
            let name = s(item, "name").unwrap_or_else(|| id.clone());
            out.push(Named { id, name });
        }
    }
}

impl Offer {
    /// Read the offer out of a `session/new` / `session/load` result.
    /// Anything malformed is treated as not offered.
    pub fn from_answer(answer: &Value) -> Offer {
        let modes = answer.get("modes").and_then(|m| {
            let current = s(m, "currentModeId")?;
            let available = m
                .get("availableModes")
                .and_then(Value::as_array)
                .map(|list| {
                    list.iter()
                        .filter_map(|mode| {
                            let id = s(mode, "id")?;
                            let name = s(mode, "name").unwrap_or_else(|| id.clone());
                            Some(Named { id, name })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            (!available.is_empty()).then_some(Modes { current, available })
        });
        let options = answer
            .get("configOptions")
            .and_then(Value::as_array)
            .map(|list| {
                list.iter()
                    .filter_map(|opt| {
                        let id = s(opt, "id")?;
                        let mut values = Vec::new();
                        if let Some(items) = opt.get("options").and_then(Value::as_array) {
                            flatten_values(items, &mut values);
                        }
                        Some(ConfigOption {
                            id,
                            category: s(opt, "category"),
                            current: opt.get("currentValue").and_then(|v| match v {
                                Value::String(x) => Some(x.clone()),
                                Value::Bool(b) => Some(b.to_string()),
                                _ => None,
                            }),
                            values,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        Offer { modes, options }
    }

    /// The selector that picks the model, if the agent has one.
    pub fn model_option(&self) -> Option<&ConfigOption> {
        self.options
            .iter()
            .find(|o| o.category.as_deref() == Some("model") || o.id.eq_ignore_ascii_case("model"))
    }

    /// The selector that picks the reasoning level, if the agent has one.
    pub fn effort_option(&self) -> Option<&ConfigOption> {
        self.options.iter().find(|o| {
            o.category.as_deref() == Some("thought_level")
                || ["effort", "reasoning_effort", "thinking_level", "thought_level"]
                    .iter()
                    .any(|id| o.id.eq_ignore_ascii_case(id))
        })
    }

    /// Which directives this agent can be told: (mode, model, effort).
    pub fn directive_caps(&self) -> (bool, bool, bool) {
        (self.modes.is_some(), self.model_option().is_some(), self.effort_option().is_some())
    }

    /// Apply a `configOptions` list the agent sent back (set_config_option
    /// answers with the full set; so does `config_option_update`).
    pub fn refresh_options(&mut self, list: &Value) {
        let fresh = Offer::from_answer(&serde_json::json!({ "configOptions": list }));
        if !fresh.options.is_empty() {
            self.options = fresh.options;
        }
    }
}

/// The agent's id for one of Hark's modes, if it offers a counterpart.
///
/// Exact ids first (the Claude adapter uses the CLI's own spellings), then
/// the aliases other agents use for the same idea. Escalation is never
/// implied: "auto" falls back to accepting edits, not to bypassing
/// everything — bypass is asked for by name or not at all.
pub fn mode_id(mode: Mode, modes: &Modes) -> Option<String> {
    let candidates: &[&str] = match mode {
        Mode::Manual => &["manual", "default", "ask"],
        Mode::AcceptEdits => &["acceptEdits", "autoEdit", "accept-edits", "auto-edit", "auto_edit"],
        Mode::Plan => &["plan"],
        Mode::Auto => &["auto", "acceptEdits", "autoEdit"],
        Mode::Bypass => &["bypassPermissions", "yolo", "bypass"],
    };
    candidates.iter().find_map(|want| {
        modes
            .available
            .iter()
            .find(|m| m.id.eq_ignore_ascii_case(want))
            .map(|m| m.id.clone())
    })
}

const EFFORT_RANK: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];

/// The offered value for one of Hark's effort levels: the same name when
/// offered, else the nearest LOWER level the agent has (a level it does
/// not know must not become a pricier one), else nothing.
pub fn effort_value(effort: Effort, option: &ConfigOption) -> Option<String> {
    let want = effort.as_flag();
    let rank = |name: &str| EFFORT_RANK.iter().position(|r| r.eq_ignore_ascii_case(name));
    let want_rank = rank(want)?;
    let mut best: Option<(usize, &Named)> = None;
    for value in &option.values {
        let Some(r) = rank(&value.id) else { continue };
        if r == want_rank {
            return Some(value.id.clone());
        }
        if r < want_rank && best.is_none_or(|(b, _)| r > b) {
            best = Some((r, value));
        }
    }
    best.map(|(_, v)| v.id.clone())
}

/// "opus[1m]" → "opus": the Claude adapter suffixes a context-window hint
/// onto its model values; it is not part of the name.
fn family(id: &str) -> String {
    let lower = id.trim().to_ascii_lowercase();
    match lower.find('[') {
        Some(at) => lower[..at].to_string(),
        None => lower,
    }
}

/// The offered value for a model the user or the router named. Recorded
/// from the real adapter: values are aliases and ids with context hints
/// ("haiku", "sonnet", "opus[1m]", "claude-fable-5-1[1m]") while Hark's
/// tiers are spelled like the CLI's ids ("claude-opus-5"). So: the same
/// name once hints are stripped, else the one whose family is inside the
/// asked name or the other way round ("opus" ⊂ "claude-opus-5"), else a
/// label match. "default" is never picked by containment.
pub fn model_value(model: &str, option: &ConfigOption) -> Option<String> {
    let want = family(model);
    if want.is_empty() {
        return None;
    }
    let exact = option
        .values
        .iter()
        .find(|v| family(&v.id) == want || v.name.eq_ignore_ascii_case(want.as_str()));
    if let Some(v) = exact {
        return Some(v.id.clone());
    }
    option
        .values
        .iter()
        .find(|v| {
            let id = family(&v.id);
            if id == "default" || id.is_empty() {
                return false;
            }
            id.contains(&want) || want.contains(&id) || v.name.to_ascii_lowercase().contains(&want)
        })
        .map(|v| v.id.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// What claude-agent-acp 0.76 answers to session/new, per its source
    /// (session-mode.ts, session-model.ts, session-effort.ts) — synthetic,
    /// not recorded: the adapter was not installed when this was written.
    fn claude_answer() -> Value {
        json!({
            "sessionId": "s-1",
            "modes": {
                "currentModeId": "default",
                "availableModes": [
                    { "id": "default", "name": "Always Ask" },
                    { "id": "acceptEdits", "name": "Accept Edits" },
                    { "id": "plan", "name": "Plan Mode" },
                    { "id": "bypassPermissions", "name": "Bypass Permissions" }
                ]
            },
            "configOptions": [
                { "id": "model", "name": "Model", "category": "model", "type": "select",
                  "currentValue": "default",
                  "options": [
                      { "value": "default", "name": "Default" },
                      { "value": "claude-opus-4-6", "name": "Opus 4.6" },
                      { "value": "claude-sonnet-4-5", "name": "Sonnet 4.5" },
                      { "value": "claude-haiku-4-5", "name": "Haiku 4.5" }
                  ] },
                { "id": "effort", "name": "Effort", "category": "thought_level", "type": "select",
                  "currentValue": "default",
                  "options": [
                      { "value": "default", "name": "Default" },
                      { "value": "low", "name": "low" },
                      { "value": "medium", "name": "medium" },
                      { "value": "high", "name": "high" },
                      { "value": "max", "name": "max" }
                  ] }
            ]
        })
    }

    /// Gemini CLI: approval modes only, no config options.
    fn gemini_answer() -> Value {
        json!({
            "sessionId": "s-2",
            "modes": {
                "currentModeId": "default",
                "availableModes": [
                    { "id": "default", "name": "Default" },
                    { "id": "autoEdit", "name": "Auto Edit" },
                    { "id": "yolo", "name": "YOLO" },
                    { "id": "plan", "name": "Plan" }
                ]
            }
        })
    }

    #[test]
    fn the_answer_says_which_directives_can_cross() {
        assert_eq!(Offer::from_answer(&claude_answer()).directive_caps(), (true, true, true));
        assert_eq!(Offer::from_answer(&gemini_answer()).directive_caps(), (true, false, false));
        assert_eq!(Offer::from_answer(&json!({ "sessionId": "s-3" })).directive_caps(), (false, false, false));
    }

    #[test]
    fn grouped_select_values_flatten_to_ids() {
        let offer = Offer::from_answer(&json!({ "configOptions": [
            { "id": "model", "category": "model", "type": "select", "currentValue": "a",
              "options": [
                  { "group": "fast", "name": "Fast", "options": [ { "value": "a", "name": "A" } ] },
                  { "value": "b", "name": "B" }
              ] }
        ] }));
        let ids: Vec<_> = offer.model_option().unwrap().values.iter().map(|v| v.id.as_str()).collect();
        assert_eq!(ids, vec!["a", "b"]);
    }

    #[test]
    fn modes_are_said_in_the_agents_own_ids() {
        let claude = Offer::from_answer(&claude_answer()).modes.unwrap();
        assert_eq!(mode_id(Mode::Manual, &claude).as_deref(), Some("default"));
        assert_eq!(mode_id(Mode::AcceptEdits, &claude).as_deref(), Some("acceptEdits"));
        assert_eq!(mode_id(Mode::Plan, &claude).as_deref(), Some("plan"));
        assert_eq!(mode_id(Mode::Bypass, &claude).as_deref(), Some("bypassPermissions"));
        // This adapter has no "auto" mode; auto means "accept edits", never
        // "bypass everything" — escalation is asked for by name.
        assert_eq!(mode_id(Mode::Auto, &claude).as_deref(), Some("acceptEdits"));

        let gemini = Offer::from_answer(&gemini_answer()).modes.unwrap();
        assert_eq!(mode_id(Mode::AcceptEdits, &gemini).as_deref(), Some("autoEdit"));
        assert_eq!(mode_id(Mode::Bypass, &gemini).as_deref(), Some("yolo"));
        assert_eq!(mode_id(Mode::Auto, &gemini).as_deref(), Some("autoEdit"));
    }

    #[test]
    fn a_mode_nobody_offers_is_not_applied() {
        let only_default = Modes { current: "default".into(), available: vec![Named { id: "default".into(), name: "d".into() }] };
        assert_eq!(mode_id(Mode::Plan, &only_default), None);
        assert_eq!(mode_id(Mode::Bypass, &only_default), None);
    }

    #[test]
    fn effort_takes_the_same_level_or_the_nearest_lower_one() {
        let offer = Offer::from_answer(&claude_answer());
        let effort = offer.effort_option().unwrap();
        assert_eq!(effort_value(Effort::Low, effort).as_deref(), Some("low"));
        assert_eq!(effort_value(Effort::Max, effort).as_deref(), Some("max"));
        // The adapter knows no "xhigh": the level below, never the one above.
        assert_eq!(effort_value(Effort::XHigh, effort).as_deref(), Some("high"));

        let nothing_ranked = ConfigOption {
            id: "effort".into(),
            category: Some("thought_level".into()),
            current: None,
            values: vec![Named { id: "default".into(), name: "Default".into() }],
        };
        assert_eq!(effort_value(Effort::High, &nothing_ranked), None);
    }

    #[test]
    fn a_model_is_found_by_id_by_label_or_by_family() {
        let offer = Offer::from_answer(&claude_answer());
        let model = offer.model_option().unwrap();
        assert_eq!(model_value("claude-haiku-4-5", model).as_deref(), Some("claude-haiku-4-5"));
        assert_eq!(model_value("Opus 4.6", model).as_deref(), Some("claude-opus-4-6"));
        assert_eq!(model_value("haiku", model).as_deref(), Some("claude-haiku-4-5"));
        assert_eq!(model_value("gpt-5", model), None);
        assert_eq!(model_value("", model), None);
    }

    /// Recorded from claude-agent-acp 0.76.0 on 14/09/2026 (HARK_ACP_TRACE):
    /// the real answer, auth notifications left out.
    const CLAUDE_NEW: &str = include_str!("../fixtures/session-new.claude-agent-acp-0.76.0.json");

    #[test]
    fn the_real_claude_adapter_offer_maps_harks_tiers_and_levels() {
        let offer = Offer::from_answer(&serde_json::from_str(CLAUDE_NEW).unwrap());
        assert_eq!(offer.directive_caps(), (true, true, true));
        let modes = offer.modes.as_ref().unwrap();
        // This adapter has a real "auto" mode (the CLI's), so auto is auto.
        assert_eq!(mode_id(Mode::Auto, modes).as_deref(), Some("auto"));
        assert_eq!(mode_id(Mode::Manual, modes).as_deref(), Some("default"));

        // Model values are aliases and ids WITH context hints: "opus[1m]",
        // "claude-fable-5-1[1m]", "sonnet", "haiku". Hark's tiers are
        // spelled like the CLI's model ids; the family has to carry.
        let model = offer.model_option().unwrap();
        assert_eq!(model_value("haiku", model).as_deref(), Some("haiku"));
        assert_eq!(model_value("claude-haiku-4-5", model).as_deref(), Some("haiku"));
        assert_eq!(model_value("claude-opus-5", model).as_deref(), Some("opus[1m]"));
        assert_eq!(model_value("claude-fable-5-1", model).as_deref(), Some("claude-fable-5-1[1m]"));
        assert_eq!(model_value("claude-sonnet-4-5[1m]", model).as_deref(), Some("sonnet"));

        let effort = offer.effort_option().unwrap();
        assert_eq!(effort_value(Effort::XHigh, effort).as_deref(), Some("xhigh"), "offered, so taken as is");
    }

    #[test]
    fn a_set_config_option_answer_refreshes_the_current_values() {
        let mut offer = Offer::from_answer(&claude_answer());
        offer.refresh_options(&json!([
            { "id": "effort", "category": "thought_level", "type": "select", "currentValue": "low",
              "options": [ { "value": "low", "name": "low" } ] }
        ]));
        assert_eq!(offer.effort_option().unwrap().current.as_deref(), Some("low"));
    }
}
