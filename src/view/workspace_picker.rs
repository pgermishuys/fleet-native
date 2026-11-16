//! Ports `NSD/WorkspacePicker.vue`: the chip that says where the session works in a repository (the current checkout, a new
//! worktree or one that exists) and the menu above it (rows B4.1-B4.14). What the rows mean is
//! `fleet::new_session::workspace`.
use crate::fleet::new_session::plan::tilde_path;
use crate::fleet::new_session::request::Workspace;
use crate::fleet::new_session::workspace::{WORKSPACE_CHIP, worktree_location};
use crate::icons::*;
use crate::theme::{Theme, sans};
use crate::ui::chip::{Chip, Option_, option, pop, pop_label, pop_separator};
use crate::view::folder_menu::spinner;
use crate::{State, Tx};
use rune::{Align, El, Side, row, text};

const WIDTH: f32 = 344.0;
const CURRENT_KEY: &str = "new-session-workspace-current";
const NEW_KEY: &str = "new-session-workspace-new";

fn worktree_key(i: usize) -> String {
    format!("new-session-workspace-worktree-{i}")
}

fn workspace(s: &State) -> Workspace {
    s.new_draft.draft().map(|d| d.workspace.clone()).unwrap_or(Workspace::New)
}

/// The chip (B4.1-B4.3): a folder, a fork or a branch for the three kinds, the label, and for the current checkout the branch
/// it is on in the muted hint.
pub fn chip(s: &State, t: &Theme) -> El {
    let ws = workspace(s);
    let (glyph, label) = match &ws {
        Workspace::Current => (&FOLDER, "Current checkout".to_string()),
        Workspace::New => (&GIT_FORK, "New worktree".to_string()),
        Workspace::Existing(path) => (&GIT_BRANCH, s.worktree_chip_label(path)),
    };
    let tail = (ws == Workspace::Current).then(|| s.current_branch().map(|b| format!(" · {b}"))).flatten();
    let disabled = s.new_draft.draft().is_some_and(|d| d.is_starting);
    Chip::new(WORKSPACE_CHIP, label).glyph(glyph).tail(tail).open(s.workspace_menu_open()).disabled(disabled).view(
        t,
        |s: &mut State, tx: &mut Tx| {
            s.open_workspace_menu();
            tx.focus(selected_key(s), false);
        },
    )
}

/// The key of the row that is checked, which the menu focuses on opening.
fn selected_key(s: &State) -> String {
    match workspace(s) {
        Workspace::Current => CURRENT_KEY.into(),
        Workspace::New => NEW_KEY.into(),
        Workspace::Existing(path) => {
            let at = s.workspace_worktrees().iter().position(|w| w.path == path);
            at.map_or_else(|| NEW_KEY.into(), worktree_key)
        }
    }
}

/// The menu: `Checkout`, the two kinds, then `Existing worktrees` when there are any or they are loading (B4.4-B4.10).
pub fn overlay(s: &State, t: &Theme) -> Vec<El> {
    let ws = workspace(s);
    let repo = s.repository_path().unwrap_or_default();
    let branch = s.current_branch();
    let current_detail = match &branch {
        Some(b) => format!("{} · {b}", tilde_path(&repo)),
        None => tilde_path(&repo),
    };
    let mut body = vec![
        pop_label(t, "Checkout"),
        option(
            t,
            Option_ {
                icon: Some(&FOLDER),
                detail: Some(current_detail),
                mono_detail: true,
                checked: ws == Workspace::Current,
                ..Option_::new(CURRENT_KEY, "Current checkout")
            },
            None,
            |s, _| s.choose_workspace(Workspace::Current),
        ),
        option(
            t,
            Option_ {
                icon: Some(&GIT_FORK),
                detail: Some("New branch from the default branch".into()),
                checked: ws == Workspace::New,
                ..Option_::new(NEW_KEY, "New worktree")
            },
            None,
            |s, _| s.choose_workspace(Workspace::New),
        ),
    ];
    let worktrees = s.workspace_worktrees();
    let loading = s.worktrees_loading();
    if !worktrees.is_empty() || loading {
        body.push(pop_separator(t));
        body.push(pop_label(t, "Existing worktrees"));
    }
    if loading && worktrees.is_empty() {
        body.push(
            row()
                .key("new-session-workspace-loading")
                .px(8.0)
                .pt(4.0)
                .pb(6.0)
                .gap(8.0)
                .center_y()
                .child(spinner(t))
                .child(text("Loading worktrees…").font(sans(11.5, 400.0, 1.4)).color(t.muted)),
        );
    }
    let last = s.sidebar.new_session_defaults(&s.target_key()).last_worktree_for(&repo);
    for (i, w) in worktrees.iter().enumerate() {
        let is_last = last.as_deref() == Some(w.path.as_str());
        let title = w.branch.clone().unwrap_or_else(|| s.worktree_chip_label(&w.path));
        let path = w.path.clone();
        let chosen = ws == Workspace::Existing(w.path.clone());
        let spec = Option_ {
            icon: Some(if is_last { &HISTORY } else { &GIT_BRANCH }),
            detail: Some(worktree_location(&repo, &w.path)),
            mono_detail: true,
            checked: chosen,
            tag: is_last.then(|| " · last used".to_string()),
            ..Option_::new(worktree_key(i), title)
        };
        body.push(option(t, spec, None, move |s, _| s.choose_workspace(Workspace::Existing(path.clone()))).help(w.path.clone()));
    }
    let panel = pop(t, "new-session-workspace-pop", WIDTH, body)
        .focus_return(WORKSPACE_CHIP.to_string())
        .modal(true, |s: &mut State| s.close_picker_menu());
    vec![
        crate::ui::overlay::scrim("new-session-workspace-scrim", |s: &mut State| s.close_picker_menu()),
        crate::ui::overlay::anchored(panel, WORKSPACE_CHIP, Side::Top, Align::Start, 6.0),
    ]
}
