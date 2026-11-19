//! Settings → Skills & tools → Mods (`ModsSection.vue`, `mods/ModRow.vue`): the mods the user kept, each with its switch and
//! Undo, the safe-mode callout and the "off" and "empty" notes. The Showcase below the list is `mods_showcase.rs`; the state
//! and the calls are `fleet::settings::mods_kept` and `mods_kept_actions`.
use super::kit::{self, SwitchState};
use super::mods_showcase;
use super::skills_tools::{empty_state, loading};
use crate::State;
use crate::fleet::settings::harnesses::relative;
use crate::fleet::settings::mods_kept::KeptMod;
use crate::icons::{CIRCLE_ALERT, PUZZLE, UNDO_2};
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, mono, sans};
use crate::ui::button::{Size, Variant, button};
use crate::ui::confirm::spinner;
use crate::ui::controls::separator;
use crate::ui::rem;
use crate::view::message::now_ms;
use rune::{Cursor, El, col, icon, row, text};

/// A link-looking button: `text-accent-text hover:underline`, an accent outline on focus. `size` is the font size.
pub fn link(t: &Theme, key: &str, words: &str, size: f32) -> El {
    row()
        .key(key.to_string())
        .role("button")
        .label(words.to_string())
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 0.0, t.accent)
        .child(text(words.to_string()).font(sans(size, 400.0, 1.43)).color(t.accent_text))
}

/// `rounded-btn border border-border px-3 py-1.5 text-xs font-medium` (the Showcase's Add, the callout's button): the small
/// outline button with its words at 12px, which `ui::button`'s sizes don't have. `lead` is a spinner while it works.
pub fn xs_button(t: &Theme, key: &str, label: &str, lead: Option<El>) -> El {
    let mut b = row()
        .key(key.to_string())
        .role("button")
        .label(label.to_string())
        .center()
        .gap(7.0)
        .px(rem(0.75))
        .py(rem(0.375))
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .border_layout()
        .hover(t.main_bg)
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 0.0, t.accent);
    if let Some(l) = lead {
        b = b.child(l);
    }
    b.child(text(label.to_string()).font(sans(rem(0.75), 500.0, 1.33)).color(t.text))
}

/// "…are in **Plugins › Mods**." (`setActiveSection('plugin:mods')`).
fn plugin_link(t: &Theme, lead: &str) -> El {
    let font = sans(rem(0.875), 400.0, 1.43);
    row()
        .flex_wrap()
        .items_center()
        .child(text(lead.to_string()).font(font).color(t.muted))
        .child(link(t, "mods-features-link", "Plugins \u{203a} Mods", rem(0.875)).on_press(|s: &mut State| s.open_mods_plugin()))
        .child(text(".").font(font).color(t.muted))
}

/// The safe-mode callout: `border-idle/40 bg-idle/10 rounded-card px-4 py-3`.
fn safe_mode(s: &State, t: &Theme) -> El {
    let saving = s.settings.skills.mods.saving;
    let b = xs_button(t, "mods-safe-mode-off", "Turn mods back on", None);
    let b = if saving { b.opacity(0.6).disabled(true) } else { b.on_press(|s: &mut State| s.turn_mods_back_on()) };
    row()
        .key("mods-safe-mode")
        .w_full()
        .mt(14.0)
        .items_center()
        .justify_between()
        .gap(10.5)
        .px(14.0)
        .py(10.5)
        .radius(RADIUS_CARD)
        .border(1.0, t.idle.alpha(0.4))
        .border_layout()
        .fill(t.idle.alpha(0.1))
        .child(
            text("Mods are stopped for now (Start without mods). They start again when Fleet restarts or you turn them back on.")
                .font(sans(rem(0.875), 400.0, 1.43))
                .color(t.text)
                .wrap()
                .shrink()
                .min_w(0.0),
        )
        .child(b)
}

/// `ErrorNote` with Try again: the red tint, the alert, the message, and the button under it.
fn load_error(t: &Theme, message: &str) -> El {
    col()
        .key("mods-error")
        .w_full()
        .mt(17.5)
        .gap(7.0)
        .px(10.5)
        .py(7.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.error.alpha(0.3))
        .border_layout()
        .fill(t.error.alpha(0.1))
        .child(
            row()
                .gap(7.0)
                .items_start()
                .child(icon(&CIRCLE_ALERT, 16.0).color(t.error_text).mt(1.0))
                .child(text(message.to_string()).font(sans(rem(0.875), 400.0, 1.43)).color(t.error_text).wrap().shrink().min_w(0.0)),
        )
        .child(row().child(
            button(t, Variant::Outline, Size::Sm, None, "Try again").key("mods-error-retry").on_press(|s: &mut State| s.retry_mods()),
        ))
}

