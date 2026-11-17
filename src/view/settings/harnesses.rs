//! Settings → Harnesses (`HarnessesSection.vue`): the Default harness group, a tab per harness, and the picked harness's panel
//! (heading with its pills and Enabled switch, the "What it may do" card, its own switches). The pieces other parts build
//! (install, providers, Weave, profiles, the Copilot path) go where `slots` puts them. Sizes are Fleet's Tailwind classes at the
//! 14px root.
use super::kit::{self, Row, SwitchState};
use crate::State;
use crate::fleet::new_session::harness_choice::{HarnessEntry, HarnessSetting};
use crate::fleet::settings::harnesses::{self as page, Status, setting_on};
use crate::icons::*;
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, mono, sans};
use crate::ui::button::{self, Size, Variant};
use crate::ui::{Open, confirm::spinner, rem, select};
use rune::{Color, Cursor, El, Icon, Span, col, el, icon, rich, row, text};
use std::rc::Rc;

pub const DEFAULT_SELECT_KEY: &str = "harnesses-default-select";

/// A pill's colours by state (`harnessStatusClasses`): border, fill, ink.
fn tones(t: &Theme, status: Status) -> (Color, Color, Color) {
    match status {
        Status::Ready => (t.running.alpha(0.3), t.running.alpha(0.1), t.running_text),
        Status::SignInRequired | Status::UpdateNeeded => (t.idle.alpha(0.3), t.idle.alpha(0.1), t.idle_text),
        Status::NotWorking => (t.error.alpha(0.3), t.error.alpha(0.1), t.error_text),
        Status::NotInstalled | Status::Disabled => (t.border, t.main_bg, t.muted),
    }
}

fn status_icon(status: Status) -> &'static Icon {
    match status {
        Status::Ready => &CIRCLE_CHECK,
        Status::SignInRequired | Status::NotWorking | Status::UpdateNeeded => &TRIANGLE_ALERT,
        Status::NotInstalled | Status::Disabled => &CIRCLE_DASHED,
    }
}

/// `rounded-full border px-2 py-0.5 text-[10px] font-medium uppercase`, an 11px icon first.
fn pill(key: &str, glyph: &'static Icon, words: &str, edge: Color, fill: Color, ink: Color) -> El {
    row()
        .key(key.to_string())
        .center_y()
        .gap(3.5)
        .px(7.0)
        .py(1.75)
        .radius(999.0)
        .fill(fill)
        .border(1.0, edge)
        .border_layout()
        .child(icon(glyph, 11.0).color(ink))
        .child(text(words.to_uppercase()).font(sans(10.0, 500.0, 1.5)).color(ink))
}

/// The dot of a tab (`size-1.5`): green when ready, amber when it needs something, faint when it isn't there.
fn dot_colour(t: &Theme, status: Status) -> Color {
    match status {
        Status::Ready => t.running,
        Status::SignInRequired | Status::UpdateNeeded | Status::NotWorking => t.idle,
        Status::NotInstalled | Status::Disabled => t.muted.alpha(0.5),
    }
}

fn small(t: &Theme, words: impl Into<String>) -> El {
    text(words.into()).font(sans(rem(0.75), 400.0, 1.33)).color(t.muted)
}

/// The select (`h-8 rounded-btn border border-border bg-main-bg px-2 text-xs`), showing the default harness's name.
fn default_select(s: &State, t: &Theme) -> El {
    let list = s.harness_list();
    let name = list.all.iter().find(|h| h.kind == list.default_type()).map_or(String::new(), |h| h.display_name.clone());
    row()
        .key(DEFAULT_SELECT_KEY)
        .role("combobox")
        .label("Default harness")
        .h(28.0)
        .gap(7.0)
        .px(7.0)
        .center_y()
        .radius(RADIUS_BTN)
        .fill(t.main_bg)
        .border(1.0, t.border)
        .border_layout()
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 0.0, t.accent)
        .child(text(name).font(sans(rem(0.75), 400.0, 1.33)).color(t.text))
        .child(icon(&CHEVRON_DOWN, 12.0).color(t.text))
        .on_press(|s: &mut State| s.ui.open = Some(Open::Select("default-harness")))
}

/// The open list: every harness, in the server's order; one that is turned off can't be the default.
pub fn default_list(s: &State, t: &Theme) -> Vec<El> {
    let list = s.harness_list();
    let options: Vec<(String, String)> = list.all.iter().map(|h| (h.kind.clone(), h.display_name.clone())).collect();
    let options: Vec<(&str, &str)> = options.iter().map(|(k, n)| (k.as_str(), n.as_str())).collect();
    let choose: select::Choose = Rc::new(|s: &mut State, kind: &str| s.set_default_harness(kind));
    select::content(t, "harnesses-default-list", DEFAULT_SELECT_KEY, 140.0, &options, Some(&list.default_type()), choose)
}

