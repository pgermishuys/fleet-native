//! The Allowed chip's scenes (component 4, piece E): rules written as a server would send them (invented), read by the
//! same reader the live path uses, with the popover open or shut and the level Settings would give.
use crate::State;
use crate::asks::rules::read_rules;
use serde_json::{Value, json};

#[derive(Clone, Copy)]
pub enum Case {
    /// One rule, the chip only.
    Chip,
    /// One rule, the list open.
    One,
    /// Two rules: Remove all is there.
    Two,
    /// A rule with no pattern, tinted, beside one with patterns and a long one that wraps.
    Broad,
    /// The level allows everything: the rows are greyed and a note says why.
    Dormant,
}

fn rule(id: &str, kind: &str, tool: &str, patterns: &[&str], created: &str) -> Value {
    json!({"id": id, "kind": kind, "tool": tool, "patterns": patterns, "createdAt": created})
}

fn rules(case: Case) -> Value {
    // `super::failure::NOW` is 2026-10-07T01:51:43Z.
    let a = rule("rule_1", "shell", "bash", &["bun test*"], "2026-10-07T01:46:43Z");
    let b = rule("rule_2", "edit", "edit", &["src/*"], "2026-10-06T23:51:43Z");
    match case {
        Case::Chip | Case::One => json!([a]),
        Case::Two | Case::Dormant => json!([a, b]),
        Case::Broad => json!([
            rule("rule_1", "shell", "bash", &["*"], "2026-10-07T01:51:10Z"),
            rule(
                "rule_2",
                "shell",
                "bash",
                &["git log *", "git diff *", "cargo test --workspace --all-features *"],
                "2026-10-04T01:51:43Z"
            ),
            rule("rule_3", "web", "webfetch", &[], "2026-08-14T01:51:43Z"),
            rule("rule_4", "other", "fleet_session_start", &["taron"], "2026-10-06T21:51:43Z"),
        ]),
    }
}

impl State {
    /// The open session has `case`'s rules, at the level Settings gives; the list is open except for `Chip`.
    pub fn show_rules(&mut self, case: Case) {
        self.set_open((0, 0));
        let level = if matches!(case, Case::Dormant) { "all" } else { "ask" };
        self.pickers.prefs.set("PermissionLevel", level);
        self.rules.replace("mock.0.0", read_rules(&rules(case)));
        self.now_ms = Some(super::failure::NOW);
        self.ui.open = (!matches!(case, Case::Chip)).then_some(crate::ui::Open::Popover("allowed"));
    }
}