/// The line under the description: "kept 2 days ago · from “Make test output readable”", the title a link to its session.
fn kept_line(s: &State, t: &Theme, m: &KeptMod) -> Option<El> {
    let v = m.active_entry()?;
    let font = sans(rem(0.75), 400.0, 1.33);
    let when = crate::fleet::work::parse_iso_ms(&v.created_at)
        .map(|at| relative(at, now_ms(s), crate::conv::time::local_offset_secs()))
        .unwrap_or_default();
    let mut line = row().key(format!("mod-kept-{}", m.name)).flex_wrap().items_center().mt(3.5);
    line = line.child(text(format!("kept {when}")).font(font).color(t.muted));
    if let Some(title) = &v.session_title {
        line = line.child(text(" \u{b7} from \u{201c}".to_string()).font(font).color(t.muted));
        line = match &v.session_id {
            Some(id) => {
                let id = id.clone();
                line.child(
                    text(title.clone())
                        .key(format!("mod-session-{}", m.name))
                        .font(font)
                        .color(t.accent_text)
                        .role("link")
                        .label(title.clone())
                        .focusable(true)
                        .cursor(Cursor::Pointer)
                        .on_press_tx(move |s: &mut State, tx: &mut rune::Tx| s.open_dreaming_session(&id, tx)),
                )
            }
            None => line.child(text(title.clone()).font(font).color(t.muted)),
        };
        line = line.child(text("\u{201d}".to_string()).font(font).color(t.muted));
    }
    Some(line)
}

fn name_line(t: &Theme, m: &KeptMod) -> El {
    let mut line =
        row().flex_wrap().items_center().gap(7.0).child(
            text(m.name.clone()).key(format!("mod-name-{}", m.name)).font(mono(rem(0.875), 600.0, 1.43)).color(t.text).wrap_anywhere(),
        );
    // Fleet prints the text "vnull" before anything is kept (it has no guard); the server only sends none then.
    let active = m.active.map_or("null".to_string(), |n| n.to_string());
    line = line.child(text(format!("v{active}")).font(mono(rem(0.75), 400.0, 1.33)).color(t.text));
    if let Some(v) = &m.active_version {
        line = line.child(text(v.clone()).font(mono(rem(0.75), 400.0, 1.33)).color(t.muted));
    }
    line
}

/// "Off" and the switch.
fn controls(s: &State, t: &Theme, m: &KeptMod) -> El {
    let busy = s.settings.skills.mods.row_busy(&m.name);
    let name = m.name.clone();
    let on = m.is_on();
    let mut r = row().items_center().gap(7.0);
    if !on && !m.off.as_ref().is_some_and(|o| o.by_strikes()) {
        r = r.child(text("Off").key(format!("mod-off-{}", m.name)).font(sans(rem(0.75), 400.0, 1.33)).color(t.muted));
    }
    r.child(kit::setting_switch(
        t,
        &format!("mod-switch-{}", m.name),
        &format!("Run {}", m.name),
        on,
        SwitchState { disabled: busy, ..Default::default() },
        move |s: &mut State| s.set_mod_on(&name, !on),
    ))
}

fn strikes_note(t: &Theme, m: &KeptMod) -> Option<El> {
    let off = m.off.as_ref().filter(|o| o.by_strikes())?;
    let name = m.name.clone();
    let font = sans(rem(0.75), 400.0, 1.33);
    Some(
        row()
            .key(format!("mod-strikes-{}", m.name))
            .role("alert")
            .mt(7.0)
            .gap(7.0)
            .items_start()
            .child(icon(&CIRCLE_ALERT, 14.0).color(t.error_text).mt(1.0))
            .child(
                col()
                    .shrink()
                    .min_w(0.0)
                    .child(
                        text(format!("Turned off after 3 failures: {}", off.error.clone().unwrap_or_default()))
                            .font(font)
                            .color(t.error_text)
                            .wrap(),
                    )
                    .child(
                        link(t, &format!("mod-turn-on-{}", m.name), "Turn on again", rem(0.75))
                            .on_press(move |s: &mut State| s.set_mod_on(&name, true)),
                    ),
            ),
    )
}

