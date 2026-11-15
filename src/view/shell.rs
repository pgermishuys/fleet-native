//! A command the user ran from the composer (`!git status`) and what it printed: one block on the user's side of the
//! conversation. Ports `components/session/ShellCommandBlock.vue`; its logic is `conv::shell`.
use super::toolcard::mix;
use crate::State;
use crate::conv::shell::{COLLAPSED_LINES, ShellCommand, ShellState, output_lines, shown_range, state_label, went_wrong};
use crate::icons::*;
use crate::theme::{RADIUS_BTN, RADIUS_PANEL, Theme, mono, sans};
use rune::{Cursor, El, ScrollSpec, Span, col, el, icon, rich, row, text};

/// The grey text button above and below the output ("Show 28 earlier lines").
fn toggle(t: &Theme, key: String, glyph: &'static rune::Icon, label: String, on_press: impl Fn(&mut State) + 'static) -> El {
    row()
        .key(key)
        .gap(4.0)
        .center_y()
        .mt(6.0)
        .cursor(Cursor::Pointer)
        .hover_group()
        .label(label.clone())
        .on_press(on_press)
        .child(icon(glyph, 12.0).color(t.muted).group_color(t.text))
        .child(text(label).font(sans(12.0, 400.0, 1.3)).color(t.muted).group_color(t.text))
}

pub fn shell_block(t: &Theme, s: &State, c: &ShellCommand) -> El {
    let id = c.id.as_str();
    let wrong = went_wrong(c.state);
    let expanded = s.shell_wide.contains(id);
    let lines = output_lines(&c.output);
    let shown = shown_range(lines.len(), expanded);
    let font = mono(12.0, 400.0, 1.55);

    let mut head = row()
        .w_full()
        .gap(6.0)
        .center_y()
        .mb(6.0)
        .child(icon(&SQUARE_TERMINAL, 13.0).color(t.muted))
        .child(text("You ran").font(sans(12.0, 500.0, 1.3)).color(t.text));
    let label = state_label(c);
    if !label.is_empty() {
        head = head.child(el().grow(1.0)).child(
            text(label).key(format!("shell-{id}-state")).font(sans(12.0, 400.0, 1.3).features("'tnum' 1")).color(if wrong {
                t.error_text
            } else {
                t.muted
            }),
        );
    }
    // "$ " is muted in Fleet and not selectable; here it is muted and part of the text.
    let command = format!("$ {}", c.command);
    let mut block = col()
        .key(format!("shell-{id}"))
        .w_frac(0.82)
        .px(10.0)
        .pt(8.0)
        .pb(10.0)
        .corners([RADIUS_PANEL + 2.0, RADIUS_PANEL + 2.0, 4.0, RADIUS_PANEL + 2.0])
        .border(1.0, if wrong { mix(t.border, t.error, 0.4) } else { t.border })
        .border_layout()
        .fill(t.wash(5.0))
        .label("Shell command you ran")
        .child(head)
        .child(
            rich(command, vec![Span::new(0, 2).color(t.muted)])
                .key(format!("shell-{id}-command"))
                .font(font)
                .color(t.text)
                .wrap()
                .selectable(true),
        );
    if shown.start > 0 {
        let key = id.to_string();
        block = block.child(toggle(
            t,
            format!("shell-{id}-expand"),
            &CHEVRON_UP,
            format!("Show {} earlier {}", shown.start, if shown.start == 1 { "line" } else { "lines" }),
            move |s: &mut State| {
                s.shell_wide.insert(key.clone());
            },
        ));
    }
    if !lines.is_empty() {
        block = block.child(
            el().key(format!("shell-{id}-output"))
                .w_full()
                .mt(6.0)
                .px(10.0)
                .py(8.0)
                .radius(RADIUS_BTN)
                .border(1.0, t.border)
                .border_layout()
                .fill(t.main_bg.alpha(0.6))
                .max_h(480.0)
                .scroll(ScrollSpec { stick_bottom: false })
                .child(text(lines[shown.clone()].join("\n")).font(font).color(t.muted).wrap().selectable(true)),
        );
    } else if c.state != ShellState::Running {
        block = block.child(text("No output").key(format!("shell-{id}-none")).font(sans(12.0, 400.0, 1.3)).color(t.muted).mt(6.0));
    }
    if lines.len() > COLLAPSED_LINES && expanded {
        let key = id.to_string();
        block = block.child(toggle(
            t,
            format!("shell-{id}-collapse"),
            &CHEVRON_DOWN,
            format!("Show only the last {COLLAPSED_LINES} lines"),
            move |s: &mut State| {
                s.shell_wide.remove(&key);
            },
        ));
    }
    if c.truncated {
        block = block.child(
            text("The harness kept only part of the output.")
                .key(format!("shell-{id}-truncated"))
                .font(sans(12.0, 400.0, 1.3))
                .color(t.muted)
                .mt(6.0),
        );
    }
    // On the user's side: pushed to the right, 82% of the column.
    row().w_full().justify_end().child(block)
}
