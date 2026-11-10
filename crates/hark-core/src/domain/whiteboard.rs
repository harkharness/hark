//! A whiteboard page: notes on a canvas, groups around them, a timeline.
//!
//! The file the agent writes — coordinates are optional, so a weak model
//! can leave them out and Hark lays the notes out:
//!
//! ```json
//! { "title": "Plano da release 0.11",
//!   "groups": [ { "id": "g1", "title": "Quick-add" } ],
//!   "notes":  [ { "id": "n1", "text": "Atalho no celular", "group": "g1", "owner": "Ana", "date": "2026-11-12", "color": "yellow" },
//!               { "id": "n2", "text": "Dúvida: entra na 0.11?", "color": "pink" } ],
//!   "timeline": [ { "date": "2026-11-12", "title": "quick-add beta", "owner": "Ana" } ] }
//! ```
//!
//! Pure: the layout, the edits the person makes by dragging and typing,
//! the notes that changed since a baseline, and the text a selection
//! becomes when it goes to a chat.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Board {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default)]
    pub groups: Vec<Group>,
    #[serde(default)]
    pub notes: Vec<Note>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub timeline: Vec<Milestone>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Group {
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub w: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Note {
    pub id: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Milestone {
    pub date: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
}

/// The colours a note may wear; anything else becomes yellow.
pub const COLORS: [&str; 5] = ["yellow", "blue", "green", "pink", "gray"];

/// A note's footprint and the gaps the layout keeps.
pub const NOTE_W: f64 = 150.0;
pub const NOTE_H: f64 = 96.0;
const GAP: f64 = 14.0;
const GROUP_PAD: f64 = 24.0;
const PER_ROW: usize = 2;

/// The file as a board, or the error the agent needs to see.
pub fn parse(json: &str) -> Result<Board, String> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| format!("não é json: {e}"))?;
    if !v.is_object() {
        return Err("o quadro é um objeto com \"notes\"".into());
    }
    let board: Board = serde_json::from_value(v).map_err(|e| format!("quadro inválido: {e}"))?;
    let mut seen = std::collections::HashSet::new();
    for n in &board.notes {
        if n.id.trim().is_empty() {
            return Err("nota sem \"id\"".into());
        }
        if !seen.insert(n.id.as_str()) {
            return Err(format!("nota {}: id repetido", n.id));
        }
        if let Some(g) = &n.group {
            if !board.groups.iter().any(|x| &x.id == g) {
                return Err(format!("nota {}: grupo \"{g}\" não existe", n.id));
            }
        }
    }
    let mut seen = std::collections::HashSet::new();
    for g in &board.groups {
        if g.id.trim().is_empty() {
            return Err("grupo sem \"id\"".into());
        }
        if !seen.insert(g.id.as_str()) {
            return Err(format!("grupo {}: id repetido", g.id));
        }
    }
    Ok(board)
}

/// Every note and group given a place when the file has none: groups
/// side by side, each wide enough for two notes per row; notes without
/// a group in a lane under them. A note or group that has coordinates
/// keeps them. Idempotent.
pub fn lay_out(board: &Board) -> Board {
    let mut out = board.clone();
    let mut cursor_x = GROUP_PAD;
    let group_w = GROUP_PAD * 2.0 + NOTE_W * PER_ROW as f64 + GAP * (PER_ROW as f64 - 1.0);
    let mut lane_y = GROUP_PAD;
    for g in &mut out.groups {
        let members: Vec<usize> = board.notes.iter().enumerate().filter(|(_, n)| n.group.as_deref() == Some(&g.id)).map(|(i, _)| i).collect();
        let rows = members.len().div_ceil(PER_ROW).max(1);
        let h = GROUP_PAD * 2.0 + NOTE_H * rows as f64 + GAP * (rows as f64 - 1.0) + 12.0;
        let gx = g.x.unwrap_or(cursor_x);
        let gy = g.y.unwrap_or(GROUP_PAD);
        let gw = g.w.unwrap_or(group_w);
        let gh = g.h.unwrap_or(h);
        *g = Group { x: Some(gx), y: Some(gy), w: Some(gw), h: Some(gh), ..g.clone() };
        for (k, &i) in members.iter().enumerate() {
            let n = &mut out.notes[i];
            if n.x.is_none() || n.y.is_none() {
                n.x = Some(gx + GROUP_PAD + (k % PER_ROW) as f64 * (NOTE_W + GAP));
                n.y = Some(gy + GROUP_PAD + 12.0 + (k / PER_ROW) as f64 * (NOTE_H + GAP));
            }
        }
        cursor_x = gx.max(cursor_x) + gw + GAP * 2.0;
        lane_y = lane_y.max(gy + gh + GAP * 2.0);
    }
    let mut k = 0;
    for n in &mut out.notes {
        if n.group.is_none() && (n.x.is_none() || n.y.is_none()) {
            n.x = Some(GROUP_PAD + (k % 5) as f64 * (NOTE_W + GAP));
            n.y = Some(lane_y + (k / 5) as f64 * (NOTE_H + GAP));
            k += 1;
        }
    }
    for n in &mut out.notes {
        if !n.color.as_deref().is_some_and(|c| COLORS.contains(&c)) {
            n.color = Some("yellow".into());
        }
    }
    out.timeline.sort_by(|a, b| a.date.cmp(&b.date));
    out
}