fn undo(s: &State, t: &Theme, m: &KeptMod) -> El {
    let tab = &s.settings.skills.mods;
    let refusal = m.undo_refusal();
    let busy = tab.row_busy(&m.name);
    let name = m.name.clone();
    let font = sans(rem(0.75), 400.0, 1.33);
    let lead = if tab.busy.get(&m.name) == Some(&"undo") { spinner(t.accent_text) } else { icon(&UNDO_2, 12.0).color(t.accent_text) };
    let b = row()
        .key(format!("mod-undo-{}", m.name))
        .role("button")
        .label(m.undo_label())
        .gap(3.5)
        .items_center()
        .focusable(true)
        .focus_ring(2.0, 0.0, t.accent)
        .child(lead)
        .child(text(m.undo_label()).font(font).color(t.accent_text));
    let b = if busy || refusal.is_some() {
        b.opacity(0.5).disabled(true).cursor(Cursor::Default)
    } else {
        b.cursor(Cursor::Pointer).on_press(move |s: &mut State| s.undo_mod(&name))
    };
    let mut c = col().mt(10.5).child(row().child(b));
    // Rune has no tooltips, so the reason Fleet puts in `title` is a muted line under the link.
    if let Some(why) = refusal {
        c = c.child(text(why.to_string()).key(format!("mod-undo-why-{}", m.name)).font(font).color(t.muted).mt(3.5).wrap());
    }
    c
}

/// `ModRow`: the name, version and words on the left, the switch on the right, then why it is off and Undo.
fn mod_row(s: &State, t: &Theme, m: &KeptMod) -> El {
    let mut words = col().w(0.0).grow(1.0).min_w(0.0).child(name_line(t, m));
    if let Some(d) = &m.description {
        words =
            words.child(row().mt(3.5).child(text(d.clone()).font(sans(rem(0.875), 400.0, 1.43)).color(t.muted).wrap().shrink().min_w(0.0)));
    }
    if let Some(k) = kept_line(s, t, m) {
        words = words.child(k);
    }
    let mut r = col()
        .key(format!("mod-row-{}", m.name))
        .w_full()
        .px(14.0)
        .py(12.25)
        .child(row().w_full().items_start().justify_between().gap(10.5).child(words).child(controls(s, t, m)));
    if let Some(n) = strikes_note(t, m) {
        r = r.child(n);
    }
    if let Some(e) = s.settings.skills.mods.row_errors.get(&m.name) {
        r = r.child(kit::error_note(t, &format!("mod-error-{}", m.name), e));
    }
    r.child(undo(s, t, m))
}

fn list(s: &State, t: &Theme) -> El {
    let mut c =
        col().key("mods-list").w_full().mt(17.5).radius(RADIUS_CARD).border(1.0, t.border).border_layout().clip_rounded(RADIUS_CARD);
    for (i, m) in s.settings.skills.mods.mods().iter().enumerate() {
        if i > 0 {
            c = c.child(separator(t));
        }
        c = c.child(mod_row(s, t, m));
    }
    c
}

/// The tab: `rounded-card border bg-card-bg p-4 sm:p-6`.
pub fn tab(s: &State, t: &Theme) -> El {
    let page = &s.settings.skills.mods;
    let on = s.skills_mods_on();
    let mut c = col()
        .key("mods-section")
        .w_full()
        .padding(21.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.card_bg)
        .child(text("Mods").font(sans(rem(1.125), 600.0, 1.56)).color(t.text));
    if on {
        c = c.child(col().mt(3.5).child(plugin_link(t, "Mods are on. The switch and the mod runtime (Bun) are in ")));
    }
    if !on && !page.loading {
        return c.child(
            col()
                .key("mods-switched-off")
                .w_full()
                .mt(17.5)
                .center()
                .padding(21.0)
                .radius(RADIUS_CARD)
                .border(1.0, t.border)
                .border_layout()
                .border_dash(3.0, 3.0)
                .child(plugin_link(t, "Mods are off. Turn Mods on in ")),
        );
    }
    if page.safe_mode() {
        c = c.child(safe_mode(s, t));
    }
    c = if page.loading {
        c.child(col().mt(17.5).child(loading(t, "mods-loading", "Loading your mods\u{2026}")))
    } else if let Some(e) = &page.error {
        c.child(load_error(t, e))
    } else if page.mods().is_empty() {
        c.child(col().mt(17.5).w_full().child(empty_state(
            t,
            "mods-empty",
            &PUZZLE,
            "No kept mods yet",
            "When an agent writes a mod in a session, you can keep it from there, or add one from the Showcase below.",
        )))
    } else {
        c.child(list(s, t))
    };
    if !page.loading && page.error.is_none() {
        c = c.child(mods_showcase::showcase(s, t));
    }
    c
}
