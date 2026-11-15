//! The composer: the frame, the text area, the error box and the toolbar. Ports `Composer.vue` and
//! `ComposerFrame.vue` (docs/parity/05-composer.md, piece B). What goes in the toolbar's other slots
//! (the effort toggle, the Allowed chip, the ring, Send now and Queue) comes with the pieces that build
//! them; they sit where Fleet puts them: agent chip, model chip, [effort], [Allowed], Interrupt, [ring],
//! [Send now / Queue], Send.
use super::conversation::COLUMN;
use crate::composer::status::{Caps, Mode, has_content, mode_of, send_label};
use crate::icons::*;
use crate::mock::Session;
use crate::theme::{Theme, sans};
use crate::{State, interrupt, submit};
use rune::{Color, Cursor, El, Key, Shadow, auto_grow, col, el, icon, row, spacer, text, text_input};

/// Fleet puts the composer's pieces in this order above the frame: the error box, the background strip
/// (component 3, piece K), the mod band (component 22), the queue (piece I) and the paste offer (piece O).
/// Only the error box exists; the others are drawn empty until their pieces arrive.
pub fn composer(s: &State, t: &Theme, _session: &Session) -> El {
    let disabled = s.composer_disabled();
    let mode = mode_of(&s.draft.text, &Caps::default(), s.side_open);
    let mut outer = col().key("composer-section").label("Message composer").w_full().pt(4.0).px(24.0).pb(18.0).items_center();
    if let Some(error) = &s.composer_error {
        outer = outer.child(error_box(t, error));
    }
    outer.child(frame(s, t, disabled, mode))
}

/// `.composer-error`: the first thing that went wrong, in a box above the frame (`role=alert`).
fn error_box(t: &Theme, message: &str) -> El {
    col()
        .key("send-prompt-error")
        .label("Error")
        .w_full()
        .max_w(COLUMN)
        .mb(10.0)
        .px(12.0)
        .py(10.0)
        .radius(crate::theme::RADIUS_CARD)
        .border(1.0, t.error.alpha(0.3))
        .border_layout()
        .fill(t.error.alpha(0.1))
        .child(text(message).font(sans(12.0, 400.0, 1.5)).color(t.error_text).wrap())
}

fn frame(s: &State, t: &Theme, disabled: bool, mode: Mode) -> El {
    // `color-mix(--accent 55%, --border)` while the frame has focus; 70% in shell and side mode, focused or not.
    let focus = t.accent_border(55.0);
    let edge = if mode == Mode::Prompt { t.border } else { t.accent_border(70.0) };
    let placeholder = if mode == Mode::Side { "Ask a follow-up in the side conversation…" } else { "Type a message…" };
    // 14 + one 21px line + 25 is the 60px the empty field measures in Fleet; seven lines is about its 180px maximum.
    let field = text_input(&s.draft, sans(14.0, 400.0, 1.5))
        .key("composer")
        .label("Message")
        .placeholder(placeholder, t.muted)
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .w_full()
        .pt(14.0)
        .px(16.0)
        .pb(25.0)
        .opacity(if disabled { 0.5 } else { 1.0 })
        .disabled(disabled)
        .on_edit(|s: &mut State| &mut s.draft)
        .on_submit(|s: &mut State, tx| submit(s, tx, false));
    let mut frame = col()
        .key("composer-frame")
        .w_full()
        .max_w(COLUMN)
        .radius(14.0)
        .fill(t.card_bg)
        .border(1.0, edge)
        .box_shadow(Shadow { x: 0.0, y: 10.0, blur: 28.0, spread: -18.0, color: Color::BLACK.alpha(0.5), inset: false })
        .focus_within_ring(1.0, -1.0, if mode == Mode::Prompt { focus } else { edge })
        // Return is the field's, Ctrl/Cmd+Return its other choice (steer where Enter queues, queue where it steers).
        .on_key(|s: &mut State, ev, tx| {
            if ev.key == Key::Enter && (ev.mods.cmd || ev.mods.ctrl) {
                submit(s, tx, true);
                return true;
            }
            false
        });
    frame = frame.child(auto_grow(field, 1, 7)).child(toolbar(s, t, disabled, mode));
    frame
}

fn toolbar(s: &State, t: &Theme, disabled: bool, mode: Mode) -> El {
    let chip = |key: &'static str, label: &str| {
        row()
            .key(key)
            .h(28.0)
            .px(8.0)
            .gap(5.0)
            .center_y()
            .radius(8.0)
            .hover(t.wash(6.0))
            .cursor(Cursor::Pointer)
            .shrink()
            .min_w(0.0)
            .on_press(move |s: &mut State| s.ui.open = Some(crate::ui::Open::Menu(if key == "agent" { "agent" } else { "model" })))
            .child(text(label).font(sans(12.0, 400.0, 1.2)).color(t.text.alpha(0.8)).truncate().shrink())
            .child(icon(&CHEVRON_DOWN, 13.0).color(t.muted))
    };
    let inactive = disabled || !has_content(&s.draft.text, mode == Mode::Shell, false, 0);
    let label = send_label(mode);
    let mut send = el()
        .key("prompt-send-button")
        .size(30.0, 30.0)
        .center()
        .radius(15.0)
        .fill(t.accent)
        .opacity(if inactive { 0.5 } else { 1.0 })
        .help(label)
        .label(label)
        .child(icon(&ARROW_UP, 14.0).color(t.primary_fg));
    send = if inactive {
        send.disabled(true)
    } else {
        send.hover(Color::BLACK.alpha(0.1)).cursor(Cursor::Pointer).on_press_tx(|s: &mut State, tx| submit(s, tx, false))
    };
    row()
        .w_full()
        .center_y()
        .gap(2.0)
        .pt(4.0)
        .px(8.0)
        .pb(8.0)
        .child(attach(t, disabled))
        .child(chip("agent", crate::ui::menus::AGENTS[s.ui.agent]))
        .child(chip("model", crate::ui::menus::MODELS[s.ui.model]))
        .child(interrupt_button(t, s.can_interrupt()))
        .child(spacer())
        .child(send)
}

/// `toolbar-icon`: the paperclip. Opening the file picker is piece N.
fn attach(t: &Theme, disabled: bool) -> El {
    let mut b = el().key("attach").size(24.5, 24.5).center().radius(8.0).help("Attach files").label("Attach files");
    b = if disabled {
        b.opacity(0.5).disabled(true).child(icon(&PAPERCLIP, 12.25).color(t.muted))
    } else {
        b.hover(t.wash(6.0))
            .press(t.wash(10.0))
            .hover_group()
            .cursor(Cursor::Pointer)
            .on_press(|_: &mut State| {})
            .child(icon(&PAPERCLIP, 12.25).color(t.muted).group_color(t.text))
    };
    b
}

/// `toolbar-icon-danger`: muted, red on hover; off unless there is something to interrupt.
fn interrupt_button(t: &Theme, live: bool) -> El {
    let mut b = el().key("interrupt").size(24.5, 24.5).center().radius(8.0).help("Interrupt").label("Interrupt");
    b = if live {
        b.hover(t.error.alpha(0.1))
            .press(t.error.alpha(0.16))
            .hover_group()
            .cursor(Cursor::Pointer)
            .on_press(interrupt)
            .child(icon(&CIRCLE_X, 12.25).color(t.muted).group_color(t.error_text))
    } else {
        b.opacity(0.5).disabled(true).child(icon(&CIRCLE_X, 12.25).color(t.muted))
    };
    b
}