fn default_group(s: &State, t: &Theme) -> El {
    let checked = s.harness_checked();
    let mut controls = row().flex_wrap().center_y().justify_end().gap(7.0).child(default_select(s, t));
    // "Set up a harness" acts on the machine the app is connected to only.
    if !s.settings.choice.is_other(&s.machines) {
        controls = controls.child(
            button::button(t, Variant::Outline, Size::Sm, Some(&DOWNLOAD), "Set up a harness")
                .key("harnesses-set-up")
                .on_press(|s: &mut State| s.open_harness_setup()),
        );
    }
    let checking = s.harnesses_checking();
    let again = if checking {
        button::button_lead(t, Variant::Outline, Size::Sm, Some(spinner(t.muted)), "Check again")
    } else {
        button::button(t, Variant::Outline, Size::Sm, Some(&REFRESH_CW), "Check again")
    };
    let again = again.key("harnesses-check-again").role("button").label("Check again");
    controls = controls.child(if checking { button::disabled(again) } else { again.on_press(|s: &mut State| s.check_harnesses_again()) });
    let mut row_ = Row::new("default-harness", "Default harness").control(controls);
    match checked {
        Some((words, exact)) => {
            let shown = if checking { "Checking\u{2026}".to_string() } else { words };
            row_ = row_.below(row().mt(2.0).child(small(t, shown).key("harnesses-checked-at").label(exact)));
        }
        None => row_ = row_.summary("The harness a new session starts on."),
    }
    kit::group(t, "default-harness", "Default harness", Some("Used when a new session doesn't pick one."), vec![row_.build(s, t)])
}

fn tab(s: &State, t: &Theme, h: &HarnessEntry, on: bool) -> El {
    let status = Status::of(h, s.harness_enabled(h));
    let kind = h.kind.clone();
    row()
        .key(format!("harness-tab-{}", h.kind))
        .role("tab")
        .label(h.display_name.clone())
        .checked(on)
        .hover_group()
        .center_y()
        .gap(7.0)
        .px(10.5)
        .py(7.0)
        .border_sides([0.0, 0.0, 2.0, 0.0], if on { t.accent } else { Color::TRANSPARENT })
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 0.0, t.accent)
        .child(el().size(6.0, 6.0).radius(3.0).fill(dot_colour(t, status)).key(format!("harness-tab-dot-{}", h.kind)).label(status.label()))
        .child(
            text(h.display_name.clone()).font(sans(rem(0.875), 400.0, 1.43)).color(if on { t.text } else { t.muted }).group_color(t.text),
        )
        .on_press(move |s: &mut State| s.select_harness_tab(&kind))
}

fn tab_strip(s: &State, t: &Theme, shown: &str) -> El {
    let tabs = s.harness_list().all.iter().map(|h| tab(s, t, h, h.kind == shown)).collect::<Vec<_>>();
    row()
        .key("harness-tabs")
        .role("tablist")
        .label("Harnesses")
        .w_full()
        .gap(3.5)
        .border_sides([0.0, 0.0, 1.0, 0.0], t.border)
        .children(tabs)
}

/// The heading and what Fleet knows about the harness.
fn heading(s: &State, t: &Theme, h: &HarnessEntry) -> El {
    let enabled = s.harness_enabled(h);
    let status = Status::of(h, enabled);
    let (edge, fill, ink) = tones(t, status);
    let mut title =
        row().flex_wrap().center_y().gap(7.0).child(text(h.display_name.clone()).font(sans(rem(1.125), 600.0, 1.55)).color(t.text));
    if s.harness_list().default_type() == h.kind {
        title = title.child(pill("harness-default-pill", &STAR, "Default", t.accent.alpha(0.25), t.accent.alpha(0.1), t.accent_text));
    }
    title = title.child(pill("harness-status-pill", status_icon(status), status.label(), edge, fill, ink));
    let mut left = col().grow(1.0).shrink().min_w(0.0).child(title);
    let line = |words: String| row().mt(3.5).child(small(t, words).wrap().shrink().min_w(0.0));
    left = left.child(line(page::eyebrow_line(h))).child(line(page::summary(h, enabled)));
    if let Some(place) = page::location(h) {
        left = left.child(
            row().mt(3.5).child(text(place).key("harness-location").font(mono(11.0, 400.0, 1.5)).color(t.muted).wrap().shrink().min_w(0.0)),
        );
    }
    let kind = h.kind.clone();
    let switch = kit::setting_switch(
        t,
        "harness-enabled-switch",
        &format!("{} enabled", h.display_name),
        enabled,
        SwitchState::default(),
        move |s: &mut State| s.toggle_harness(&kind),
    );
    let enabled_label = text(if enabled { "Enabled" } else { "Disabled" }).font(sans(rem(0.75), 500.0, 1.33)).color(t.muted);
    row().w_full().items_start().justify_between().gap(14.0).child(left).child(row().center_y().gap(7.0).child(enabled_label).child(switch))
}