/// The notes new or changed (text, group, owner, date) since `before`;
/// a move alone is not a change worth marking.
pub fn changed(before: &Board, after: &Board) -> Vec<String> {
    after
        .notes
        .iter()
        .filter(|n| {
            before
                .notes
                .iter()
                .find(|b| b.id == n.id)
                .map(|b| b.text != n.text || b.group != n.group || b.owner != n.owner || b.date != n.date)
                .unwrap_or(true)
        })
        .map(|n| n.id.clone())
        .collect()
}

/// What the person does on the canvas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Op {
    Move { id: String, x: f64, y: f64 },
    Text { id: String, text: String },
    Add { text: String, x: f64, y: f64, #[serde(default)] group: Option<String>, #[serde(default)] color: Option<String> },
    Remove { id: String },
    Color { id: String, color: String },
    Group { id: String, group: Option<String> },
    MoveGroup { id: String, x: f64, y: f64 },
    AddGroup { title: String, x: f64, y: f64 },
    RemoveGroup { id: String },
}

/// A fresh id for a note or a group: n1, n2… not yet taken.
pub fn fresh_id(board: &Board, prefix: &str) -> String {
    let taken = |id: &str| board.notes.iter().any(|n| n.id == id) || board.groups.iter().any(|g| g.id == id);
    (1..).map(|i| format!("{prefix}{i}")).find(|id| !taken(id)).unwrap_or_else(|| format!("{prefix}0"))
}

