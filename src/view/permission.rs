//! The card an agent's permission ask shows under the conversation. Ports `components/session/PermissionCard.vue`
//! (rows 3.1-3.11, 3.13, 3.16 and 4.1-4.4, 4.6 of `docs/parity/04-questions-and-permissions.md`). `asks::wording` says
//! what the words are and `asks::permission` what an ask holds; this file draws them. Piece B of the component wires
//! them to the server (`fleet::asks`) and gives the card its keys (`on_key`, the deny field's `on_input_key`).
use super::ask_card;
use crate::State;
use crate::asks::permission::PermissionAsk;
use crate::asks::wording::{DiffLine, LineKind, counts, diff_lines, dont_ask_again_wording, handoff_rows, level_name, permission_heading};
use crate::conv::tools::PermissionKind;
use crate::fleet::asks::Reply;
use crate::icons::*;
use crate::theme::{RADIUS_BTN, Theme, mono, sans};
use rune::{Cursor, El, Key, KeyEvent, Tx, col, el, icon, row, text, text_input};

/// `.pcard__lines`: the diff scrolls at 240px.
const LINES_MAX_H: f32 = 240.0;
/// A monospace character at 12px, to give every diff line the width of the longest so its tint reaches the end.
const CHAR_W: f32 = 7.2;

/// `.pcard` and what is in it, for one waiting ask.
pub fn card(t: &Theme, s: &State, ask: &PermissionAsk, first: bool) -> El {
    let heading = permission_heading(ask);
    // Cards after the first are keyed by their ask, so two waiting at once don't share a key. Their buttons still do:
    // the keys act on the first card (docs/parity/04, row 3.14).
    let key = if first { "permission-card".to_string() } else { format!("permission-card-{}", ask.id) };
    let mut card = ask_card::frame(t, key).label(format!("The agent asks: {heading}")).child(head(t, ask, &heading));
    card = match &ask.handoff {
        Some(handoff) => card.child(handoff_block(t, handoff)),
        None => match body(t, ask) {
            Some(body) => card.child(body),
            None => card,
        },
    };
    card = card.child(choices(t, s, ask));
    if let Some(error) = s.ask_act(&ask.id).error {
        // `.pcard__error`, role alert: Fleet's own reason, or that the answer didn't get there.
        card = card.child(text(error).key("permission-error").label("alert").font(sans(12.0, 400.0, 1.4)).color(t.error_text).wrap());
    }
    card.child(foot(t, ask))
}

/// `.pcard__head`: the shield, the heading, who asked, and "Needs you" at the far end.
fn head(t: &Theme, ask: &PermissionAsk, heading: &str) -> El {
    let mut head = row().w_full().gap(8.0).center_y().child(icon(&SHIELD_ALERT, 15.0).color(t.status_waiting())).child(
        text(heading.to_string()).key("permission-heading").font(sans(13.0, 600.0, 1.3)).color(t.text).truncate().shrink().min_w(0.0),
    );
    if ask.subagent.is_some() {
        head = head.child(
            row()
                .key("permission-subagent")
                .px(8.0)
                .radius(999.0)
                .border(1.0, t.border)
                .border_layout()
                .child(text("Subagent").font(sans(11.5, 400.0, 1.4)).color(t.muted)),
        );
    }
    head.child(el().grow(1.0)).child(ask_card::needs_you(t, "permission-needs-you"))
}

/// `.pcard__cmd`: the plain block, scrolling sideways.
fn block(t: &Theme, inner: El) -> El {
    el().w_full().px(12.0).py(9.0).radius(RADIUS_BTN).border(1.0, t.border).border_layout().fill(t.panel_bg).scroll_x().child(inner)
}

/// What the agent wants to touch: a command, a file and its changes, or any other line.
fn body(t: &Theme, ask: &PermissionAsk) -> Option<El> {
    if ask.kind == PermissionKind::Shell
        && let Some(command) = &ask.title
    {
        let mut inner = col().key("permission-cmd");
        if let Some(directory) = &ask.directory {
            inner = inner.child(text(directory.clone()).font(mono(11.5, 400.0, 1.5)).color(t.muted).mb(2.0));
        }
        let line = row()
            .child(text("$ ").font(mono(12.5, 400.0, 1.5)).color(t.muted))
            .child(text(command.clone()).font(mono(12.5, 400.0, 1.5)).color(t.text));
        return Some(block(t, inner.child(line)));
    }
    if ask.kind == PermissionKind::Edit {
        return Some(file(t, ask));
    }
    ask.title
        .as_ref()
        .map(|title| block(t, col().key("permission-cmd").child(text(title.clone()).font(mono(12.5, 400.0, 1.5)).color(t.text))))
}

