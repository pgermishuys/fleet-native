//! Settings → General (`GeneralSection.vue`): the everyday preference switches in three groups. Every one is a server
//! preference on this machine; the page asks for them when it opens, and a switch waits while they load.
use super::kit::{self, Row, SwitchState};
use crate::State;
use crate::fleet::settings::switches::{self, LARGE_PASTES, Switch};
use crate::icons::CHEVRON_DOWN;
use crate::theme::{Theme, sans};
use crate::ui::{Open, rem, select};
use rune::{Cursor, El, col, icon, row, text};
use std::rc::Rc;

const LARGE_PASTES_KEY: &str = "large-pastes-select";

/// A row with a switch on a preference. `spinner` is false for the one Fleet doesn't show a spinner on (the usage chip).
fn switch_row(s: &State, t: &Theme, spec: &Spec) -> Row {
    let on = spec.switch.enabled(&s.pickers.prefs);
    let saving = s.setting_saving(spec.switch.key);
    let state = SwitchState { saving: saving && spec.spinner, disabled: s.preferences_loading() || saving, pending: false };
    let switch = spec.switch;
    Row::new(spec.id, spec.title).summary(spec.summary).more(spec.more).control(kit::setting_switch(
        t,
        spec.key,
        spec.label,
        on,
        state,
        move |s: &mut State| s.toggle_setting(switch),
    ))
}

struct Spec {
    id: &'static str,
    title: &'static str,
    summary: &'static str,
    more: &'static str,
    /// The element's key and its accessible name (Fleet's `data-testid` and `aria-label`).
    key: &'static str,
    label: &'static str,
    switch: Switch,
    spinner: bool,
}

const STARTERS: Spec = Spec {
    id: "starters",
    title: "Show starters",
    summary: "A few sentences to start from on the new-session page.",
    more: "On the new-session page, offer a few sentences to start from, such as \"Explore the code\". A starter fills the box; \
           nothing is sent until you press Enter.",
    key: "starters-switch",
    label: "Show starters",
    switch: switches::STARTERS,
    spinner: true,
};
const NOTIFICATIONS: Spec = Spec {
    id: "notifications",
    title: "Desktop notifications",
    summary: "When a session needs you or finishes, unless it's the one in front of you.",
    more: "Tell me when a session needs an answer, or finishes, while I'm looking at something else. Nothing is sent about the \
           session in front of you.",
    key: "notifications-switch",
    label: "Enable desktop notifications",
    switch: switches::DESKTOP_NOTIFICATIONS,
    spinner: true,
};
const RECAP: Spec = Spec {
    id: "recap",
    title: "Session recap",
    summary: "A one-line recap when a turn finishes out of view. One extra request each.",
    more: "When a turn finishes while you're looking at something else, write a one-line recap above the composer for when you \
           come back. Each recap is one extra request to the session's model.",
    key: "recap-switch",
    label: "Enable session recap",
    switch: switches::SESSION_RECAP,
    spinner: true,
};
const RESTART_CONTINUE: Spec = Spec {
    id: "restart-continue",
    title: "Continue cut-off sessions by itself",
    summary: "After a restart, picks up the sessions it cut off mid-turn, two at a time.",
    more: "When Fleet restarts while a session is mid-turn, Fleet sends it a message labelled \"Sent by Fleet after the restart\" \
           that names the call that didn't finish. Only sessions cut off less than 2 hours ago, not waiting on a question, with \
           their folder still there. Two at a time. A notice lists them and has Stop all.",
    key: "restart-continue-switch",
    label: "Continue cut-off sessions by itself",
    switch: switches::RESTART_CONTINUE_AUTO,
    spinner: true,
};
const RETRY: Spec = Spec {
    id: "retry-after-limits",
    title: "Try again when a limit resets",
    summary: "Sends \"Continue where you left off.\" once a rate limit resets.",
    more: "When a model provider's rate limit or usage limit stops a turn, send \"Continue where you left off.\" when the limit \
           resets, or after a wait when the provider doesn't say. What you queued waits for it.",
    key: "retry-after-limits-switch",
    label: "Try again when a limit resets",
    switch: switches::RETRY_AFTER_LIMITS,
    spinner: true,
};
const USAGE: Spec = Spec {
    id: "usage-chip",
    title: "Show usage in the status bar",
    summary: "The tightest provider limit, grey until 80%.",
    more: "Fleet asks the providers you've signed in to how much you've used, read-only: Claude automatically, the others after \
           you press Allow. The chip shows the tightest limit, grey until 80%. Turn it off to hide it; nothing else changes.",
    key: "usage-limits-chip-switch",
    label: "Show usage in the status bar",
    switch: switches::USAGE_LIMITS_CHIP,
    spinner: false,
};
const BOARD: Spec = Spec {
    id: "board",
    title: "Board",
    summary: "The Board entry in the rail, with its panels.",
    more: "Show the Board entry in the left rail and enable the board workspace panels.",
    key: "board-switch",
    label: "Enable Board feature",
    switch: switches::BOARD,
    spinner: true,
};