/// "Its version and updates are on Updates & about." with the link.
fn version_line(t: &Theme) -> El {
    let font = sans(rem(0.75), 400.0, 1.33);
    row()
        .key("harness-version")
        .w_full()
        .px(3.5)
        .flex_wrap()
        .items_end()
        .child(text("Its version and updates are on").font(font).color(t.muted).mr(3.0))
        .child(
            row()
                .key("harness-updates-link")
                .role("button")
                .label("Updates & about")
                .cursor(Cursor::Pointer)
                .focusable(true)
                .focus_ring(2.0, 0.0, t.accent)
                .child(text("Updates & about").font(font).color(t.accent_text))
                .on_press(|s: &mut State| s.open_settings("updates", Some("harnesses"))),
        )
        .child(text(".").font(font).color(t.muted))
}

/// "What it may do": what Fleet hands the harness, and the way to where it is set.
fn may_do(s: &State, t: &Theme, h: &HarnessEntry) -> El {
    let font = sans(rem(0.75), 400.0, 1.33);
    let words = format!("{} Edits, commands, the browser and messages are set in", s.harness_may_do(h));
    let card = row()
        .key(format!("harness-may-do-{}", h.kind))
        .w_full()
        .flex_wrap()
        .items_end()
        .px(14.0)
        .py(10.5)
        .radius(RADIUS_CARD)
        .fill(t.card_bg)
        .border(1.0, t.border)
        .border_layout()
        .child(text(words).font(font).color(t.muted).wrap().shrink().min_w(0.0).mr(3.0))
        .child(
            row()
                .key("harness-may-do-link")
                .role("button")
                .label("What agents can do")
                .cursor(Cursor::Pointer)
                .focusable(true)
                .focus_ring(2.0, 0.0, t.accent)
                .child(rich("What agents can do", vec![Span::new(0, 18).underline(t.accent_text)]).font(font).color(t.accent_text))
                .on_press(|s: &mut State| s.open_settings("permissions", None)),
        )
        .child(text(".").font(font).color(t.muted));
    kit::anchor("may-do", card)
}

/// One of the harness's own switches (`harness-setting-<key>`).
fn setting_row(s: &State, t: &Theme, setting: &HarnessSetting) -> El {
    let prefs = s.harness_prefs();
    let saving = s.settings.harnesses.saving.as_deref() == Some(setting.key.as_str());
    let busy = s.harness_prefs_loading() || s.settings.harnesses.saving.is_some();
    let spec = setting.clone();
    let switch = kit::setting_switch(
        t,
        &format!("harness-setting-toggle-{}", setting.key),
        &setting.label,
        setting_on(setting, &prefs),
        SwitchState { saving, disabled: busy, pending: false },
        move |s: &mut State| s.toggle_harness_setting(&spec),
    );
    let summary = format!("{} by default. {}", if setting.default { "On" } else { "Off" }, setting.description);
    let mut r = Row::new(&format!("harness-setting-{}", setting.key), &setting.label).summary(&summary).control(switch);
    if let Some((_, message)) = s.settings.harnesses.error.as_ref().filter(|(key, _)| *key == setting.key) {
        r = r.below(kit::error_note(t, &format!("harness-setting-error-{}", setting.key), message));
    }
    r.build(s, t)
}

/// Where the parts that come in later pieces sit, in Fleet's order: the install panel under the version line, Providers,
/// Weave, Profiles and the Copilot path field. Each piece returns its part here; until then there is none.
fn slots(s: &State, t: &Theme, h: &HarnessEntry) -> (Vec<El>, Vec<El>) {
    // (after the version line and before "What it may do", after it and before the harness's own switches)
    let mut after = Vec::new();
    if h.supports_profiles {
        after.push(super::harness_profiles::section(s, t, &h.kind, &h.display_name));
    }
    (Vec::new(), after)
}

fn panel(s: &State, t: &Theme, h: &HarnessEntry) -> El {
    let (before, after) = slots(s, t, h);
    let mut panel = col()
        .key(format!("harness-panel-{}", h.kind))
        .role("tabpanel")
        .w_full()
        .gap(21.0)
        .child(heading(s, t, h))
        .child(version_line(t))
        .children(before)
        .child(may_do(s, t, h))
        .children(after);
    if h.card.settings.is_empty() {
        if !h.card.describes_install {
            panel = panel.child(row().px(3.5).child(small(t, "No settings yet").key("harness-no-settings")));
        }
    } else {
        let rows = h.card.settings.iter().map(|setting| setting_row(s, t, setting)).collect();
        panel = panel.child(kit::group(t, "harness-settings", &format!("{} settings", h.display_name), None, rows));
    }
    panel
}

pub fn content(s: &State, t: &Theme) -> El {
    let mut page = col()
        .key("harnesses-section")
        .w_full()
        .gap(21.0)
        .child(super::super::settings::page_header(s, t, &crate::fleet::settings_nav::Page::Harnesses))
        .child(default_group(s, t));
    if let Some(shown) = s.harness_tab() {
        page = page.child(tab_strip(s, t, &shown.kind)).child(panel(s, t, &shown));
    }
    page
}
