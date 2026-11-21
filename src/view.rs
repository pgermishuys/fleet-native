//! The shell: `[rail][context]` over the status bar. Sizes are Fleet's CSS pixels at its default
//! 14px root (see `client/src/components/layout/AppShell.vue`).
pub mod add_machine;
pub mod agent_row;
pub mod agent_tab;
pub mod agents;
pub mod agents_row;
pub mod allowed;
pub mod analytics;
pub mod analytics_page;
pub mod annotation;
pub mod ask_card;
pub mod attachments;
pub mod autocomplete;
pub mod automations;
pub mod base_picker;
pub mod board;
pub mod browser_agent_bar;
pub mod browser_app;
pub mod browser_open;
pub mod browser_steps;
pub mod browser_tab;
pub mod canvas_host;
pub mod changes;
pub mod changes_find;
pub mod code_edit;
pub mod code_input_spike;
pub mod code_view;
mod compaction;
pub mod completion;
pub mod composer;
pub mod context_tab;
pub mod conversation;
pub mod dashboard;
pub mod dashboard_row;
pub mod diff;
pub mod document_tab;
pub mod draft_row;
pub mod drop_zones;
pub mod export;
pub mod failure;
pub mod family;
pub mod feature_off;
pub mod file_compare;
pub mod file_diff;
pub mod file_find;
pub mod file_media;
pub mod file_tab;
pub mod file_tree;
pub mod files_tab;
pub mod find;
pub mod find_in_files;
mod fold;
pub mod folder_menu;
pub mod folder_menu_clone;
pub mod folder_menu_path;
pub mod fork;
pub mod gh;
pub mod go_to_file;
pub mod harness_picker;
pub mod harness_setup;
pub mod harness_setup_rows;
pub mod header;
pub mod header_banners;
pub mod header_context;
pub mod history;
pub mod image;
pub mod improve_skill;
pub mod interrupted;
pub mod jobs;
mod labs_frame;
pub mod lines_bar;
pub mod machine_addresses;
pub mod machine_card;
pub mod machine_group;
pub mod machine_header;
pub mod machine_picker;
pub mod machine_switch;
pub mod machines_page;
mod marks;
pub mod message;
pub mod mission;
pub mod mods;
pub mod more_options;
pub mod new_project;
pub mod new_session;
pub mod new_session_box;
pub mod new_session_refs;
pub mod new_session_strip;
pub mod notice_card;
pub mod notice_chips;
pub mod open_in;
pub mod page_card;
pub mod page_choice;
pub mod page_tab;
pub mod pagepick;
pub mod pages;
mod parts;
pub mod permission;
pub mod pr_popover;
pub mod profile_picker;
pub mod progress_ring;
pub mod progress_strip;
pub mod progress_subagent;
pub mod progress_tab;
pub mod project_dialogs;
pub mod project_header;
pub mod project_menu;
pub mod prose;
pub mod question;
pub mod queued;
pub mod quit_dialog;
pub mod rail;
mod reasoning;
mod recap;
pub mod report;
pub mod report_review;
pub mod review;
pub mod review_lines;
pub mod review_split;
mod right_rail;
pub mod ring;
pub mod ring_popover;
pub mod row_actions;
pub mod running_counter;
pub mod scroll;
pub mod selection_bar;
pub mod selector;
pub mod session_row;
mod sessions;
pub mod settings;
mod sheet;
mod shell;
pub mod side;
pub mod side_tab;
pub mod sign_in;
pub mod skill_diff;
pub mod smart_link_card;
pub mod smart_link_row;
pub mod start_page;
pub mod starters;
mod status;
pub mod subagent_row;
pub mod summary_bar;
pub mod terminal_bar;
pub mod terminal_drawer;
pub mod terminal_pop;
pub mod terminal_view;
pub mod toolcard;
pub mod toolrow;
pub mod turn_detail;
pub mod turns;
pub mod unsaved_dialog;
pub mod update;
pub mod usage_chip;
pub mod usage_popover;
pub mod web_card;
pub mod work;
pub mod work_output;
pub mod workflows;
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
    let t = s.with_code_theme(Theme::of(s.dark.unwrap_or_else(|| cx.system_dark())));
    s.drawn_dark.set(t.dark);
    // Font size (Appearance): Fleet sets the root size, 13 to 16 px against 14; the window scales as a page zoom does.
    cx.zoom(s.sidebar.appearance().font_size.px() / 14.0);
    cx.help_look(crate::ui::help_look(&t));
    // The splash and the error page are pages of their own too, while the app starts its Fleet.
    if let Some(page) = start_page::page(s, &t) {
        return rune_kit::root(page, crate::ui::overlays(s, &t));
    }
    // Fleet's login page is a page of its own: no rail, no status bar.
    if s.sign_in.open {
        return rune_kit::root(sign_in::page(s, &t), crate::ui::overlays(s, &t));
    }
    let mut main = row().w_full().grow(1.0).min_h(0.0).padding(8.0).child(rail::rail(s, &t));
    main = match s.rail {
        _ if s.machine_switch_scene.is_some() => main.child(machine_switch::scene_page(s, &t).ml(8.0)),
        _ if s.on_bare_page() => main.child(pages::page(s, &t).ml(8.0)),
        _ if s.gh_parts => main.child(gh::parts(&t).ml(8.0)),
        _ if s.gallery => main.child(crate::ui::gallery::view(s, &t).ml(8.0)),
        Rail::Sessions if s.context_panel_shown() => main.child(sessions::panel(s, &t).ml(8.0)).child(gutter(&t)).child(centre(s, &t)),
        Rail::Sessions => main.child(centre(s, &t).ml(8.0)),
        Rail::Settings if s.context_panel_shown() => {
            main.child(settings::menu(s, &t).ml(8.0)).child(gutter(&t)).child(settings::detail(s, &t))
        }
        Rail::Settings => main.child(settings::detail(s, &t).ml(8.0)),
        Rail::Workflows => {
            let mut panes = workflows::page(s, &t).into_iter();
            if let Some(first) = panes.next() {
                main = main.child(first.ml(8.0));
            }
            main.children(panes)
        }
        Rail::Fleet => {
            let mut panes = mission::page(s, &t).into_iter();
            if let Some(first) = panes.next() {
                main = main.child(first.ml(8.0));
            }
            main.children(panes)
        }
        Rail::Jobs => {
            let mut panes = jobs::page(s, &t).into_iter();
            if let Some(first) = panes.next() {
                main = main.child(first.ml(8.0));
            }
            main.children(panes)
        }
        Rail::Board => {
            let mut panes = board::page(s, &t).into_iter();
            if let Some(first) = panes.next() {
                main = main.child(first.ml(8.0));
            }
            main.children(panes)
        }
        Rail::Analytics => {
            let mut panes = analytics_page::page(s, &t).into_iter();
            if let Some(first) = panes.next() {
                main = main.child(first.ml(8.0));
            }
            main.children(panes)
        }
        Rail::Automations => {
            let mut panes = automations::page(s, &t).into_iter();
            if let Some(first) = panes.next() {
                main = main.child(first.ml(8.0));
            }
            main.children(panes)
        }
        Rail::Plugin(id) => match s.places.rail_views(id, &s.pickers.prefs) {
            // A plugin's panel (when it has one) and its page: the plugin owns the whole context.
            Some((Some(panel), page)) if s.context_panel_shown() => main.child(panel(s, &t).ml(8.0)).child(gutter(&t)).child(page(s, &t)),
            Some((_, page)) => main.child(page(s, &t).ml(8.0)),
            None => main.child(notice(s, &t, Rail::Plugin(id)).ml(8.0)),
        },
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
    if s.on_new_session_page() {
        new_session::page(s, t)
    } else if s.on_home() {
        dashboard::page(s, t)
    } else {
        sheet::sheet(s, t)
    }
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
fn notice(s: &State, t: &Theme, which: Rail) -> El {
    let (name, glyph): (&str, &'static Icon) = match which {
        Rail::Plugin(id) => s.places.rail_item(id).map_or(("Plugin", &crate::icons::PUZZLE), |p| (p.label, p.icon)),
        _ => ("Settings", &crate::icons::SETTINGS),
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
