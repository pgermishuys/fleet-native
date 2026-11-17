//! Settings → Memory's Dreaming panel (`memory/DreamingPanel.vue`): the header with the harness and "Dream now", the dream
//! that is reading or failed, or the summary of the last one. Its flags and changes are `dream_changes.rs`.
use super::dream_changes;
use super::memory_notes::pressable;
use crate::State;
use crate::conv::time::local_offset_secs;
use crate::fleet::row_status::{format_date_time, to_timestamp};
use crate::fleet::settings::dreaming::{Dream, Status, plural};
use crate::icons::{CHEVRON_DOWN, CIRCLE_ALERT, MOON};
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, sans};
use crate::ui::{Open, confirm::spinner, rem, select};
use crate::view::message::now_ms;
use rune::{Cursor, El, col, icon, row, text};
use std::rc::Rc;

pub const HARNESS_SELECT: &str = "dreaming-harness";

/// `text-xs text-muted`.
pub fn small(t: &Theme, words: &str) -> El {
    text(words.to_string()).font(sans(rem(0.75), 400.0, 1.33)).color(t.muted)
}

/// A link that opens a session (`text-accent-text hover:underline`; Rune draws no underline on hover, so only the colour).
pub fn session_link(t: &Theme, key: String, words: &str, id: &str, size: f32) -> El {
    let id = id.to_string();
    text(words.to_string())
        .key(key)
        .font(sans(size, 400.0, 1.4))
        .color(t.accent_text)
        .role("link")
        .label(words.to_string())
        .focusable(true)
        .cursor(Cursor::Pointer)
        .on_press_tx(move |s: &mut State, tx: &mut rune::Tx| s.open_dreaming_session(&id, tx))
}

fn harness_field(s: &State, t: &Theme) -> El {
    let names = s.dreaming_harnesses();
    let chosen = s.dreaming_harness_type();
    let label = names.iter().find(|(kind, _)| *kind == chosen).map_or(String::new(), |(_, name)| name.clone());
    let field = row()
        .key(HARNESS_SELECT)
        .gap(7.0)
        .px(7.0)
        .py(3.5)
        .center_y()
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.main_bg)
        .role("combobox")
        .label("Harness for dreaming")
        .focusable(true)
        .focus_ring(2.0, 1.0, t.accent)
        .child(text(label).font(sans(rem(0.875), 400.0, 1.43)).color(t.text))
        .child(icon(&CHEVRON_DOWN, 12.0).color(t.muted));
    if names.is_empty() {
        field
    } else {
        field.cursor(Cursor::Pointer).on_press(|s: &mut State| s.ui.open = Some(Open::Select(HARNESS_SELECT)))
    }
}

/// The open list under the harness field: each enabled harness by its name.
pub fn harness_list(s: &State, t: &Theme) -> Vec<El> {
    let names = s.dreaming_harnesses();
    let options: Vec<(&str, &str)> = names.iter().map(|(kind, name)| (kind.as_str(), name.as_str())).collect();
    let chosen = s.dreaming_harness_type();
    let choose: select::Choose = Rc::new(|s: &mut State, kind: &str| {
        s.ui.open = None;
        s.dreaming_pick_harness(kind);
    });
    select::content(t, "dreaming-harness-list", HARNESS_SELECT, 160.0, &options, Some(chosen.as_str()), choose)
}

/// "Dream now": `rounded-btn bg-accent px-3 py-1 text-sm font-medium`, 60% while it can't be pressed.
fn dream_now(s: &State, t: &Theme) -> El {
    let d = &s.settings.memory.dreaming;
    let off = d.busy() || d.starting;
    let e = row()
        .px(10.5)
        .py(3.5)
        .radius(RADIUS_BTN)
        .fill(t.accent)
        .opacity(if off { 0.6 } else { 1.0 })
        .focus_ring(2.0, 1.0, t.accent)
        .child(text("Dream now").font(sans(rem(0.875), 500.0, 1.43)).color(t.primary_fg));
    pressable(e, "dream-now".into(), "Dream now", off, |s: &mut State| s.dream_now())
}

fn header(s: &State, t: &Theme) -> El {
    row()
        .w_full()
        .flex_wrap()
        .center_y()
        .justify_between()
        .gap(7.0)
        .child(
            row()
                .center_y()
                .gap(7.0)
                .child(icon(&MOON, 15.0).color(t.accent_text))
                .child(text("Dreaming").font(sans(rem(0.875), 500.0, 1.43)).color(t.text)),
        )
        .child(row().center_y().gap(7.0).child(harness_field(s, t)).child(dream_now(s, t)))
}

