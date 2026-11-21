//! Ports the strip of `AutomationComposer.vue` (row 9.1) and the two pickers only it has: `AutomationRunsInPicker.vue` (9.6)
//! and `AutomationMoreOptions.vue` (9.7). The folder, workspace and base chips are New session's (`FolderPicker`,
//! `WorkspacePicker`, `BasePicker`; 9.2, 9.5), drawn here on the composer's state (`fleet::new_session::host`).
use crate::fleet::automations::strip::{MORE_CHIP, NAME_KEY, NEW_SESSION, RUNS_IN_CHIP, SAME_SESSION, older_target, runs_in_label};
use crate::fleet::new_session::request::{Folder, Workspace};
use crate::icons::{CHECK, ELLIPSIS, MESSAGE_SQUARE, REPEAT};
use crate::theme::{Theme, sans};
use crate::ui::chip::{Chip, Option_, option, pop, pop_field, pop_field_box};
use crate::ui::text_menu::text_input;
use crate::view::automations::composer::MESSAGE;
use crate::{State, Tx};
use rune::{Align, Cursor, El, Key, KeyEvent, Side, col, el, icon, row, text};

/// `.ns-more`.
const MORE_WIDTH: f32 = 300.0;
const POP_WIDTH: f32 = 344.0;

/// `.automation-composer__separator`: 1 by 14 in the border colour, 3 px either side.
pub fn separator(t: &Theme) -> El {
    el().w(1.0).h(14.0).mx(3.0).fill(t.border)
}

/// The chips after the When chip, in Fleet's order: the folder; for a workflow New worktree and the base, else for a repository
/// the workspace and (for a new worktree) the base; the Runs chip; for a workflow its chip, its steps and the harness, else
/// Runs in; and the "…" chip pushed to the end.
pub fn chips(s: &State, t: &Theme, busy: bool) -> Vec<El> {
    let editing = s.composer_editing();
    let c = s.automations_nav.composer(editing);
    let mut out = vec![separator(t), crate::view::folder_menu::trigger(s, t)];
    let repository = matches!(c.folder, Some(Folder::Repository(_)));
    let workflow = c.is_workflow();
    if workflow {
        // A workflow always runs in a new worktree: a fact, not a choice (row 12.3).
        out.push(super::workflow::new_worktree(t));
        if repository {
            out.push(crate::view::base_picker::chip(s, t));
        }
    } else if repository {
        out.push(crate::view::workspace_picker::chip(s, t));
        if c.workspace == Workspace::New {
            out.push(crate::view::base_picker::chip(s, t));
        }
    }
    if s.automation_shows_target_chip() {
        out.push(super::workflow::target_chip(s, t, workflow, busy));
    }
    if workflow {
        out.push(super::workflow::workflow_chip(s, t, busy));
        out.extend(super::workflow::step_chips(s, t, busy));
        if s.automation_workflow_harnesses().len() > 1 {
            out.push(crate::view::harness_picker::chip(s, t));
        }
    } else {
        out.push(runs_in_chip(s, t, &c.target_type, busy));
    }
    // `.automation-composer__strip-end` (`margin-left: auto`): at the right end of whichever line it wraps onto.
    out.push(row().grow(1.0).justify_end().child(more_chip(s, t, busy)));
    out
}

/// The Runs in chip: Repeat for the same session, else MessageSquare, and the target in words.
fn runs_in_chip(s: &State, t: &Theme, target_type: &str, busy: bool) -> El {
    let glyph = if target_type == SAME_SESSION { &REPEAT } else { &MESSAGE_SQUARE };
    Chip::new(RUNS_IN_CHIP, runs_in_label(target_type)).glyph(glyph).open(s.runs_in_menu_open()).disabled(busy).view(
        t,
        |s: &mut State, tx: &mut Tx| {
            s.open_runs_in_menu();
            tx.focus("automation-runs-in-new", false);
        },
    )
}

