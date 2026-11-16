//! The New session page's message box (`NewSessionComposer.vue`'s `.new-session__frame`, `ComposerFrame.vue`; rows A4, A5,
//! A14.1): the frame, the field, and the toolbar with the harness, profile, agent and model chips and Send. The sent message
//! that stands in for the stage while a session starts is `conversation` (A1.12-A1.14). `@`, images and `#` are piece M's.
use super::conversation::COLUMN;
use super::conversation::{Message, Part};
use crate::icons::ARROW_UP;
use crate::theme::{Theme, sans};
use crate::{State, Tx};
use rune::{Color, Cursor, El, Shadow, TextEdit, auto_grow, col, el, icon, row, spacer, text_input};

/// `.composer-frame` with the field above and the toolbar below.
pub fn frame(s: &State, t: &Theme) -> El {
    let starting = s.new_session_starting();
    let placeholder = placeholder(s);
    // While it starts the field is blank and read-only; the words are in the conversation (`:value` is "" then).
    let blank = TextEdit::new("");
    let shown = if starting && s.sent_message().is_some() { &blank } else { &s.new_box.edit };
    let mut field = text_input(shown, sans(14.0, 400.0, 1.5))
        .key(crate::fleet::new_session::send::MESSAGE_KEY)
        .label("First message")
        .placeholder(placeholder, t.muted)
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .w_full()
        .pt(14.0)
        .px(16.0)
        .pb(6.0)
        .min_h(64.0)
        .disabled(starting);
    if !starting {
        field = field.on_edit(|s: &mut State| &mut s.new_box.edit).on_submit(|s: &mut State, tx| s.submit_new_session(tx, false));
    }
    let mut toolbar = row().key("new-session-toolbar").w_full().center_y().gap(2.0).pt(9.0).px(8.0).pb(8.0);
    toolbar = toolbar.children(super::new_session_strip::toolbar_chips(s, t)).child(spacer()).child(send_button(s, t));
    col()
        .key("new-session-frame")
        .w_full()
        .max_w(COLUMN)
        .radius(14.0)
        .fill(t.card_bg)
        .border(1.0, t.border)
        .box_shadow(Shadow { x: 0.0, y: 10.0, blur: 28.0, spread: -18.0, color: Color::BLACK.alpha(0.5), inset: false })
        .focus_within_ring(1.0, -1.0, t.accent_border(55.0))
        .child(auto_grow(field, 2, 7))
        .child(toolbar)
}

/// A4.6-A4.8.
fn placeholder(s: &State) -> &'static str {
    let Some(d) = s.new_draft.draft() else { return "Describe the task, or ask a question…" };
    if d.github_preset.is_some() {
        "Add instructions (optional). The issue is attached."
    } else if d.folder == Some(crate::fleet::new_session::request::Folder::None) {
        "Ask anything…"
    } else {
        "Describe the task, or ask a question…"
    }
}

/// `.composer-frame__send`: 30 by 30, the arrow, a spinner while the session starts, half faded when it can't send.
fn send_button(s: &State, t: &Theme) -> El {
    let starting = s.new_session_starting();
    let can = s.can_send_new_session();
    let button = el()
        .key("create-session-submit")
        .role("button")
        .size(30.0, 30.0)
        .center()
        .radius(15.0)
        .fill(t.accent)
        .opacity(if can || starting { 1.0 } else { 0.5 })
        .help("Start session (Enter)")
        .label("Start session")
        .child(if starting {
            icon(&crate::icons::LOADER_CIRCLE, 14.0).color(t.primary_fg).cycle(super::ring_popover::spinner())
        } else {
            icon(&ARROW_UP, 14.0).color(t.primary_fg)
        });
    if can {
        button
            .hover(Color::BLACK.alpha(0.1))
            .cursor(Cursor::Pointer)
            .on_press_tx(|s: &mut State, tx: &mut Tx| s.submit_new_session(tx, false))
    } else {
        button.disabled(true)
    }
}

/// `.new-session__conversation`: the message just sent, where the session page will show it (A1.12-A1.14).
pub fn conversation(s: &State, t: &Theme, message: &str, sent_at: i64) -> El {
    let md = super::conversation::styles(t);
    let mut sent = Message::user("new-session-sent".to_string(), message.to_string());
    sent.meta.created_at = Some(sent_at);
    let body = match sent.parts.first() {
        Some(Part::Text { id, .. }) => id.clone(),
        _ => String::new(),
    };
    col()
        .key("new-session-conversation")
        .label("Activity stream")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .items_center()
        .px(32.0)
        .pt(24.0)
        .pb(12.0)
        .child(col().w_full().max_w(COLUMN).mb(20.0).child(super::message::user_bubble(t, s, &md, &sent, &body, message)))
}