/// The board after the person's edit; an id that is not there is an error.
pub fn apply(board: &mut Board, op: Op) -> Result<(), String> {
    fn note<'a>(b: &'a mut Board, id: &str) -> Result<&'a mut Note, String> {
        b.notes.iter_mut().find(|n| n.id == id).ok_or_else(|| format!("nota \"{id}\" não existe"))
    }
    match op {
        Op::Move { id, x, y } => {
            let n = note(board, &id)?;
            n.x = Some(x);
            n.y = Some(y);
            // Dropped inside a group's frame: it joins; outside every frame: it leaves.
            let hit = board.groups.iter().find(|g| inside(g, x + NOTE_W / 2.0, y + NOTE_H / 2.0)).map(|g| g.id.clone());
            board.notes.iter_mut().find(|n| n.id == id).unwrap().group = hit;
        }
        Op::Text { id, text } => note(board, &id)?.text = text,
        Op::Add { text, x, y, group, color } => {
            if let Some(g) = &group {
                if !board.groups.iter().any(|x| &x.id == g) {
                    return Err(format!("grupo \"{g}\" não existe"));
                }
            }
            let id = fresh_id(board, "n");
            board.notes.push(Note { id, text, x: Some(x), y: Some(y), color: color.or(Some("yellow".into())), owner: None, date: None, group });
        }
        Op::Remove { id } => {
            let before = board.notes.len();
            board.notes.retain(|n| n.id != id);
            if board.notes.len() == before {
                return Err(format!("nota \"{id}\" não existe"));
            }
        }
        Op::Color { id, color } => {
            if !COLORS.contains(&color.as_str()) {
                return Err(format!("cor \"{color}\": use {}", COLORS.join(", ")));
            }
            note(board, &id)?.color = Some(color);
        }
        Op::Group { id, group } => {
            if let Some(g) = &group {
                if !board.groups.iter().any(|x| &x.id == g) {
                    return Err(format!("grupo \"{g}\" não existe"));
                }
            }
            note(board, &id)?.group = group;
        }
        Op::MoveGroup { id, x, y } => {
            let g = board.groups.iter_mut().find(|g| g.id == id).ok_or_else(|| format!("grupo \"{id}\" não existe"))?;
            let (dx, dy) = (x - g.x.unwrap_or(0.0), y - g.y.unwrap_or(0.0));
            g.x = Some(x);
            g.y = Some(y);
            for n in board.notes.iter_mut().filter(|n| n.group.as_deref() == Some(&id)) {
                n.x = Some(n.x.unwrap_or(0.0) + dx);
                n.y = Some(n.y.unwrap_or(0.0) + dy);
            }
        }
        Op::AddGroup { title, x, y } => {
            let id = fresh_id(board, "g");
            board.groups.push(Group { id, title, x: Some(x), y: Some(y), w: None, h: None });
        }
        Op::RemoveGroup { id } => {
            let before = board.groups.len();
            board.groups.retain(|g| g.id != id);
            if board.groups.len() == before {
                return Err(format!("grupo \"{id}\" não existe"));
            }
            for n in board.notes.iter_mut().filter(|n| n.group.as_deref() == Some(&id)) {
                n.group = None;
            }
        }
    }
    Ok(())
}

fn inside(g: &Group, x: f64, y: f64) -> bool {
    let (gx, gy, gw, gh) = (g.x.unwrap_or(0.0), g.y.unwrap_or(0.0), g.w.unwrap_or(0.0), g.h.unwrap_or(0.0));
    x >= gx && x <= gx + gw && y >= gy && y <= gy + gh
}

/// The notes whose frame lies inside a rectangle: what a lasso catches.
pub fn within(board: &Board, x1: f64, y1: f64, x2: f64, y2: f64) -> Vec<String> {
    let (l, r) = (x1.min(x2), x1.max(x2));
    let (t, b) = (y1.min(y2), y1.max(y2));
    board
        .notes
        .iter()
        .filter(|n| {
            let (x, y) = (n.x.unwrap_or(0.0), n.y.unwrap_or(0.0));
            x >= l && x + NOTE_W <= r && y >= t && y + NOTE_H <= b
        })
        .map(|n| n.id.clone())
        .collect()
}

/// The selected notes as the text a chat receives: one line each, the
/// group in brackets, the owner and date after. Nothing else of the
/// board goes along — only the excerpt enters the chat.
pub fn selection_text(board: &Board, ids: &[String]) -> String {
    let mut out = String::new();
    for n in board.notes.iter().filter(|n| ids.contains(&n.id)) {
        let group = n.group.as_ref().and_then(|g| board.groups.iter().find(|x| &x.id == g)).map(|g| format!("[{}] ", g.title)).unwrap_or_default();
        let who = match (&n.owner, &n.date) {
            (Some(o), Some(d)) => format!(" ({o}, {d})"),
            (Some(o), None) => format!(" ({o})"),
            (None, Some(d)) => format!(" ({d})"),
            (None, None) => String::new(),
        };
        out.push_str(&format!("- {group}{}{who}\n", n.text.trim()));
    }
    out
}