/// `.pcard__diff`: the file line, then the changed lines when there are any (rows 3.5-3.7).
fn file(t: &Theme, ask: &PermissionAsk) -> El {
    let lines = diff_lines(ask.detail.as_deref());
    let (added, removed) = counts(&lines);
    let mut file_line = row().w_full().gap(8.0).px(12.0).py(6.0).child(
        text(ask.title.clone().unwrap_or_else(|| ask.tool.clone()))
            .key("permission-path")
            .font(mono(11.5, 400.0, 1.5))
            .color(t.muted)
            .truncate()
            .shrink()
            .min_w(0.0),
    );
    if added > 0 {
        file_line = file_line.child(text(format!("+{added}")).key("permission-add").font(mono(11.5, 400.0, 1.5)).color(t.diff_add_text()));
    }
    if removed > 0 {
        // A real minus sign, as Fleet writes it.
        file_line =
            file_line.child(text(format!("\u{2212}{removed}")).key("permission-del").font(mono(11.5, 400.0, 1.5)).color(t.diff_del_text()));
    }
    let mut diff = col()
        .key("permission-file")
        .w_full()
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.panel_bg)
        .clip_rounded(RADIUS_BTN)
        .child(file_line);
    if added + removed > 0 {
        diff = diff.child(lines_box(t, &lines));
    }
    diff
}

fn lines_box(t: &Theme, lines: &[DiffLine]) -> El {
    let widest = lines.iter().map(|l| l.text.chars().count()).max().unwrap_or(0);
    let width = widest as f32 * CHAR_W + 24.0;
    let mut rows = col().min_w(width).w_full();
    for line in lines {
        let (fill, ink) = match line.kind {
            LineKind::Add => (Some(t.diff_add().alpha(0.10)), t.text),
            LineKind::Del => (Some(t.diff_del().alpha(0.10)), t.text),
            LineKind::Hunk => (None, t.muted),
            LineKind::Context => (None, t.text),
        };
        // An empty line draws a space, so it keeps its height.
        let shown = if line.text.is_empty() { " ".to_string() } else { line.text.clone() };
        let mut one = row().w_full().px(12.0).child(text(shown).font(mono(12.0, 400.0, 1.5)).color(ink));
        if let Some(fill) = fill {
            one = one.fill(fill);
        }
        rows = rows.child(one);
    }
    el().key("permission-lines").w_full().max_h(LINES_MAX_H).border_sides([0.0, 1.0, 0.0, 0.0], t.border).scroll_xy().child(rows)
}

/// `.pcard__handoff` and `.pcard__note`: what goes to the other machine, under whose permission level (rows 4.2, 4.3).
fn handoff_block(t: &Theme, handoff: &crate::asks::permission::HandoffAsk) -> El {
    let mut labels = col().gap(4.0);
    let mut values = col().gap(4.0).grow(1.0).min_w(0.0);
    for (label, value) in handoff_rows(handoff) {
        labels = labels.child(text(label).font(sans(12.5, 400.0, 1.5)).color(t.muted));
        values = values.child(text(value).font(mono(12.5, 400.0, 1.5)).color(t.text).truncate());
    }
    let list = row()
        .key("permission-handoff")
        .w_full()
        .gap(14.0)
        .px(12.0)
        .py(9.0)
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.panel_bg)
        .child(labels)
        .child(values);
    let note = row()
        .key("permission-handoff-level")
        .w_full()
        .flex_wrap()
        .child(text(format!("It runs under {}'s permission level: ", handoff.machine)).font(sans(12.5, 400.0, 1.5)).color(t.muted))
        .child(text(level_name(handoff.level.as_deref())).font(sans(12.5, 600.0, 1.5)).color(t.text))
        .child(text(".").font(sans(12.5, 400.0, 1.5)).color(t.muted));
    col().w_full().gap(12.0).child(list).child(note)
}