/// The Runs in menu above its chip: the two targets this page makes, and an older automation's own while it still has it.
pub fn runs_in_overlay(s: &State, t: &Theme) -> Vec<El> {
    let target = s.automations_nav.composer(s.composer_editing()).target_type.clone();
    let row_for = |key: &str, glyph, title: &str, detail: &str, value: &'static str| {
        let spec = Option_ { icon: Some(glyph), detail: Some(detail.to_string()), checked: target == value, ..Option_::new(key, title) };
        option(t, spec, None, move |s, _| s.set_runs_in(value))
    };
    let mut body = vec![
        row_for("automation-runs-in-new", &MESSAGE_SQUARE, "New session each run", "each run starts fresh", NEW_SESSION),
        row_for("automation-runs-in-same", &REPEAT, "Same session each run", "later runs continue the first run's session", SAME_SESSION),
    ];
    if let Some(words) = older_target(&target) {
        let spec = Option_ {
            icon: Some(&MESSAGE_SQUARE),
            detail: Some("how this automation was set up".into()),
            checked: true,
            ..Option_::new("automation-runs-in-older", words)
        };
        body.push(option(t, spec, None, |s, _| s.close_strip_menus()));
    }
    let panel = pop(t, "automation-runs-in-pop", POP_WIDTH, body)
        .focus_return(MESSAGE.to_string())
        .modal(true, |s: &mut State| s.close_strip_menus());
    vec![
        crate::ui::overlay::scrim("automation-runs-in-scrim", |s: &mut State| s.close_strip_menus()),
        crate::ui::overlay::anchored(panel, RUNS_IN_CHIP, Side::Top, Align::Start, 6.0),
    ]
}

/// The "…" chip: the ellipsis alone.
fn more_chip(s: &State, t: &Theme, busy: bool) -> El {
    Chip::new(MORE_CHIP, "")
        .glyph(&ELLIPSIS)
        .flat()
        .open(s.automation_more_open())
        .disabled(busy)
        .help(Some("Name, skipping".into()))
        .view(t, |s: &mut State, tx: &mut Tx| {
            s.open_automation_more();
            tx.focus(NAME_KEY, false);
        })
        .label("More options: name, skipping")
}

/// `.automation-more__check`: a box (or, for a workflow, nothing to tick) beside the words and the small line under them.
fn check(t: &Theme, key: &str, on: Option<bool>, title: &str, detail: &str) -> El {
    let words = col()
        .grow(1.0)
        .min_w(0.0)
        .child(text(title.to_string()).font(sans(12.5, 400.0, 1.4)).color(t.text).wrap().w_full())
        .child(text(detail.to_string()).font(sans(11.5, 400.0, 1.4)).color(t.muted).wrap().w_full());
    let mut line = row().key(key.to_string()).w_full().gap(8.0).px(8.0).pt(6.0).pb(8.0).items_start();
    let Some(on) = on else { return line.child(words) };
    // A checkbox in the accent colour (`accent-color: var(--accent)`), 2 px down to sit on the first line.
    let mut mark = el().size(13.0, 13.0).mt(2.0).radius(3.0).border(1.0, if on { t.accent } else { t.muted }).border_layout();
    if on {
        mark = mark.fill(t.accent).center().child(icon(&CHECK, 10.0).color(t.primary_fg));
    }
    line = line
        .role("checkbox")
        .label(title.to_string())
        .checked(on)
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 0.0, t.accent)
        .on_press(|s: &mut State| s.toggle_automation_skip());
    line.child(mark).child(words)
}

/// The "…" popover above its chip, its right edge on the chip's: the Name, and skipping.
pub fn more_overlay(s: &State, t: &Theme) -> Vec<El> {
    let c = s.automations_nav.composer(s.composer_editing());
    let placeholder = Some(c.name_placeholder()).filter(|p| !p.is_empty()).unwrap_or_else(|| "From the first words".into());
    let name = text_input(&s.automations_nav.name_box, sans(13.0, 400.0, 1.3))
        .key(NAME_KEY)
        .label("Name")
        .placeholder(placeholder, t.muted)
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .single_line()
        .w_full()
        .on_edit(|s: &mut State| &mut s.automations_nav.name_box)
        .on_input_change(|s: &mut State, _: &mut Tx| s.automation_name_edited())
        .on_input_key(|s: &mut State, ev: &KeyEvent, tx: &mut Tx| {
            let enter = ev.key == Key::Enter;
            if enter {
                s.automation_commit_name(tx);
            }
            enter
        });
    let skip = if c.is_workflow() {
        check(
            t,
            "automation-always-skips",
            None,
            "A firing is skipped while the last run is unfinished",
            "So a workflow run waiting on you never has another piled up behind it.",
        )
    } else {
        check(
            t,
            "automation-skip",
            Some(c.skip),
            "Skip a run while the last one is still going",
            "So a slow run never piles up behind itself.",
        )
    };
    let body = vec![pop_field(t, "Name", pop_field_box(t, name)), skip];
    let panel = pop(t, "automation-more-pop", MORE_WIDTH, body)
        .focus_return(MESSAGE.to_string())
        .modal(true, |s: &mut State| s.close_strip_menus());
    vec![
        crate::ui::overlay::scrim("automation-more-scrim", |s: &mut State| s.close_strip_menus()),
        crate::ui::overlay::anchored(panel, MORE_CHIP, Side::Top, Align::End, 6.0),
    ]
}