/// The whole board as text, for a chat that cites the page: groups with
/// their notes, loose notes, the timeline. Short, never the json.
pub fn summary(board: &Board) -> String {
    let mut out = String::new();
    if let Some(t) = &board.title {
        out.push_str(&format!("# {t}\n\n"));
    }
    for g in &board.groups {
        out.push_str(&format!("## {}\n", g.title));
        let ids: Vec<String> = board.notes.iter().filter(|n| n.group.as_deref() == Some(&g.id)).map(|n| n.id.clone()).collect();
        out.push_str(&selection_text(&Board { groups: vec![], ..board.clone() }, &ids));
        out.push('\n');
    }
    let loose: Vec<String> = board.notes.iter().filter(|n| n.group.is_none()).map(|n| n.id.clone()).collect();
    if !loose.is_empty() {
        out.push_str("## Soltas\n");
        out.push_str(&selection_text(board, &loose));
        out.push('\n');
    }
    if !board.timeline.is_empty() {
        out.push_str("## Linha do tempo\n");
        for m in &board.timeline {
            out.push_str(&format!("- {} · {}{}\n", m.date, m.title, m.owner.as_ref().map(|o| format!(" ({o})")).unwrap_or_default()));
        }
    }
    out.trim_end().to_string()
}

pub fn to_json(board: &Board) -> String {
    let mut s = serde_json::to_string_pretty(board).unwrap_or_else(|_| "{}".into());
    s.push('\n');
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{ "title": "Plano",
      "groups": [ {"id": "g1", "title": "Quick-add"}, {"id": "g2", "title": "Relatórios"} ],
      "notes": [ {"id": "n1", "text": "Atalho no celular", "group": "g1", "owner": "Ana", "date": "2026-11-12"},
                 {"id": "n2", "text": "Voz local", "group": "g1"},
                 {"id": "n3", "text": "Sem internet", "group": "g1", "color": "green"},
                 {"id": "n4", "text": "Gastos por categoria", "group": "g2", "color": "blue"},
                 {"id": "n5", "text": "Entra na 0.11?", "color": "pink", "owner": "Bruno"} ],
      "timeline": [ {"date": "2026-12-03", "title": "release"}, {"date": "2026-11-12", "title": "beta", "owner": "Ana"} ] }"#;

    #[test]
    fn a_board_is_parsed_and_its_loose_ends_are_named() {
        let b = parse(SAMPLE).unwrap();
        assert_eq!(b.notes.len(), 5);
        assert!(parse("nope").unwrap_err().contains("json"));
        assert!(parse(r#"{"notes":[{"id":"a","text":"x"},{"id":"a","text":"y"}]}"#).unwrap_err().contains("repetido"));
        assert!(parse(r#"{"notes":[{"id":"a","text":"x","group":"zz"}]}"#).unwrap_err().contains("zz"));
        assert!(parse(r#"{"notes":[{"id":"","text":"x"}]}"#).unwrap_err().contains("id"));
        assert_eq!(parse("{}").unwrap().notes.len(), 0, "an empty board is a board");
    }

    #[test]
    fn notes_without_a_place_are_laid_out_in_their_groups_and_colours_default() {
        let b = lay_out(&parse(SAMPLE).unwrap());
        let g1 = &b.groups[0];
        let g2 = &b.groups[1];
        assert!(g1.x.unwrap() < g2.x.unwrap(), "groups side by side");
        assert!(g1.h.unwrap() > g2.h.unwrap(), "three notes take two rows");
        let n1 = &b.notes[0];
        let n2 = &b.notes[1];
        let n3 = &b.notes[2];
        assert!(n1.x.unwrap() >= g1.x.unwrap() && n1.x.unwrap() + NOTE_W <= g1.x.unwrap() + g1.w.unwrap(), "inside its group");
        assert_eq!(n1.y, n2.y, "two per row");
        assert!(n3.y.unwrap() > n1.y.unwrap(), "the third goes under");
        assert!(b.notes[4].y.unwrap() > g1.y.unwrap() + g1.h.unwrap(), "a loose note sits under the groups");
        assert_eq!(n1.color.as_deref(), Some("yellow"));
        assert_eq!(b.notes[2].color.as_deref(), Some("green"));
        assert_eq!(b.timeline[0].date, "2026-11-12", "the timeline is sorted");
        assert_eq!(lay_out(&b), b, "idempotent");
        // A note with its own place keeps it.
        let mut placed = parse(SAMPLE).unwrap();
        placed.notes[4].x = Some(900.0);
        placed.notes[4].y = Some(10.0);
        assert_eq!(lay_out(&placed).notes[4].x, Some(900.0));
    }

    #[test]
    fn the_notes_the_agent_changed_are_named_and_a_move_is_not_a_change() {
        let before = lay_out(&parse(SAMPLE).unwrap());
        let mut after = before.clone();
        apply(&mut after, Op::Text { id: "n2".into(), text: "Voz local (STT)".into() }).unwrap();
        apply(&mut after, Op::Add { text: "Nova".into(), x: 10.0, y: 10.0, group: None, color: None }).unwrap();
        apply(&mut after, Op::Move { id: "n5".into(), x: 500.0, y: 900.0 }).unwrap();
        assert_eq!(changed(&before, &after), vec!["n2", "n6"]);
        assert!(changed(&before, &before).is_empty());
    }

    #[test]
    fn dragging_a_note_into_a_group_joins_it_and_the_other_edits_hold() {
        let mut b = lay_out(&parse(SAMPLE).unwrap());
        let g2 = b.groups[1].clone();
        apply(&mut b, Op::Move { id: "n5".into(), x: g2.x.unwrap() + 10.0, y: g2.y.unwrap() + 30.0 }).unwrap();
        assert_eq!(b.notes[4].group.as_deref(), Some("g2"));
        apply(&mut b, Op::Move { id: "n5".into(), x: 2000.0, y: 2000.0 }).unwrap();
        assert_eq!(b.notes[4].group, None, "dropped outside every frame, it leaves");
        apply(&mut b, Op::Color { id: "n5".into(), color: "gray".into() }).unwrap();
        assert!(apply(&mut b, Op::Color { id: "n5".into(), color: "magenta".into() }).is_err());
        apply(&mut b, Op::AddGroup { title: "Dúvidas".into(), x: 700.0, y: 20.0 }).unwrap();
        assert_eq!(b.groups[2].id, "g3");
        apply(&mut b, Op::Group { id: "n5".into(), group: Some("g3".into()) }).unwrap();
        assert!(apply(&mut b, Op::Group { id: "n5".into(), group: Some("zz".into()) }).is_err());
        let before = b.notes[0].x.unwrap();
        apply(&mut b, Op::MoveGroup { id: "g1".into(), x: 100.0, y: 100.0 }).unwrap();
        assert!((b.notes[0].x.unwrap() - before - (100.0 - GROUP_PAD)).abs() < 1e-6, "the notes move with their group");
        apply(&mut b, Op::RemoveGroup { id: "g3".into() }).unwrap();
        assert_eq!(b.notes[4].group, None, "its notes stay, loose");
        apply(&mut b, Op::Remove { id: "n5".into() }).unwrap();
        assert!(apply(&mut b, Op::Remove { id: "n5".into() }).is_err());
        assert_eq!(parse(&to_json(&b)).unwrap(), b, "round-trips through the file");
    }

    #[test]
    fn a_lasso_catches_the_notes_inside_it_and_the_selection_becomes_text() {
        let b = lay_out(&parse(SAMPLE).unwrap());
        let g1 = &b.groups[0];
        let caught = within(&b, g1.x.unwrap(), g1.y.unwrap(), g1.x.unwrap() + g1.w.unwrap(), g1.y.unwrap() + g1.h.unwrap());
        assert_eq!(caught, vec!["n1", "n2", "n3"]);
        let text = selection_text(&b, &caught);
        assert_eq!(text, "- [Quick-add] Atalho no celular (Ana, 2026-11-12)\n- [Quick-add] Voz local\n- [Quick-add] Sem internet\n");
        assert!(within(&b, 0.0, 0.0, 10.0, 10.0).is_empty());
    }

    #[test]
    fn the_summary_reads_as_a_short_document_not_as_json() {
        let s = summary(&lay_out(&parse(SAMPLE).unwrap()));
        assert!(s.starts_with("# Plano\n\n## Quick-add\n- Atalho no celular (Ana, 2026-11-12)\n"), "{s}");
        assert!(s.contains("## Soltas\n- Entra na 0.11? (Bruno)"), "{s}");
        assert!(s.contains("## Linha do tempo\n- 2026-11-12 · beta (Ana)\n- 2026-12-03 · release"), "{s}");
        assert!(!s.contains('{'));
    }
}
