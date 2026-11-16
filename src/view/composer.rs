//! The composer: the frame, the text area, the error box and the toolbar. Ports `Composer.vue` and
//! `ComposerFrame.vue` (docs/parity/05-composer.md, piece B). What goes in the toolbar's other slots
//! (the effort toggle, the Allowed chip, the ring, Send now and Queue) comes with the pieces that build
//! them; they sit where Fleet puts them: agent chip, model chip, [effort], [Allowed], Interrupt, [ring],
//! [Send now / Queue], Send.
use super::conversation::COLUMN;
use crate::composer::draftrefs::RefKind;
use crate::composer::status::{ComposerStatus, Mode, has_content, mode_of, send_words};
use crate::icons::*;
use crate::mock::Session;
use crate::theme::{Theme, mono, sans};
use crate::{State, interrupt, submit};
use rune::{Color, Cursor, El, Key, Shadow, TextHighlight, auto_grow, col, el, icon, row, spacer, text, text_input};

/// Fleet puts the composer's pieces in this order above the frame: the error box, the background strip
/// (component 3, piece K, `view::work`), the mod band (component 22), the queue (piece I) and the paste offer (piece O).
/// The mod band is drawn empty until component 22.
pub fn composer(s: &State, t: &Theme, _session: &Session) -> El {
    let disabled = s.composer_disabled();
    let mode = mode_of(&s.draft.text, &s.caps(), s.side_open);
    let mut outer = col().key("composer-section").label("Message composer").w_full().pt(4.0).px(24.0).pb(18.0).items_center();
    // The first of the send errors, then the pasted-file one (`sendError || pasteError`).
    if let Some(error) = s.composer_error.as_deref().or(s.attach_error()) {
        outer = outer.child(error_box(t, "send-prompt-error", error));
    }
    if let Some(strip) = super::work::strip(s, t) {
        outer = outer.child(strip);
    }
    if let Some(queue) = super::queued::list(s, t) {
        outer = outer.child(queue);
    }
    outer.children(super::attachments::paste_offer(s, t)).child(frame(s, t, disabled, mode))
}

/// `.composer-error`: the first thing that went wrong, in a box above the frame (`role=alert`).
pub fn error_box(t: &Theme, key: &'static str, message: &str) -> El {
    col()
        .key(key)
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
    let edge = if s.dragging && !disabled {
        t.accent
    } else if mode == Mode::Prompt {
        t.border
    } else {
        t.accent_border(70.0)
    };
    let placeholder = if mode == Mode::Side { "Ask a follow-up in the side conversation…" } else { "Type a message…" };
    // 14 + one 21px line + 25 is the 60px the empty field measures in Fleet; seven lines is about its 180px maximum.
    // Shell mode sets the draft in the terminal's type, 13px (row 9.2).
    let font = if mode == Mode::Shell { mono(13.0, 400.0, 1.5) } else { sans(14.0, 400.0, 1.5) };
    let field = text_input(&s.draft, font)
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
        .on_submit(|s: &mut State, tx| submit(s, tx, false))
        // Return is the field's own submit; Ctrl/Cmd+Return is its other choice (steer where Enter queues, queue where
        // it steers). The field takes the key before the frame does, so it is asked here.
        // A picture on the clipboard is attached; anything else pastes as text (row 10.1).
        .on_input_paste(|s: &mut State, content, tx| s.paste_files(content, tx))
        .on_input_key(|s: &mut State, ev: &rune::KeyEvent, tx| {
            // The large-paste bar's keys run before the text area's own (row 11.9).
            if s.offer_key(ev, tx) {
                return true;
            }
            // The history list (ArrowUp in an empty draft) and Esc in shell mode come before the rest (rows 8.2, 8.4, 9.4).
            if s.history_key(ev, tx) {
                return true;
            }
            // The popup, when it is open, takes the arrows, Enter, Tab and Esc first (row 5.13).
            if s.autocomplete_key(ev, tx) {
                return true;
            }
            // Backspace, Delete and the arrows take a picked reference as one piece (row 7.3).
            if s.reference_key(ev) {
                return true;
            }
            if ev.key == Key::Enter && (ev.mods.cmd || ev.mods.ctrl) {
                submit(s, tx, true);
                return true;
            }
            false
        });
    let mut frame = col()
        .key("composer-frame")
        .w_full()
        .max_w(COLUMN)
        .radius(14.0)
        .fill(if s.dragging && !disabled { t.card_mix(t.accent, 5.0) } else { t.card_bg })
        .border(1.0, edge)
        .box_shadow(Shadow { x: 0.0, y: 10.0, blur: 28.0, spread: -18.0, color: Color::BLACK.alpha(0.5), inset: false })
        .on_layout(|s: &mut State, [_, _, w, _]| s.pickers.composer_w = w)
        // A file dropped on the frame is attached (row 1.4): `--dragging` is the accent border over a 5% wash.
        .on_drop_files(|s: &mut State, files, tx| {
            s.dragging = false;
            s.add_files(files, tx);
        })
        .on_drop_hover(|s: &mut State, over, _| s.dragging = over)
        .focus_within_ring(1.0, -1.0, if mode == Mode::Prompt { focus } else { edge });
    frame = frame.child(auto_grow(field.text_highlights(reference_tints(s, t)), 1, 7));
    frame = frame
        .children(super::attachments::strip(s, t))
        .children(super::attachments::terminal_strip(s, t))
        .child(toolbar(s, t, disabled, mode));
    frame
}