/// `.pcard__choice`: a button-shaped row with its number chip.
fn choice_row(t: &Theme, busy: bool, ask: &PermissionAsk, key: &'static str, number: &'static str, reply: Reply) -> El {
    let ask = ask.clone();
    row()
        .key(key)
        .focusable(!busy)
        .opacity(if busy { 0.55 } else { 1.0 })
        .on_press(move |s: &mut State| s.answer_ask(&ask, reply))
        .w_full()
        .min_w(0.0)
        .gap(10.0)
        .px(10.0)
        .py(8.0)
        .center_y()
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.panel_bg)
        .cursor(Cursor::Pointer)
        .child(ask_card::chip(t, number.to_string()))
}

fn name(t: &Theme, words: String) -> El {
    text(words).font(sans(13.0, 500.0, 1.3)).color(t.text).truncate().shrink().min_w(0.0)
}

fn scope(t: &Theme, words: String) -> El {
    text(words).font(sans(12.0, 400.0, 1.3)).color(t.muted)
}

fn choices(t: &Theme, s: &State, ask: &PermissionAsk) -> El {
    let busy = s.ask_act(&ask.id).busy;
    let group = col().key("permission-choices").label("Answer").w_full().gap(4.0);
    let once = choice_row(t, busy, ask, "permission-allow-once", "1", Reply::Once).child(name(t, "Allow once".into()));
    if let Some(handoff) = &ask.handoff {
        let machine = &handoff.machine;
        return group
            .child(once)
            .child(
                choice_row(t, busy, ask, "permission-allow-always", "2", Reply::Always)
                    .child(name(t, format!("Always for {machine}")))
                    .child(el().grow(1.0))
                    .child(scope(t, format!("sets {machine} to Always"))),
            )
            .child(
                choice_row(t, busy, ask, "permission-handoff-never", "3", Reply::Reject)
                    .child(name(t, format!("Not {machine}")))
                    .child(el().grow(1.0))
                    .child(scope(t, format!("sets {machine} to Never"))),
            );
    }
    let wording = dont_ask_again_wording(ask);
    let lead = text(wording.lead).font(sans(13.0, 500.0, 1.3)).color(t.text);
    let mut always_name = row().gap(0.0).center_y().shrink().min_w(0.0).child(lead);
    if let Some(code) = wording.code {
        always_name = always_name.child(
            row()
                .key("permission-always-code")
                .ml(4.0)
                .px(4.0)
                .radius(4.0)
                .fill(t.wash(6.0))
                .shrink()
                .min_w(0.0)
                .child(text(code).font(mono(12.0, 400.0, 1.5)).color(t.text).truncate().shrink().min_w(0.0)),
        );
    }
    let always = choice_row(t, busy, ask, "permission-allow-always", "2", Reply::Always)
        .child(always_name)
        .child(el().grow(1.0))
        .child(scope(t, "this session".into()));
    group.child(once).child(always).child(deny_row(t, s, ask, busy))
}

/// `.pcard__choice--deny`: the words for the agent, and the Deny pill (row 3.10).
fn deny_row(t: &Theme, s: &State, ask: &PermissionAsk, busy: bool) -> El {
    let hint = "Deny, and tell the agent what to do instead…";
    let field = text_input(&s.deny, sans(13.0, 400.0, 1.3))
        .key("permission-deny-input")
        .label("Deny, with an optional message for the agent")
        .placeholder(hint, t.muted)
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .single_line()
        .grow(1.0)
        .min_w(0.0)
        .on_edit(|s: &mut State| &mut s.deny)
        .disabled(busy)
        .on_input_key({
            let ask = ask.clone();
            move |s: &mut State, ev: &KeyEvent, _: &mut Tx| {
                // Enter and Esc deny with the words typed; anything else is the field's.
                let denies = !ev.mods.cmd && !ev.mods.alt && !ev.mods.ctrl && matches!(ev.key, Key::Enter | Key::Escape);
                if denies {
                    s.answer_ask(&ask, Reply::Reject);
                }
                denies
            }
        });
    let denied = ask.clone();
    let pill = el()
        .key("permission-deny")
        .h(24.0)
        .px(10.0)
        .center()
        .radius(999.0)
        .hover(t.error.alpha(0.10))
        .hover_group()
        .cursor(Cursor::Pointer)
        .focusable(!busy)
        .on_press(move |s: &mut State| s.answer_ask(&denied, Reply::Reject))
        .child(text("Deny").font(sans(12.0, 400.0, 1.3)).color(t.muted).group_color(t.error_text));
    // Clicking the row (not the pill) puts the caret in the field; Enter there denies.
    choice_row(t, busy, ask, "permission-deny-row", "3", Reply::Reject)
        .focusable(false)
        .on_press_tx(|_: &mut State, tx: &mut Tx| tx.focus("permission-deny-input", false))
        .cursor(Cursor::Text)
        .child(field)
        .child(pill)
}