/// `ErrorNote` (not compact): the tinted box.
fn failure(t: &Theme, words: &str) -> El {
    row()
        .key("dream-failed")
        .role("alert")
        .w_full()
        .mt(10.5)
        .gap(7.0)
        .items_start()
        .px(10.5)
        .py(7.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.error.alpha(0.3))
        .border_layout()
        .fill(t.error.alpha(0.1))
        .child(icon(&CIRCLE_ALERT, 16.0).color(t.error_text).mt(1.0))
        .child(text(words.to_string()).font(sans(rem(0.875), 400.0, 1.43)).color(t.error_text).wrap().shrink().min_w(0.0))
}

fn busy_line(t: &Theme, dream: &Dream) -> El {
    let n = dream.read_sessions.len();
    let mut words = "Dreaming…".to_string();
    if n > 0 {
        words += &format!(" reading {n} {}", plural(n, "session", "sessions"));
    }
    let mut line = row()
        .key("dream-running")
        .w_full()
        .mt(10.5)
        .flex_wrap()
        .center_y()
        .gap(7.0)
        .child(spinner(t.muted))
        .child(text(words).font(sans(rem(0.875), 400.0, 1.43)).color(t.muted));
    if let Some(id) = &dream.session_id {
        line = line.child(session_link(t, "dream-open-session".into(), "Open the dreaming session", id, rem(0.875)));
    }
    line
}

/// "Last night · lantern · 2 notes changed" and what it read.
fn summary(s: &State, t: &Theme, dream: &Dream) -> El {
    let (now, offset) = (now_ms(s), local_offset_secs());
    let n = dream.read_sessions.len();
    let mut meta = format!("Read {n} {}", plural(n, "session", "sessions"));
    if let Some(tokens) = dream.tokens {
        meta += &format!(" · {} tokens", crate::composer::format::format_tokens(Some(tokens as f64)));
    }
    if let Some(finished) = &dream.finished {
        let at = to_timestamp(finished, offset).map_or_else(|| finished.clone(), |at| format_date_time(at, now, offset));
        meta += &format!(" · {at}");
    }
    let mut line = row().key("dream-meta").w_full().mt(1.75).flex_wrap().center_y().gap(3.5);
    match &dream.session_id {
        Some(id) => {
            line = line.child(small(t, &format!("{meta} ·"))).child(session_link(
                t,
                "dream-open-session".into(),
                "Open the dreaming session",
                id,
                rem(0.75),
            ));
        }
        None => line = line.child(small(t, &meta)),
    }
    col()
        .key("dream-summary")
        .w_full()
        .mt(10.5)
        .child(text(dream.heading()).key("dream-heading").font(sans(rem(0.875), 500.0, 1.43)).color(t.text).wrap().w_full())
        .child(line)
}

/// The panel: shown with Memory on and a repository chosen.
pub fn panel(s: &State, t: &Theme) -> El {
    let d = &s.settings.memory.dreaming;
    let mut panel = col()
        .key("dreaming-panel")
        .w_full()
        .mt(17.5)
        .px(14.0)
        .py(14.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.main_bg)
        .child(header(s, t));
    if let Some(message) = &d.error {
        panel = panel.child(col().w_full().mt(3.5).child(super::kit::error_note(t, "dreaming-error", message)));
    }
    let Some(dream) = &d.dream else { return panel };
    if matches!(dream.status, Status::Reading | Status::Running) {
        return panel.child(busy_line(t, dream));
    }
    if dream.status == Status::Failed {
        let why = dream.error.as_deref().unwrap_or("no reason given.");
        return panel.child(failure(t, &format!("The last dream failed: {why}")));
    }
    panel
        .child(summary(s, t, dream))
        .children(dream_changes::flags(s, t, dream))
        .children(dream_changes::changes(s, t, dream))
        .children(dream_changes::skipped(s, t, dream))
}

/// Used by the flag and change cards for their small buttons.
pub fn small_button(t: &Theme, key: String, label: &str, disabled: bool, press: impl Fn(&mut State) + 'static) -> El {
    let e = row()
        .px(7.0)
        .py(1.75)
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .border_layout()
        .opacity(if disabled { 0.6 } else { 1.0 })
        .focus_ring(2.0, 1.0, t.accent)
        .child(text(label.to_string()).font(sans(rem(0.75), 400.0, 1.33)).color(t.text));
    pressable(e, key, label, disabled, press)
}