/// `.composer-reference`: each reference is tinted behind the text, widened 3 px each side without moving the text. `@path`
/// and `@folder/` are the accent at 28%, a session 18%, a skill that goes with the message `--running` at 22% and one
/// that goes as a note `--idle` at 22% (rows 7.1, 7.4).
fn reference_tints(s: &State, t: &Theme) -> Vec<TextHighlight> {
    s.draft_references()
        .into_iter()
        .map(|span| {
            let tint = match span.kind {
                RefKind::File | RefKind::Folder => t.accent.alpha(0.28),
                RefKind::Session => t.accent.alpha(0.18),
                RefKind::Skill { note: false } => t.running.alpha(0.22),
                RefKind::Skill { note: true } => t.idle.alpha(0.22),
            };
            TextHighlight::new(span.range, tint).pad(3.0)
        })
        .collect()
}

/// `.composer-toolbar` agent or model chip: the label and a chevron that turns over while its menu is open. The New session
/// page's toolbar draws the same chips.
pub fn selector_chip(t: &Theme, open: Option<crate::ui::Open>, key: &'static str, chip: crate::fleet::catalog::Chip) -> El {
    let name: &'static str = if key == "agent" { "agent" } else { "model" };
    let expanded = open == Some(crate::ui::Open::Menu(name));
    let mut b = row()
        .key(key)
        .role("button")
        .label(if key == "agent" { "Agent selector" } else { "Model selector" })
        .h(28.0)
        .px(8.0)
        .gap(5.0)
        .center_y()
        .radius(8.0)
        .hover(t.wash(6.0))
        .focus_ring(2.0, 2.0, t.accent)
        .cursor(Cursor::Pointer)
        .shrink()
        .min_w(0.0)
        .on_press(move |s: &mut State| s.open_selector(name))
        .child(text(chip.label).font(sans(12.0, 400.0, 1.2)).color(t.text.alpha(0.8)).truncate().shrink())
        // The chevron turns over while the menu is open.
        .child(icon(&CHEVRON_DOWN, 13.0).color(t.muted).rotate(if expanded { std::f32::consts::PI } else { 0.0 }));
    if expanded {
        b = b.fill(t.wash(6.0));
    }
    // The narrow rule shortened the label: the whole one is the tooltip (row 3.2).
    if let Some(full) = chip.help {
        b = b.help(full);
    }
    b
}

fn toolbar(s: &State, t: &Theme, disabled: bool, mode: Mode) -> El {
    let open = s.ui.open;
    let chip = |key: &'static str, chip: crate::fleet::catalog::Chip| selector_chip(t, open, key, chip);
    let inactive = disabled || s.uploading || !has_content(&s.draft.text, mode == Mode::Shell, s.shell_support_pending(), s.extras());
    let busy = s.composer_status() == ComposerStatus::Busy;
    let caps = s.caps();
    let (label, title) = send_words(mode, busy, caps.steers_by_default);
    let mut send = el()
        .key("prompt-send-button")
        .size(30.0, 30.0)
        .center()
        .radius(15.0)
        .fill(t.accent)
        .opacity(if inactive { 0.5 } else { 1.0 })
        .help(title)
        .label(label)
        .child(if s.uploading {
            icon(&LOADER_CIRCLE, 14.0).color(t.primary_fg).cycle(super::ring_popover::spinner())
        } else {
            icon(&ARROW_UP, 14.0).color(t.primary_fg)
        });
    // While a turn runs and the harness can take a message into it, the other choice of Enter gets a button too.
    let other = if busy && caps.can_steer && mode == Mode::Prompt && !is_side_draft(&s.draft.text) {
        Some(other_button(t, caps.steers_by_default, inactive))
    } else {
        None
    };
    send = if inactive {
        send.disabled(true)
    } else {
        send.hover(Color::BLACK.alpha(0.1)).cursor(Cursor::Pointer).on_press_tx(|s: &mut State, tx| submit(s, tx, false))
    };
    // Shell mode shows its own chip instead of the buttons up to Interrupt; side mode a pill in place of the paperclip.
    let mut bar = row().w_full().center_y().gap(2.0).pt(4.0).px(8.0).pb(8.0);
    bar = match mode {
        Mode::Shell => bar.child(shell_mode(t)),
        _ => bar
            .child(if mode == Mode::Side { side_mode(t) } else { attach(t, disabled) })
            .child(chip("agent", s.agent_chip()))
            .child(chip("model", s.model_chip()))
            .children(if disabled { None } else { s.effort_view().map(|v| effort_toggle(t, &v)) })
            .children(if mode == Mode::Side { None } else { super::allowed::chip(s, t) }),
    };
    bar.child(interrupt_button(t, s.can_interrupt())).child(spacer()).children(crate::view::ring::button(s, t)).children(other).child(send)
}

