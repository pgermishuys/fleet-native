//! The Automations list, `[automations-list]` (`AutomationsNavPanel.vue`): the header, the machine switch, New automation, and a
//! row for each automation with the word that says when it runs next, or that it's running, off or failed.
use crate::State;
use crate::fleet::automations::model::Automation;
use crate::fleet::automations::nav::{ViewMode, machine_of};
use crate::fleet::automations::words::{Glyph, RowStatus, Tone, automation_row_status};
use crate::icons::{PENCIL, PLUS};
use crate::theme::{RADIUS_BTN, Theme, mono, sans};
use crate::ui::{empty_state, rem};
use crate::view::{files_tab, machine_switch, message::now_ms, parts};
use rune::{Cursor, El, ScrollSpec, col, icon, row, text};

/// The switch under the header: "Automations for machine".
pub const SWITCH: machine_switch::Switch =
    machine_switch::Switch { label: "Automations", testid: "automations", choice: |s| &mut s.automations_nav.choice };

/// The switch's open menu, over the window.
pub fn overlay(s: &State, t: &Theme) -> Vec<El> {
    machine_switch::overlay(s, t, SWITCH, &s.automations_nav.choice)
}

pub fn panel(s: &State, t: &Theme) -> El {
    let nav = &s.automations_nav;
    let store = s.shown_automations();
    let header = row().pt(4.0).child(
        row().px(16.0).pt(14.0).pb(10.0).child(text("AUTOMATIONS").font(sans(rem(0.71429), 600.0, 1.5).tracking(0.5)).color(t.muted)),
    );
    let machine = machine_switch::chip(s, t, SWITCH, &nav.choice).map(|chip| row().px(12.0).pb(6.0).child(chip));
    let create = row().w_full().px(12.0).pb(12.0).child(new_automation(t));
    let body = if nav.mode == ViewMode::Create || (!store.is_loading && !store.list.is_empty()) {
        rows(s, t)
    } else if store.is_loading {
        col()
            .key("automations-loading")
            .w_full()
            .grow(1.0)
            .center()
            .px(12.0)
            .py(24.0)
            .label("Loading automations…")
            .child(files_tab::spinner(t.muted, rem(1.4286)))
    } else if store.error.is_some() || nav.retrying {
        let note = super::error_note(
            t,
            "automations-load-error",
            "Couldn't load automations.",
            Some(store.error.as_deref().unwrap_or(crate::fleet::automations::nav::RETRYING)),
            Some((nav.retrying, |s, tx| s.automations_retry(tx))),
        );
        col().w_full().padding(12.0).child(note)
    } else {
        col().w_full().grow(1.0).center().child(empty_state::compact_bare(t, "automations-empty", "No automations yet"))
    };
    col()
        .key("automations-list")
        .label("Automations navigation")
        .w(s.sidebar_w)
        .h_full()
        .min_h(0.0)
        .child(header)
        .children(machine)
        .child(create)
        .child(col().w_full().grow(1.0).shrink().min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(body))
}

/// `.new-automation-btn`: full width, 36 high, a 1 px border; the accent washes it on hover (Rune has no hover border).
fn new_automation(t: &Theme) -> El {
    row()
        .key("new-automation")
        .role("button")
        .label("New automation")
        .w_full()
        .min_h(36.0)
        .px(12.0)
        .gap(8.0)
        .center()
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.main_bg)
        .hover(t.mix(t.main_bg, t.accent, 0.08))
        .cursor(Cursor::Pointer)
        .focus_ring(2.0, 2.0, t.accent)
        .child(icon(&PLUS, 16.0).color(t.text))
        .child(text("New automation").font(sans(rem(0.85714), 500.0, 1.5)).color(t.text))
        .on_press_tx(|s: &mut State, tx: &mut rune::Tx| s.start_automation_create(tx))
}

