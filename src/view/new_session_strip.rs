//! Ports the chips under the New session box (`NewSessionComposer.vue`'s `.new-session__strip`, rows A11, B11.17): the
//! machine chip, a separator, the folder chip, and the workspace, base and "…" chips as their pieces land. Each chip is
//! `ui::chip::Chip`; what a chip opens is its picker's piece (the toast says which until it is built).
use crate::theme::{Theme, mono, sans};
use crate::{State, Tx};
use rune::{El, el, row, text};

use crate::fleet::catalog::Chip as SelectorChip;

/// `.new-session__strip-separator`: 1 by 14, `--border`, 3px either side.
fn separator(t: &Theme) -> El {
    el().w(1.0).h(14.0).mx(3.0).fill(t.border)
}

/// `.new-session__strip`: wraps, 2px apart, `6px 4px 0`.
pub fn strip(s: &State, t: &Theme) -> El {
    let chips = s.strip_chips();
    let mut r = row().key("new-session-strip").w_full().wrap().items_center().gap(2.0).px(4.0).pt(6.0);
    if chips.machine {
        r = r.child(super::machine_picker::chip(s, t)).child(separator(t));
    }
    r = r.child(super::folder_menu::trigger(s, t));
    if chips.workspace {
        r = r.child(separator(t)).child(super::workspace_picker::chip(s, t));
    }
    if chips.base {
        r = r.child(super::base_picker::chip(s, t));
    }
    // `margin-left: auto` pushes the "…" chip to the strip's right end.
    r.child(el().grow(1.0)).child(super::more_options::chip(s, t))
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

/// The harness, profile, agent and model chips, which Fleet puts in the message box's toolbar after the attach button
/// (`NewSessionComposer.vue`'s `#toolbar`). The box and its frame are piece L's; until then they sit at the box's foot.
pub fn toolbar_chips(s: &State, t: &Theme) -> El {
    let mut r = row().key("new-session-toolbar").w_full().items_center().gap(2.0).pt(4.0).px(8.0).pb(8.0);
    if s.shows_harness_picker() {
        r = r.child(super::harness_picker::chip(s, t));
    }
    if s.shows_profile_picker() {
        r = r.child(super::profile_picker::chip(s, t));
    }
    if s.offers_agent_and_model() {
        let open = s.ui.open;
        let chip = |label: String| SelectorChip { label, help: None };
        r = r.child(super::composer::selector_chip(t, open, "agent", chip(s.page_agent_chip()))).child(super::composer::selector_chip(
            t,
            open,
            "model",
            chip(s.page_model_chip()),
        ));
    }
    r
}

/// `.new-session__no-harness`: why no session can start yet, with the link that fixes it (`fixNoHarness`). It sits above the box,
/// in the idle colour at 10% with a 35% border.
pub fn no_harness_notice(s: &State, t: &Theme) -> Option<El> {
    let (reason, action) = s.no_harness_notice()?;
    let spans = vec![rune::Span::new(0, action.len()).color(t.accent_text).underline(t.accent_text)];
    let fix = row()
        .key("new-session-no-harness-fix")
        .role("button")
        .label(action.clone())
        .cursor(rune::Cursor::Pointer)
        .focus_ring(2.0, 0.0, t.accent)
        .on_press_tx(|s: &mut State, tx: &mut Tx| s.fix_no_harness(tx))
        .child(rune::rich(action, spans).font(sans(12.0, 500.0, 1.5)).color(t.accent_text));
    Some(
        row()
            .key("new-session-no-harness")
            .role("status")
            .w_full()
            .flex_wrap()
            .items_center()
            .gap(10.0)
            .px(12.0)
            .py(10.0)
            .mb(10.0)
            .radius(crate::theme::RADIUS_CARD)
            .fill(t.idle.alpha(0.10))
            .border(1.0, t.idle.alpha(0.35))
            .border_layout()
            .child(text(reason).font(sans(12.0, 400.0, 1.5)).color(t.text))
            .child(fix),
    )
}
