//! Settings → Harnesses → Weave (`HarnessWeaveGroup.vue`): one harness's row of which Weave it loads (with the pill, Check again,
//! Add Weave and Remove), the notices under it, and the Config row or the empty state. The answer is the page's, shared by
//! every tab. Sizes are Fleet's Tailwind classes at the 14px root.
use super::kit::{self, Row};
use crate::State;
use crate::fleet::settings::harness_weave::{
    Detection, Flavor, Group, NoticeTone, Tone, added_by_fleet, both_loaded, describe_install, loaded_flavors, outdated_flavors, pill,
};
use crate::icons::*;
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, sans};
use crate::ui::button::{self, Size, Variant};
use crate::ui::{confirm::spinner, rem};
use rune::{Align, Color, Cursor, El, Span, Tx, col, el, rich, row, text};

const TITLE: &str = "Weave";
const HINT: &str = "Agents, prompts and workflows Weave adds.";
pub const DOCS: &str = "https://tryweave.io/docs/quickstart/";

fn small(key: &str, words: &str, ink: Color) -> El {
    row()
        .mt(7.0)
        .child(text(words.to_string()).key(key.to_string()).font(sans(rem(0.75), 400.0, 1.33)).color(ink).wrap().shrink().min_w(0.0))
}

/// The state pill (`rounded-full border px-2 py-0.5 text-[11px] font-medium`, a `size-1.5` dot).
fn state_pill(t: &Theme, d: &Detection) -> El {
    let (tone, label) = pill(d);
    let (edge, fill, ink) = match tone {
        Tone::Ok => (t.running.alpha(0.35), t.running.alpha(0.1), t.running_text),
        Tone::Warn => (t.idle.alpha(0.35), t.idle.alpha(0.1), t.idle_text),
        Tone::Off => (t.border, Color::TRANSPARENT, t.muted),
    };
    row()
        .key("weave-pill")
        .center_y()
        .gap(5.25)
        .px(7.0)
        .py(1.75)
        .radius(999.0)
        .fill(fill)
        .border(1.0, edge)
        .border_layout()
        .child(el().size(5.25, 5.25).radius(3.0).fill(ink))
        .child(text(label).font(sans(11.0, 500.0, 1.5)).color(ink))
}

/// The box under the row: `rounded-btn border border-idle/35 bg-idle/10 px-3 py-2 text-xs`, a bold lead and plain words, with
/// an optional path in mono between `before` and `after`.
fn warning(t: &Theme, key: &str, lead: &str, before: &str, mono: Option<&str>, after: &str) -> El {
    let mut words = lead.to_string();
    let mut spans = vec![Span::new(0, lead.len()).weight(600.0).color(t.idle_text)];
    words.push_str(before);
    if let Some(path) = mono {
        let at = words.len();
        words.push_str(path);
        spans.push(Span::new(at, words.len()).mono());
    }
    words.push_str(after);
    row()
        .key(key.to_string())
        .mt(7.0)
        .px(10.5)
        .py(7.0)
        .radius(RADIUS_BTN)
        .fill(t.idle.alpha(0.1))
        .border(1.0, t.idle.alpha(0.35))
        .border_layout()
        .child(rich(words, spans).font(sans(rem(0.75), 400.0, 1.33)).color(t.text).wrap().shrink().min_w(0.0))
}

fn outdated(t: &Theme, flavor: Flavor) -> El {
    let lead = format!("Update {} to {} to use a config kept in Fleet.", flavor.name(), flavor.minimum());
    let key = format!("weave-outdated-{}", if flavor == Flavor::Weave { "weave" } else { "legacy" });
    warning(
        t,
        &key,
        &lead,
        " This version only reads ",
        Some(flavor.own_path()),
        ". Update it in your opencode.json plugin list, then check again.",
    )
}

/// The buttons: Check again, then Add Weave or Remove.
fn buttons(s: &State, t: &Theme, d: &Detection) -> El {
    let w = &s.settings.harnesses.weave;
    let locked = w.changing.is_some();
    let busy = w.changing.as_deref() == Some(d.harness_type.as_str());
    let again = if w.loading {
        button::button_lead(t, Variant::Ghost, Size::Sm, Some(spinner(t.muted)), "Check again")
    } else {
        button::button(t, Variant::Ghost, Size::Sm, None, "Check again")
    }
    .key("weave-recheck");
    let again = if w.loading || locked { button::disabled(again) } else { again.on_press(|s: &mut State| s.check_weave_again()) };
    let mut r = row().center_y().gap(7.0).child(again);
    let kind = d.harness_type.clone();
    if d.add_to.is_some() {
        let b = if busy {
            button::button_lead(t, Variant::Outline, Size::Sm, Some(spinner(t.muted)), "Adding\u{2026}")
        } else {
            button::button(t, Variant::Outline, Size::Sm, Some(&PLUS), "Add Weave")
        }
        .key(format!("weave-add-{kind}"));
        r = r.child(if locked { button::disabled(b) } else { b.on_press(move |s: &mut State| s.change_weave_plugin(&kind, true)) });
    } else if added_by_fleet(d) {
        let b = if busy {
            button::button_lead(t, Variant::Ghost, Size::Sm, Some(spinner(t.muted)), "Removing\u{2026}")
        } else {
            button::button(t, Variant::Ghost, Size::Sm, None, "Remove")
        }
        .key(format!("weave-remove-{kind}"));
        r = r.child(if locked { button::disabled(b) } else { b.on_press(move |s: &mut State| s.change_weave_plugin(&kind, false)) });
    }
    r
}

