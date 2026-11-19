//! Updates & about › Machines, the strip at the top (`MachineUpdatesStrip.vue`): the release line with the rule every machine
//! follows, Check now and Update machines now; or, while a rollout runs, where it is and how to stop it. Under it the rule's
//! hint, the outcome of the last rollout and the last refusal.
use super::updates::{link_in, note};
use crate::State;
use crate::fleet::updates::{
    Narrative, RULE_OPTIONS, ReleaseKind, ReleaseLine, Tone, UpdateRow, UpdateRule, describe_release_line, describe_rollout_narrative,
    describe_rollout_outcome, newest_checked_at, ready_machines, rule_hint, summarize_rows,
};
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, mono, sans};
use crate::ui::button::disabled;
use crate::ui::confirm::spinner;
use crate::ui::{Open, rem, select};
use rune::{Color, Cursor, El, Span, col, icon, rich, row, text};
use std::collections::HashSet;
use std::rc::Rc;

pub const RULE_SELECT: &str = "update-default-rule";

/// `buttonSecondaryClass`: `rounded-btn border border-border bg-card-bg px-3 py-1.5 text-sm font-medium`.
fn action(t: &Theme, key: &str, label: &str, lead: Option<El>, accent: bool) -> El {
    let ink = if accent { t.primary_fg } else { t.text };
    let mut b = row()
        .key(key.to_string())
        .role("button")
        .label(label.to_string())
        .center_y()
        .justify_center()
        .gap(7.0)
        .px(10.5)
        .py(5.25)
        .radius(RADIUS_BTN)
        .fill(if accent { t.accent } else { t.card_bg })
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 2.0, t.accent);
    if accent {
        b = b.hover(t.accent.alpha(0.9));
    } else {
        b = b.border(1.0, t.border).border_layout().hover(t.main_bg);
    }
    if let Some(lead) = lead {
        b = b.child(lead);
    }
    b.child(text(label.to_string()).font(sans(rem(0.875), 500.0, 1.43)).color(ink))
}

/// The pill the strip and the version line use for a select (`rounded-btn border border-border bg-card-bg px-2 py-1 text-xs`).
pub fn small_select(s: &State, t: &Theme, key: &str, label: &str, shown: &str, open: bool) -> El {
    let _ = s;
    row()
        .key(key.to_string())
        .role("combobox")
        .label(label.to_string())
        .center_y()
        .gap(5.0)
        .px(7.0)
        .py(3.5)
        .radius(RADIUS_BTN)
        .border(1.0, if open { t.accent } else { t.border })
        .border_layout()
        .fill(t.card_bg)
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 1.0, t.accent)
        .child(text(shown.to_string()).font(sans(rem(0.75), 400.0, 1.33)).color(t.text).truncate().shrink())
        .child(icon(&crate::icons::CHEVRON_DOWN, 10.0).color(t.muted))
}

fn outcome_ink(t: &Theme, tone: Tone) -> Color {
    match tone {
        Tone::Ok => t.running,
        Tone::Warn => t.idle,
        Tone::Info => t.complete,
        Tone::Bad => t.error,
        Tone::Muted => t.muted,
    }
}

/// A line of mono 12 in a bordered box: the outcome and the refusal (`mt-3 rounded-btn border px-3 py-2 font-mono text-[12px]`).
fn mono_box(key: &str, words: &str, ink: Color, edge: Color) -> El {
    col()
        .w_full()
        .mt(10.5)
        .px(10.5)
        .py(7.0)
        .radius(RADIUS_BTN)
        .border(1.0, edge)
        .border_layout()
        .child(text(words.to_string()).key(key.to_string()).font(mono(12.0, 400.0, 1.5)).color(ink).wrap().w_full())
}

/// The headline: the lead in bold, the rest muted when an update is out, with the "What's new" link after it.
fn headline(t: &Theme, release: &ReleaseLine, latest: Option<&str>) -> El {
    let available = release.kind == ReleaseKind::Available;
    let full = if release.lead.is_empty() { release.text.clone() } else { format!("{} {}", release.lead, release.text) };
    let mut spans = Vec::new();
    if !release.lead.is_empty() {
        spans.push(Span::new(0, release.lead.len()).weight(600.0).color(t.text));
    }
    if available {
        spans.push(Span::new(full.len() - release.text.len(), full.len()).color(t.muted));
    }
    let mut line = col().w(0.0).grow(1.0).min_w(256.0).child(
        rich(full, spans)
            .key("updates-headline")
            .font(sans(rem(0.875), 400.0, 1.43))
            .color(if available { t.text } else { t.muted })
            .wrap()
            .w_full(),
    );
    if let (true, Some(latest)) = (available, latest) {
        let version = latest.to_string();
        line = line.child(link_in(t, "updates-whats-new", "What's new", sans(rem(0.875), 400.0, 1.43), move |s: &mut State| {
            s.open_whats_new(Some(&version), None);
        }));
    }
    line
}