/// The list: the draft row while creating, then built-ins first and the rest as the server sent them (a stable sort).
fn rows(s: &State, t: &Theme) -> El {
    let nav = &s.automations_nav;
    let store = s.shown_automations();
    let now = now_ms(s);
    let tz = |ms: i64| crate::conv::time::local_offset_at(ms);
    let mut list = col().key("automations-rows").w_full().gap(1.0).px(8.0).pb(12.0);
    if nav.mode == ViewMode::Create {
        list = list.child(draft_row(t, &nav.draft_title()));
    }
    let mut ordered: Vec<&Automation> = store.list.iter().collect();
    ordered.sort_by_key(|a| a.builtin.is_none());
    list.children(ordered.into_iter().map(|a| item(s, t, a, &automation_row_status(a, now, &tz), machine_of(s, a))))
}

/// The draft row: a pencil, the title and "Draft", always the open one.
fn draft_row(t: &Theme, title: &str) -> El {
    row()
        .key("automation-draft-row")
        .role("button")
        .label(title.to_string())
        .w_full()
        .min_w(0.0)
        .min_h(32.0)
        .gap(9.0)
        .center_y()
        .px(10.0)
        .radius(RADIUS_BTN)
        .fill(t.wash(9.0))
        .child(icon(&PENCIL, 11.0).color(t.muted))
        .child(title_text(t, title, true))
        .child(text("Draft").font(sans(rem(0.85714), 400.0, 1.3)).color(t.muted.alpha(0.8)))
}

fn title_text(t: &Theme, title: &str, active: bool) -> El {
    text(title.to_string())
        .font(sans(rem(0.92857), if active { 500.0 } else { 400.0 }, 1.3))
        .color(if active { t.text } else { t.text.alpha(0.86) })
        .grow(1.0)
        .shrink()
        .min_w(0.0)
        .truncate()
}

/// `.automation-row`: 32 high, the glyph slot, the title, its pills and the status word.
fn item(s: &State, t: &Theme, a: &Automation, status: &RowStatus, machine: Option<String>) -> El {
    let active = s.automations_nav.open_id() == Some(a.id.as_str());
    let lead: El = match status.glyph {
        Some(Glyph::Working) => row().w(8.0).h(8.0).center().label("Running").child(parts::status_glyph(t, "active", false)),
        Some(Glyph::Error) => row().w(8.0).h(8.0).center().label("Its last run failed").child(parts::status_glyph(t, "error", false)),
        None => row().w(8.0).h(8.0),
    };
    let ink = match status.tone {
        Tone::Working => t.running_text,
        Tone::Error => t.error_text,
        Tone::Warn | Tone::Quiet => t.muted.alpha(0.8),
    };
    let id = a.id.clone();
    let off = !a.is_enabled && !active;
    let mut title = title_text(t, &a.name, active);
    if off {
        title = title.color(t.text.alpha(0.45));
    }
    let mut e = row()
        .key(format!("automation-row-{}", a.id))
        .role("button")
        .label(a.name.clone())
        .w_full()
        .min_w(0.0)
        .min_h(32.0)
        .gap(9.0)
        .center_y()
        .px(10.0)
        .radius(RADIUS_BTN)
        .cursor(Cursor::Pointer)
        .hover(t.wash(5.0))
        .focus_ring(2.0, -2.0, t.accent)
        .child(lead)
        .child(title);
    if a.builtin.is_some() {
        e = e.child(
            row()
                .key("automation-row-builtin")
                .px(7.0)
                .radius(999.0)
                .border(1.0, t.border)
                .border_layout()
                .child(text("Built in").font(sans(rem(0.78571), 500.0, 1.5)).color(t.muted)),
        );
    }
    if let Some(name) = machine {
        e = e.child(
            text(name).key("automation-row-machine").font(mono(rem(0.82143), 400.0, 1.4)).color(t.coral_text).max_w(rem(7.0)).truncate(),
        );
    }
    if !status.label.is_empty() {
        e = e.child(text(status.label.clone()).key("automation-row-status").font(sans(rem(0.85714), 400.0, 1.3)).color(ink));
    }
    if active {
        e = e.fill(t.wash(9.0));
    }

    e.on_press(move |s: &mut State| s.set_active_automation(&id))
}