/// `.pcard__foot`: the keys, the undo hint, and the way to Settings (rows 3.13, 4.6).
fn foot(t: &Theme, ask: &PermissionAsk) -> El {
    let small = |words: String| text(words).font(sans(11.0, 400.0, 1.4)).color(t.muted);
    let (settings, section, spans) = match &ask.handoff {
        Some(handoff) => (
            "Machine settings",
            "machines",
            vec![("", "1, 2 or 3 answers".to_string()), ("", format!("You can change it on {}'s card.", handoff.machine))],
        ),
        None => (
            "Permission settings",
            "permissions",
            vec![
                ("", "1 or 2 answers \u{b7} 3 then Enter denies \u{b7} Esc denies".to_string()),
                ("permission-undo-hint", "You can undo option 2 from Allowed in the composer.".to_string()),
            ],
        ),
    };
    let mut foot = row().w_full().flex_wrap().center_y().gap(8.0);
    for (key, words) in spans {
        foot = foot.child(if key.is_empty() { small(words) } else { small(words).key(key) });
    }
    // Underline on hover has no equivalent at Rune's pin; the link keeps its colour (docs/RUNE.md).
    foot.child(el().grow(1.0)).child(
        text(settings)
            .key("permission-settings")
            .font(sans(11.0, 400.0, 1.4))
            .color(t.accent_text)
            .cursor(Cursor::Pointer)
            .on_press_tx(move |s: &mut State, _: &mut Tx| s.settings_section = Some(section)),
    )
}

/// The elements of a card that can hold the keyboard: a key pressed while one of them has it is the card's
/// (`handleCardKeydown` is on the card, so only keys from inside it count).
const CARD_KEYS: [&str; 6] = [
    "permission-allow-once",
    "permission-allow-always",
    "permission-handoff-never",
    "permission-deny-input",
    "permission-deny",
    "permission-settings",
];

/// The choice buttons the arrows move between: the deny row is a field, not a button, so it is left out.
fn buttons(ask: &PermissionAsk) -> &'static [&'static str] {
    if ask.handoff.is_some() {
        &["permission-allow-once", "permission-allow-always", "permission-handoff-never"]
    } else {
        &["permission-allow-once", "permission-allow-always"]
    }
}

/// The window's key handler, for a key while a card holds the keyboard (rows 3.14 and 4.5). True when it took it.
pub fn on_key(s: &mut State, ev: &KeyEvent, tx: &mut Tx) -> bool {
    let Some(ask) = s.open_asks().first().cloned() else { return false };
    let Some(focused) = tx.focused().filter(|k| CARD_KEYS.contains(k)).map(str::to_string) else { return false };
    if ev.mods.cmd || ev.mods.alt || ev.mods.ctrl || ev.mods.logo {
        return false;
    }
    if let Some(step) = crate::asks::focus::step_of(&ev.key) {
        // Arrows move only from a choice button, and not at all from the deny field.
        return match crate::asks::focus::next_choice(buttons(&ask), &focused, step) {
            Some(next) => {
                tx.focus(next, true);
                true
            }
            None => false,
        };
    }
    let handoff = ask.handoff.is_some();
    let reply = match &ev.key {
        Key::Escape if handoff => return true,
        Key::Escape => Some(Reply::Reject),
        Key::Text(t) if t == "1" => Some(Reply::Once),
        Key::Text(t) if t == "2" => Some(Reply::Always),
        Key::Text(t) if t == "3" && handoff => Some(Reply::Reject),
        Key::Text(t) if t == "3" => {
            tx.focus("permission-deny-input", false);
            return true;
        }
        // Enter on a focused choice presses it.
        Key::Enter => match focused.as_str() {
            "permission-allow-once" => Some(Reply::Once),
            "permission-allow-always" => Some(Reply::Always),
            "permission-handoff-never" | "permission-deny" => Some(Reply::Reject),
            _ => None,
        },
        _ => None,
    };
    match reply {
        Some(reply) => {
            s.answer_ask(&ask, reply);
            true
        }
        None => false,
    }
}
