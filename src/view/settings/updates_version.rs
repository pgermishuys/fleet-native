//! The line along the bottom of a machine's row in Updates & about › Machines (`MachineVersionLine.vue`): the version it runs,
//! where it is with updating, and the "Updates:" list that gives it a rule of its own.
use super::updates_strip::small_select;
use crate::State;
use crate::fleet::updates::{Kind, Rollout, Tone, UPDATE_RULES, UpdateRow, UpdateRule, default_rule_label, describe_version_line};
use crate::theme::{Theme, mono, sans};
use crate::ui::confirm::spinner;
use crate::ui::{Open, rem, select};
use rune::{Color, El, row, text};
use std::rc::Rc;

pub const RULE_SELECT: &str = "update-machine-rule";

fn ink(t: &Theme, tone: Tone) -> Color {
    match tone {
        Tone::Ok => t.running,
        Tone::Warn => t.idle,
        Tone::Info => t.complete,
        Tone::Bad => t.error,
        Tone::Muted => t.muted,
    }
}

pub fn view(s: &State, t: &Theme, r: &UpdateRow, rollout: Option<&Rollout>) -> El {
    let line = describe_version_line(r, rollout);
    let font = mono(11.5, 400.0, 1.5);
    let target = line.target.as_ref().map_or(String::new(), |to| format!(" \u{2192} {to}"));
    let spinning = matches!(r.state.kind, Kind::Downloading | Kind::Checking | Kind::Waiting | Kind::Restarting);
    let mut state = row().center_y().gap(4.0);
    if spinning {
        state = state.child(spinner(ink(t, line.tone)));
    }
    state = state.child(text(line.text.clone()).key(format!("machine-version-state-{}", r.key)).font(font).color(ink(t, line.tone)));
    let mut out = row()
        .key(format!("machine-version-{}", r.key))
        .w_full()
        .flex_wrap()
        .center_y()
        .gaps(4.0, 8.0)
        .mt(6.0)
        .child(text(format!("{}{target}", line.version)).key(format!("machine-version-text-{}", r.key)).font(font).color(t.muted))
        .child(text("\u{b7}").font(font).color(t.muted))
        .child(state);
    if let (true, Some(rule)) = (line.rule_menu, r.rule) {
        let open = s.updates.rule_open.as_deref() == Some(r.key.as_str());
        let chosen = rule.overridden;
        let shown = match chosen {
            Some(rule) => rule.info().short.to_string(),
            None => format!("Default ({})", default_rule_label(s.chosen_update_rule(), r.kind)),
        };
        let key = r.key.clone();
        let trigger = small_select(s, t, &format!("update-rule-{}", r.key), &format!("When {} updates", r.name), &shown, open).on_press(
            move |s: &mut State| {
                s.updates.rule_open = Some(key.clone());
                s.ui.open = Some(Open::Select(RULE_SELECT));
            },
        );
        out = out
            .child(rune::el().w(0.0).grow(1.0))
            .child(row().center_y().gap(5.25).child(text("Updates:").font(sans(rem(0.75), 400.0, 1.33)).color(t.muted)).child(trigger));
    }
    out
}

/// The open list of a machine's own rule: Default, then the three rules.
pub fn list(s: &State, t: &Theme) -> Vec<El> {
    let Some(key) = s.updates.rule_open.clone() else { return Vec::new() };
    let Some(r) = s.update_rows().into_iter().find(|r| r.key == key) else { return Vec::new() };
    let default = format!("Default ({})", default_rule_label(s.chosen_update_rule(), r.kind));
    let mut options: Vec<(&str, &str)> = vec![("default", default.as_str())];
    options.extend(UPDATE_RULES.iter().map(|u| (u.value.word(), u.short)));
    let current = r.rule.and_then(|c| c.overridden).map_or("default", UpdateRule::word);
    let anchor = format!("update-rule-{key}");
    let choose: select::Choose = Rc::new(move |s: &mut State, v: &str| {
        s.ui.open = None;
        s.updates.rule_open = None;
        s.override_update_rule(&key, UpdateRule::parse(v));
    });
    select::content(t, "update-machine-rule-list", &anchor, 220.0, &options, Some(current), choose)
}
