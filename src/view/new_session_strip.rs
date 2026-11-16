//! Ports the chips under the New session box (`NewSessionComposer.vue`'s `.new-session__strip`, rows A11, B11.17): the
//! machine chip, a separator, the folder chip, and the workspace, base and "…" chips as their pieces land. Each chip is
//! `ui::chip::Chip`; what a chip opens is its picker's piece (the toast says which until it is built).
use crate::theme::{Theme, mono, sans};
use crate::ui::chip::Chip;
use crate::{State, Tx};
use rune::{El, el, row, text};

/// `.new-session__strip-separator`: 1 by 14, `--border`, 3px either side.
fn separator(t: &Theme) -> El {
    el().w(1.0).h(14.0).mx(3.0).fill(t.border)
}

/// The machine chip (`MachinePicker.vue`): a 7px dot in `--running` (`--error` when it can't be reached), the machine's name
/// in the UI font at 13px, a chevron. Its menu is piece K's.
fn machine(s: &State, t: &Theme) -> El {
    let down = s.target_is_down();
    let chip = Chip::new("new-session-machine", s.target_name())
        .dot(if down { t.error } else { t.running })
        .label_font(sans(13.0, 500.0, 1.2))
        .disabled(starting(s));
    let chip = if down { chip.ink(t.error_text) } else { chip };
    chip.view(t, |s: &mut State, tx: &mut Tx| {
        s.toast("The machine menu isn't in the native app yet (component 7, piece K).".into(), None, 4.0, tx);
    })
}

fn starting(s: &State) -> bool {
    s.new_draft.draft().is_some_and(|d| d.is_starting)
}

/// `.new-session__strip`: wraps, 2px apart, `6px 4px 0`.
pub fn strip(s: &State, t: &Theme) -> El {
    let chips = s.strip_chips();
    let mut r = row().key("new-session-strip").w_full().wrap().items_center().gap(2.0).px(4.0).pt(6.0);
    if chips.machine {
        r = r.child(machine(s, t)).child(separator(t));
    }
    r = r.child(super::folder_menu::trigger(s, t));
    if chips.workspace {
        r = r.child(separator(t)).child(super::workspace_picker::chip(s, t));
    }
    if chips.base {
        r = r.child(super::base_picker::chip(s, t));
    }
    r
}

/// `.new-session__plan`: what Enter will do, in one muted 12px line under the chips (`min-height: 22px`, `4px 11px 0`); a path
/// or branch is mono 11.5px in `--text`, a warning in the waiting colour. Piece E draws the line from the folder alone
/// ("Machine and folder" until one is chosen); the branch and worktree details come with the pickers (pieces I, L).
pub fn plan_line(s: &State, t: &Theme) -> El {
    let mut line = row().key("new-session-plan").w_full().flex_wrap().items_center().min_h(22.0).px(11.0).pt(4.0);
    for part in s.plan_parts() {
        line = line.child(if part.code {
            text(part.text).font(mono(11.5, 400.0, 1.5)).color(t.text)
        } else {
            text(part.text).font(sans(12.0, 400.0, 1.5)).color(if part.warn { t.status_waiting_text } else { t.muted })
        });
    }
    line
}