/// What the select shows for the stored choice; an unknown value reads as the default, "Ask".
fn large_pastes_choice(s: &State) -> &'static str {
    let stored = s.pickers.prefs.get(crate::composer::attached::LARGE_PASTES_KEY).unwrap_or("ask");
    LARGE_PASTES.iter().find(|(v, _)| *v == stored).map_or("ask", |(v, _)| *v)
}

/// `rounded-md border border-border bg-transparent px-2 py-1 text-sm text-text`. It never waits for the preferences and has
/// no saving state, as Fleet's doesn't.
fn large_pastes(s: &State, t: &Theme) -> El {
    let choice = large_pastes_choice(s);
    let words = LARGE_PASTES.iter().find(|(v, _)| *v == choice).map_or("Ask", |(_, w)| *w);
    row()
        .key(LARGE_PASTES_KEY)
        .role("combobox")
        .label("Large pastes")
        .gap(7.0)
        .px(7.0)
        .py(3.5)
        .center_y()
        .radius(6.0)
        .border(1.0, t.border)
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 0.0, t.accent)
        .child(text(words).font(sans(rem(0.875), 400.0, 1.43)).color(t.text))
        .child(icon(&CHEVRON_DOWN, 12.0).color(t.text))
        .on_press(|s: &mut State| s.ui.open = Some(Open::Select("large-pastes")))
}

/// The open list under the select: the three choices; picking one saves it at once.
pub fn large_pastes_list(s: &State, t: &Theme) -> Vec<El> {
    let choose: select::Choose = Rc::new(|s: &mut State, v: &str| {
        s.ui.open = None;
        s.save_setting(crate::composer::attached::LARGE_PASTES_KEY, v);
    });
    select::content(t, "large-pastes-list", LARGE_PASTES_KEY, 140.0, &LARGE_PASTES, Some(large_pastes_choice(s)), choose)
}

/// The line under Desktop notifications: this app saves the choice (the phone and the browser honour it) but shows nothing
/// itself yet (owner's decision 3, docs/parity/16-settings.md).
fn notifications_note(t: &Theme) -> El {
    row().key("notifications-later").mt(7.0).child(
        text("This app doesn't show them yet. The choice is kept for Fleet on your phone and in the browser.")
            .font(sans(rem(0.75), 400.0, 1.33))
            .color(t.muted)
            .wrap()
            .shrink()
            .min_w(0.0),
    )
}

pub fn content(s: &State, t: &Theme) -> El {
    let r = |spec: &Spec| switch_row(s, t, spec);
    let new_sessions = vec![
        r(&STARTERS).build(s, t),
        Row::new("large-pastes", "Large pastes")
            .summary("When you paste 2,000 characters or 25 lines or more.")
            .control(large_pastes(s, t))
            .build(s, t),
    ];
    let away = vec![
        r(&NOTIFICATIONS).below(notifications_note(t)).build(s, t),
        r(&RECAP).build(s, t),
        r(&RESTART_CONTINUE).build(s, t),
        r(&RETRY).build(s, t),
    ];
    let rail = vec![r(&USAGE).build(s, t), r(&BOARD).build(s, t)];
    col()
        .key("general-section")
        .w_full()
        .gap(21.0)
        .child(super::super::settings::page_header(s, t, &crate::fleet::settings_nav::Page::General))
        .child(kit::group(t, "new-sessions", "New sessions", None, new_sessions))
        .child(kit::group(t, "away", "While you're away", None, away))
        .child(kit::group(t, "rail", "Rail and status bar", None, rail))
}
