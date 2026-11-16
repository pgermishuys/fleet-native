//! The shell: `[rail][context]` over the status bar. Sizes are Fleet's CSS pixels at its default
//! 14px root (see `client/src/components/layout/AppShell.vue`).
pub mod agent_row;
pub mod allowed;
pub mod analytics;
pub mod ask_card;
pub mod attachments;
pub mod autocomplete;
pub mod base_picker;
pub mod browser_steps;
mod compaction;
pub mod composer;
pub mod conversation;
pub mod diff;
pub mod draft_row;
pub mod export;
pub mod failure;
pub mod family;
pub mod find;
mod fold;
pub mod folder_menu;
pub mod folder_menu_clone;
pub mod folder_menu_path;
pub mod fork;
pub mod gh;
pub mod harness_picker;
pub mod harness_setup;
pub mod header;
pub mod header_banners;
pub mod header_context;
pub mod history;
pub mod image;
pub mod interrupted;
pub mod machine_group;
pub mod machine_header;
pub mod machine_picker;
mod marks;
pub mod message;
pub mod more_options;
pub mod new_session;
pub mod new_session_box;
pub mod new_session_refs;
pub mod new_session_strip;
pub mod pagepick;
mod parts;
pub mod permission;
pub mod pr_popover;
pub mod profile_picker;
pub mod progress_ring;
pub mod project_dialogs;
pub mod project_header;
pub mod project_menu;
mod prose;
pub mod question;
pub mod queued;
mod rail;
mod reasoning;
mod recap;
pub mod ring;
pub mod ring_popover;
pub mod row_actions;
pub mod scroll;
pub mod selection_bar;
pub mod selector;
pub mod session_row;
mod sessions;
mod sheet;
mod shell;
pub mod starters;
mod status;
pub mod subagent_row;
pub mod toolcard;
pub mod toolrow;
pub mod update;
pub mod work;
pub mod work_output;
pub mod working;
pub mod workspace_picker;

use crate::theme::{Theme, sans};
use crate::{Rail, State};
use rune::{Cursor, Cx, Drag, El, Icon, Phase, col, el, icon, row, text};

/// The session list's width, and how far its gutter can drag it (`ContextPanel.vue`).
pub const SIDEBAR_W: f32 = 306.0;
const SIDEBAR_MIN: f32 = 200.0;
const SIDEBAR_MAX: f32 = 500.0;

pub fn root(s: &State, cx: &mut Cx<State>) -> El {
    let t = Theme::of(s.dark.unwrap_or_else(|| cx.system_dark()));
    s.drawn_dark.set(t.dark);
    cx.help_look(crate::ui::help_look(&t));
    let mut main = row().w_full().grow(1.0).min_h(0.0).padding(8.0).child(rail::rail(s, &t));
    main = match s.rail {
        _ if s.gh_parts => main.child(gh::parts(&t).ml(8.0)),
        _ if s.gallery => main.child(crate::ui::gallery::view(s, &t).ml(8.0)),
        Rail::Sessions if s.sidebar_open => main.child(sessions::panel(s, &t).ml(8.0)).child(gutter(&t)).child(centre(s, &t)),
        Rail::Sessions => main.child(centre(s, &t).ml(8.0)),
        other => main.child(notice(&t, other).ml(8.0)),
    };
    // The window's size, for the selector menu's width and height caps (`100vw - 16px`, `60vh`).
    let frame = col().w_full().h_full().fill(t.main_bg).child(main).child(status::bar(s, &t)).on_layout(|s: &mut State, [_, _, w, h]| {
        if (s.ui.sel.window.0 - w).abs() > 0.5 || (s.ui.sel.window.1 - h).abs() > 0.5 {
            s.ui.sel.window = (w, h);
        }
    });
    rune_kit::root(frame, crate::ui::overlays(s, &t))
}

/// What fills the rest of the row beside the list: the session sheet, or the New session page (no content panel, A1.6).
fn centre(s: &State, t: &Theme) -> El {
    if s.on_new_session_page() { new_session::page(s, t) } else { sheet::sheet(s, t) }
}

/// The resize handle between the session list and the sheet: 8px wide in the 16px between them.
fn gutter(t: &Theme) -> El {
    el().key("sidebar-gutter")
        .w(8.0)
        .h_full()
        .mx(4.0)
        .radius(4.0)
        .hover(t.accent.alpha(0.15))
        .press(t.accent.alpha(0.15))
        .cursor(Cursor::ResizeColumn)
        .on_drag(|s: &mut State, d: &mut Drag, _| {
            if d.phase == Phase::Down {
                d.data = s.sidebar_w;
            }
            s.sidebar_w = if d.cancelled() { d.data } else { (d.data + d.dx).clamp(SIDEBAR_MIN, SIDEBAR_MAX) };
        })
}

/// What a rail entry other than Sessions shows: it owns the whole context area, as Fleet's rule
/// says, and says plainly that this prototype stops at the Sessions shell.
fn notice(t: &Theme, which: Rail) -> El {
    let (name, glyph): (&str, &'static Icon) = match which {
        Rail::Workflows => ("Workflows", &crate::icons::WORKFLOW),
        Rail::Automations => ("Automations", &crate::icons::ZAP),
        Rail::Analytics => ("Analytics", &crate::icons::CHART_COLUMN),
        Rail::GitHub => ("GitHub", &crate::icons::GITHUB),
        Rail::Settings | Rail::Sessions => ("Settings", &crate::icons::SETTINGS),
    };
    col()
        .grow(1.0)
        .h_full()
        .center()
        .gap(10.0)
        .fill(t.panel_bg)
        .border(1.0, t.border)
        .radius(12.0)
        .child(icon(glyph, 22.0).color(t.muted))
        .child(text(name).font(sans(14.0, 600.0, 1.3)).color(t.text))
        .child(text("Only the Sessions shell is rebuilt in this prototype.").font(sans(13.0, 400.0, 1.4)).color(t.muted))
}
