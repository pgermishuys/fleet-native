//! The Workflows page (component 18): `[workflows-list][workflows-detail]`. Ports `components/pages/WorkflowsPage.vue` and
//! `WorkflowsPane.vue`: the list and the pane, and the off notice that takes the whole context while Workflows is off.
//! The detail is the bare `PageHeader` until piece C.
use crate::State;
use crate::theme::{RADIUS_CARD, RADIUS_PANEL, Theme, sans};
use crate::ui::rem;
use crate::view::feature_off;
use rune::{ScrollSpec, col, row, text};

pub mod list;

pub const DESCRIPTION: &str =
    "A workflow is a recipe of steps for an agent, like fixing a bug or reviewing a change, that you can run again and again.";

/// The sheet every pane of the context area sits in (`AppShell`'s main card).
fn sheet(t: &Theme, key: &str) -> rune::El {
    col()
        .key(key.to_string())
        .grow(1.0)
        .h_full()
        .min_w(0.0)
        .fill(t.panel_bg)
        .border(1.0, t.border)
        .border_layout()
        .radius(RADIUS_PANEL)
        .clip_rounded(RADIUS_PANEL)
}

/// The whole context while Workflows is on: the list, its gutter and the pane. Off, the notice fills it: no other view's
/// panel sits beside it (`noPanel`).
pub fn page(s: &State, t: &Theme) -> Vec<rune::El> {
    match s.workflows_feature() {
        // Until the preferences have loaded nothing is said.
        None => vec![sheet(t, "workflows-pane")],
        Some(false) => vec![sheet(t, "workflows-pane").child(off(s, t))],
        Some(true) if s.context_panel_shown() => vec![list::panel(s, t), super::gutter(t), detail(s, t)],
        Some(true) => vec![detail(s, t)],
    }
}

fn off(s: &State, t: &Theme) -> rune::El {
    feature_off::notice(t, "workflows", "Workflows", DESCRIPTION, s.workflows_nav.turning_on, |s| s.workflows_turn_on())
}

/// `WorkflowDetailPanel`: with nothing open, or the library still loading, the view's own title; a failed library is a
/// centred error note (row 3.13).
fn detail(s: &State, t: &Theme) -> rune::El {
    let nav = &s.workflows_nav;
    let body = if let (None, Some(error)) = (&nav.library, &nav.library_error) {
        col().w_full().h_full().center().child(
            row()
                .key("workflows-error")
                .role("alert")
                .max_w(rem(28.0))
                .gap(8.0)
                .items_start()
                .px(12.0)
                .py(8.0)
                .radius(RADIUS_CARD)
                .border(1.0, t.error.alpha(0.3))
                .border_layout()
                .fill(t.error.alpha(0.1))
                .child(rune::icon(&crate::icons::CIRCLE_ALERT, 16.0).color(t.error_text).mt(1.0))
                .child(text(error.clone()).font(sans(rem(0.875), 400.0, 1.43)).color(t.error_text).wrap().shrink().min_w(0.0)),
        )
    } else {
        col().w_full().gap(21.0).child(page_header(t, "Workflows")).padding(24.0)
    };
    sheet(t, "workflows-detail").child(body.scroll(ScrollSpec { stick_bottom: false }).w_full().grow(1.0).min_h(0.0))
}

/// `PageHeader`: the title (21, 600).
fn page_header(t: &Theme, title: &str) -> rune::El {
    col().key("workflows-header").w_full().child(text(title.to_string()).font(sans(rem(1.5), 600.0, 1.25).tracking(-0.525)).color(t.text))
}