/// `.composer-shell-mode`: what a `!` draft does, in place of the pickers.
fn shell_mode(t: &Theme) -> El {
    row()
        .key("composer-shell-mode")
        .label("Shell command")
        .gap(6.0)
        .px(6.0)
        .center_y()
        .shrink()
        .min_w(0.0)
        .child(icon(&SQUARE_TERMINAL, 14.0).color(t.accent))
        .child(text("Shell command").font(sans(12.0, 500.0, 1.5)).color(t.text))
        .child(
            text("Runs in the session's folder, no model turn · Esc for a prompt")
                .font(sans(12.0, 400.0, 1.5))
                .color(t.muted)
                .truncate()
                .shrink()
                .min_w(0.0),
        )
}

/// `.composer-side-mode`: the pill that says the draft goes to the side conversation, not the session.
fn side_mode(t: &Theme) -> El {
    row()
        .key("composer-side-mode")
        .label("btw")
        .help("Goes to the side conversation above, not the session. Close it to talk to the session again.")
        .h(24.0)
        .px(8.0)
        .gap(5.0)
        .center_y()
        .radius(crate::theme::RADIUS_BTN)
        .fill(t.accent_dim)
        .child(icon(&MESSAGE_CIRCLE_QUESTION_MARK, 13.0).color(t.accent_text))
        .child(text("btw").font(sans(12.0, 600.0, 1.5)).color(t.accent_text))
}

/// A `/btw` question never waits for the turn, so it is offered no Send now (`offerSendNowButton`).
fn is_side_draft(text: &str) -> bool {
    crate::composer::route::parse_side_question(text.trim()).is_some()
}

/// `composer-send-now`: Ctrl+Enter's button. Where Enter steers it reads "Queue", elsewhere "Send now". Outline, 32 px tall.
fn other_button(t: &Theme, steers_by_default: bool, inactive: bool) -> El {
    let (key, name, title, glyph) = if steers_by_default {
        ("prompt-queue-button", "Queue", "Queue (Ctrl+Enter): sent when the agent finishes", &LIST_END)
    } else {
        (
            "prompt-send-now-button",
            "Send now",
            "Send now (Ctrl+Enter): the agent reads it at its next step, without waiting for the turn to end",
            &CORNER_DOWN_RIGHT,
        )
    };
    let b = row()
        .key(key)
        .role("button")
        .label(name)
        .help(title)
        .h(32.0)
        .px(10.0)
        .gap(6.0)
        .center_y()
        .radius(crate::theme::RADIUS_BTN)
        .border(1.0, t.border)
        .fill(t.panel_bg)
        .opacity(if inactive { 0.5 } else { 1.0 })
        .child(icon(glyph, 12.25).color(t.text))
        .child(text(name).font(sans(12.0, 500.0, 1.0)).color(t.text));
    if inactive {
        b.disabled(true)
    } else {
        b.hover(t.accent.alpha(0.1)).cursor(Cursor::Pointer).on_press_tx(|s: &mut State, tx| submit(s, tx, true))
    }
}

/// `toolbar-icon`: the paperclip, which opens the file picker.
fn attach(t: &Theme, disabled: bool) -> El {
    let mut b = el().key("attach").size(24.5, 24.5).center().radius(8.0).help("Attach files").label("Attach files");
    b = if disabled {
        b.opacity(0.5).disabled(true).child(icon(&PAPERCLIP, 12.25).color(t.muted))
    } else {
        b.hover(t.wash(6.0))
            .press(t.wash(10.0))
            .hover_group()
            .cursor(Cursor::Pointer)
            .on_press_tx(|s: &mut State, tx| s.pick_files(tx))
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

/// `EffortToggle.vue`: the level's name and one dot per level the model offers, filled up to the current one; a click
/// goes to the next level (row 4.2). Not drawn in shell mode (row 4.7).
fn effort_toggle(t: &Theme, view: &crate::fleet::catalog::EffortView) -> El {
    let filled = view.variants.iter().position(|v| *v == view.level).map_or(0, |i| i + 1);
    let mut dots = row().gap(2.0).center_y();
    for i in 0..view.variants.len() {
        dots = dots.child(el().size(7.0, 7.0).radius(3.5).fill(if i < filled { t.accent } else { t.muted.alpha(0.35) }));
    }
    let mut b = row()
        .key("effort")
        .h(28.0)
        .px(8.0)
        .gap(7.0)
        .center_y()
        .radius(8.0)
        .hover(t.wash(6.0))
        .cursor(Cursor::Pointer)
        .shrink()
        .help(view.help())
        .label(format!("Reasoning effort: {}", crate::composer::effort::effort_label(&view.level)))
        .on_press(|s: &mut State| s.cycle_effort());
    if let Some(note) = &view.note {
        b = b.child(text(note.clone()).font(sans(11.0, 400.0, 1.2)).color(t.accent_text));
    }
    b.child(text(crate::composer::effort::effort_label(&view.level)).font(sans(12.0, 400.0, 1.2)).color(t.text.alpha(0.8))).child(dots)
}