/// What is under the plugin row: the failed re-ask, the busy line or the last notice, the old-version boxes.
fn below(s: &State, t: &Theme, d: &Detection) -> El {
    let w = &s.settings.harnesses.weave;
    let mut b = col().w_full();
    if let Some(why) = &w.error {
        b = b.child(kit::error_note(t, "weave-error", &format!("Couldn't ask the harnesses again: {why}")));
    }
    if w.changing.as_deref() == Some(d.harness_type.as_str()) {
        b = b.child(small("weave-plugin-busy", "The first time, the harness downloads Weave from npm, which can take a minute.", t.muted));
    } else if let Some(n) = w.notices.get(&d.harness_type) {
        let (key, ink) = match n.tone {
            NoticeTone::Ok => ("weave-plugin-notice", t.muted),
            NoticeTone::Warn => ("weave-plugin-notice", t.idle_text),
            NoticeTone::Error => ("weave-plugin-notice", t.error_text),
        };
        let mut line = text(n.text.clone()).key(key).font(sans(rem(0.75), 400.0, 1.33)).color(ink).wrap().shrink().min_w(0.0);
        if n.tone == NoticeTone::Error {
            line = line.role("alert");
        }
        b = b.child(row().mt(7.0).child(line));
    }
    for flavor in outdated_flavors(std::slice::from_ref(d)) {
        b = b.child(outdated(t, flavor));
    }
    if both_loaded(d) {
        let both =
            "This harness loads both Weave and Weave Legacy, and both add agents such as Loom. Keep one of them in your plugin list.";
        b = b.child(
            row()
                .key("weave-both")
                .mt(7.0)
                .px(10.5)
                .py(7.0)
                .radius(RADIUS_BTN)
                .fill(t.idle.alpha(0.1))
                .border(1.0, t.idle.alpha(0.35))
                .border_layout()
                .child(text(both).font(sans(rem(0.75), 400.0, 1.33)).color(t.text).wrap().shrink().min_w(0.0)),
        );
    }
    b
}

fn plugin_row(s: &State, t: &Theme, d: &Detection) -> El {
    Row::new("weave-plugin", "Weave plugin")
        .summary(&describe_install(d))
        .badge(state_pill(t, d))
        .control(buttons(s, t, d))
        .below(below(s, t, d))
        .build(s, t)
}

/// `EmptyState bordered` (Fleet's border is dashed; Rune has no dashed line, so this one is solid).
fn none(s: &State, t: &Theme, all: &[Detection]) -> El {
    let _ = s;
    let mut words = "Nothing here changes how Fleet works. Sessions use the harness's own agents. Once Weave is in a harness's plugin list, you can set its config here.".to_string();
    if all.iter().any(|d| d.add_to.is_some()) {
        words.push_str(" Add Weave above puts it there for you.");
    }
    let link = row()
        .key("weave-docs")
        .role("link")
        .label("What Weave adds")
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 0.0, t.accent)
        .child(text("What Weave adds").font(sans(rem(0.875), 400.0, 1.43)).color(t.accent_text))
        .on_press_tx(|_: &mut State, tx: &mut Tx| {
            let _ = tx.open_url(DOCS);
        });
    row().w_full().px(14.0).py(10.5).child(
        col()
            .key("weave-none")
            .w_full()
            .items_center()
            .gap(4.0)
            .px(21.0)
            .py(21.0)
            .radius(RADIUS_CARD)
            .border(1.0, t.border)
            .border_layout()
            .child(text("Weave isn't running in any harness").font(sans(rem(1.0), 600.0, 1.5)).color(t.text))
            .child(text(words).font(sans(rem(0.875), 400.0, 1.43)).color(t.muted).align(Align::Center).wrap().max_w(560.0))
            .child(row().mt(10.5).child(link)),
    )
}

/// The Weave group of the shown harness's tab; none when Fleet answered and has nothing for this harness.
pub fn group(s: &State, t: &Theme, harness: &str) -> Option<El> {
    let w = &s.settings.harnesses.weave;
    match w.group(harness) {
        Group::Absent => None,
        Group::Asking => {
            // Until Fleet answers the group holds its place; a link to `weave` waits for the answer, so this isn't its target.
            let asking =
                row().key("weave-asking").role("status").w_full().center_y().gap(7.0).px(14.0).py(10.5).child(spinner(t.muted)).child(
                    text("Asking your harnesses which plugins they load\u{2026}").font(sans(rem(0.875), 400.0, 1.43)).color(t.muted),
                );
            Some(kit::group(t, "weave-loading", TITLE, Some(HINT), vec![asking]))
        }
        Group::Failed(why) => {
            let again = button::button(t, Variant::Outline, Size::Sm, None, "Check again").key("weave-recheck");
            let again = if w.loading { button::disabled(again) } else { again.on_press(|s: &mut State| s.check_weave_again()) };
            let r = Row::new("weave-error", "Couldn't ask the harnesses about Weave").summary(&why).control(again).build(s, t);
            Some(kit::group(t, "weave", TITLE, Some(HINT), vec![r]))
        }
        Group::Detected(d) => {
            let mut rows = vec![plugin_row(s, t, &d)];
            let all = w.view.as_ref().map(|v| v.harnesses.clone()).unwrap_or_default();
            if loaded_flavors(&all).is_empty() {
                rows.push(none(s, t, &all));
            } else {
                let words = if w.view.as_ref().is_some_and(|v| v.fleet_source) {
                    "Kept in Fleet. Weave reads it in Fleet sessions."
                } else {
                    "Weave reads your own file in Fleet sessions."
                };
                let edit = button::button(t, Variant::Outline, Size::Sm, None, "Edit\u{2026}")
                    .key("weave-edit-config")
                    .on_press(|s: &mut State| s.open_weave_config());
                rows.push(Row::new("weave-config", "Config").summary(words).control(edit).build(s, t));
            }
            Some(kit::group(t, "weave", TITLE, Some(HINT), rows))
        }
    }
}
