//! The card an agent's permission ask shows under the conversation. Ports `components/session/PermissionCard.vue`
//! (rows 3.1-3.11, 3.13, 3.16 and 4.1-4.4, 4.6 of `docs/parity/04-questions-and-permissions.md`). `asks::wording` says
//! what the words are and `asks::permission` what an ask holds; this file draws them. The answer buttons do nothing
//! yet: piece B wires them to the server, with the keys.
use super::parts::diamond;
use super::toolcard::mix;
use crate::State;
use crate::asks::permission::PermissionAsk;
use crate::asks::wording::{DiffLine, LineKind, counts, diff_lines, dont_ask_again_wording, handoff_rows, level_name, permission_heading};
use crate::conv::tools::PermissionKind;
use crate::icons::*;
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, mono, sans};
use rune::{Cursor, El, Tx, col, el, icon, row, text, text_input};

/// `.pcard__lines`: the diff scrolls at 240px.
const LINES_MAX_H: f32 = 240.0;
/// A monospace character at 12px, to give every diff line the width of the longest so its tint reaches the end.
const CHAR_W: f32 = 7.2;

/// `.pcard` and what is in it, for one waiting ask.
pub fn card(t: &Theme, s: &State, ask: &PermissionAsk) -> El {
    let heading = permission_heading(ask);
    let mut card = col()
        .key("permission-card")
        .label(format!("The agent asks: {heading}"))
        .w_full()
        .mt(10.0)
        .padding(12.0)
        .gap(12.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border_mix(t.status_waiting(), 40.0))
        .border_layout()
        .fill(mix(t.card_bg, t.status_waiting(), 0.05))
        .child(head(t, ask, &heading));
    card = match &ask.handoff {
        Some(handoff) => card.child(handoff_block(t, handoff)),
        None => match body(t, ask) {
            Some(body) => card.child(body),
            None => card,
        },
    };
    card = card.child(choices(t, s, ask));
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
    head.child(el().grow(1.0)).child(
        row()
            .key("permission-needs-you")
            .gap(6.0)
            .center_y()
            .child(diamond(t.status_waiting()))
            .child(text("Needs you").font(sans(12.0, 600.0, 1.3)).color(t.status_waiting_text)),
    )
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

/// `.pcard__key`: an 18px chip with the number that answers.
fn chip(t: &Theme, number: &'static str) -> El {
    el().size(18.0, 18.0)
        .center()
        .radius(5.0)
        .border(1.0, t.border)
        .border_layout()
        .child(text(number).font(mono(11.0, 400.0, 1.0)).color(t.muted))
}

/// `.pcard__choice`: a button-shaped row with its number chip.
fn choice_row(t: &Theme, key: &'static str, number: &'static str) -> El {
    row()
        .key(key)
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
        .child(chip(t, number))
}

fn name(t: &Theme, words: String) -> El {
    text(words).font(sans(13.0, 500.0, 1.3)).color(t.text).truncate().shrink().min_w(0.0)
}

fn scope(t: &Theme, words: String) -> El {
    text(words).font(sans(12.0, 400.0, 1.3)).color(t.muted)
}

fn choices(t: &Theme, s: &State, ask: &PermissionAsk) -> El {
    let group = col().key("permission-choices").label("Answer").w_full().gap(4.0);
    let once = choice_row(t, "permission-allow-once", "1").child(name(t, "Allow once".into()));
    if let Some(handoff) = &ask.handoff {
        let machine = &handoff.machine;
        return group
            .child(once)
            .child(
                choice_row(t, "permission-allow-always", "2")
                    .child(name(t, format!("Always for {machine}")))
                    .child(el().grow(1.0))
                    .child(scope(t, format!("sets {machine} to Always"))),
            )
            .child(
                choice_row(t, "permission-handoff-never", "3")
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
    let always =
        choice_row(t, "permission-allow-always", "2").child(always_name).child(el().grow(1.0)).child(scope(t, "this session".into()));
    group.child(once).child(always).child(deny_row(t, s))
}

/// `.pcard__choice--deny`: the words for the agent, and the Deny pill (row 3.10).
fn deny_row(t: &Theme, s: &State) -> El {
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
        .on_edit(|s: &mut State| &mut s.deny);
    let pill = el()
        .key("permission-deny")
        .h(24.0)
        .px(10.0)
        .center()
        .radius(999.0)
        .hover(t.error.alpha(0.10))
        .hover_group()
        .cursor(Cursor::Pointer)
        .child(text("Deny").font(sans(12.0, 400.0, 1.3)).color(t.muted).group_color(t.error_text));
    choice_row(t, "permission-deny-row", "3").cursor(Cursor::Text).child(field).child(pill)
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