fn rolling(t: &Theme, narrative: &Narrative) -> Vec<El> {
    let full = format!("{} {}", narrative.title, narrative.text);
    let spans = vec![
        Span::new(0, narrative.title.len()).weight(600.0).color(t.text),
        Span::new(narrative.title.len() + 1, full.len()).color(t.muted),
    ];
    let stop = match &narrative.active {
        Some(name) => format!("Stop after {name}"),
        None => "Stop after this one".to_string(),
    };
    vec![
        row()
            .w(0.0)
            .grow(1.0)
            .min_w(256.0)
            .child(rich(full, spans).key("updates-narrative").font(sans(rem(0.875), 400.0, 1.43)).color(t.text).wrap().w_full()),
        action(t, "updates-stop-rollout", &stop, None, false).on_press(|s: &mut State| s.stop_rollout()),
    ]
}

pub fn view(s: &State, t: &Theme, rows: &[UpdateRow]) -> El {
    let summary = summarize_rows(rows);
    let checked = {
        let entries = s.machines.entries();
        newest_checked_at(entries.iter().map(|e| e.info.as_ref().and_then(|i| i.update.as_ref()).and_then(|u| u.checked_at.as_deref())))
    };
    let now = crate::view::message::now_ms(s);
    let release = describe_release_line(&summary, checked.as_deref(), now, crate::conv::time::local_offset_secs());
    let rollout = s.updates.rollout.as_ref();
    let running = rollout.is_some_and(|r| r.running());
    let unreachable: HashSet<String> =
        rows.iter().filter(|r| r.state.kind == crate::fleet::updates::Kind::Unreachable).map(|r| r.key.clone()).collect();
    let narrative = rollout.filter(|r| r.running()).map(|r| describe_rollout_narrative(r, &unreachable));
    let chosen = s.chosen_update_rule();
    let mut strip = row()
        .key("machines-updates-strip")
        .w_full()
        .flex_wrap()
        .center_y()
        .gaps(7.0, 14.0)
        .px(14.0)
        .py(10.5)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.main_bg);
    if let Some(n) = &narrative {
        for e in rolling(t, n) {
            strip = strip.child(e);
        }
    } else {
        strip = strip.child(headline(t, &release, summary.latest.as_deref()));
        if release.kind != ReleaseKind::None {
            let shown = RULE_OPTIONS.iter().find(|o| o.value == chosen).map_or("", |o| o.label);
            let open = s.ui.open == Some(Open::Select(RULE_SELECT));
            let trigger = small_select(s, t, RULE_SELECT, "When machines update", shown, open)
                .on_press(|s: &mut State| s.ui.open = Some(Open::Select(RULE_SELECT)));
            strip = strip.child(
                row().center_y().gap(5.25).child(text("Machines update:").font(sans(rem(0.75), 400.0, 1.33)).color(t.muted)).child(trigger),
            );
        }
        let checking = s.updates.checking;
        let lead = if checking { spinner(t.muted) } else { icon(&crate::icons::REFRESH_CW, 14.0).color(t.text) };
        let check = action(t, "updates-check-now", "Check now", Some(lead), false);
        let check = if checking { disabled(check) } else { check.on_press(|s: &mut State| s.check_updates_now()) };
        // `ml-auto`: right-aligned whether it shares the line or wraps onto its own.
        let mut buttons = row().grow(0.001).justify_end().flex_wrap().center_y().gap(7.0).child(check);
        let ready = ready_machines(rows);
        if release.kind == ReleaseKind::Available && ready > 0 {
            buttons = buttons.child(
                action(t, "updates-update-machines", &format!("Update machines now ({ready})"), None, true)
                    .on_press(|s: &mut State| s.start_rollout(Vec::new())),
            );
        }
        strip = strip.child(buttons);
    }
    let mut out = col().key("machines-updates").w_full().child(strip);
    if narrative.is_none() && release.kind != ReleaseKind::None {
        out = out.child(note(t, "updates-rule-hint", &rule_hint(chosen)).mt(7.0));
    }
    if let (Some((words, tone)), false) = (describe_rollout_outcome(rollout), running) {
        out = out.child(mono_box("updates-outcome", &words, outcome_ink(t, tone), outcome_edge(t, tone)));
    }
    if let Some(why) = &s.updates.action_error {
        out = out.child(mono_box("updates-error", why, t.error, t.error.alpha(0.4)));
    }
    out
}

fn outcome_edge(t: &Theme, tone: Tone) -> Color {
    if tone == Tone::Muted { t.border } else { outcome_ink(t, tone).alpha(0.4) }
}

/// The open list of the rule every machine follows.
pub fn rule_list(s: &State, t: &Theme) -> Vec<El> {
    let options: Vec<(&str, &str)> = RULE_OPTIONS.iter().map(|o| (o.value.map_or("", UpdateRule::word), o.label)).collect();
    let current = s.chosen_update_rule().map_or("", UpdateRule::word);
    let choose: select::Choose = Rc::new(|s: &mut State, v: &str| {
        s.ui.open = None;
        s.choose_update_rule(UpdateRule::parse(v));
    });
    select::content(t, "update-default-rule-list", RULE_SELECT, 300.0, &options, Some(current), choose)
}
