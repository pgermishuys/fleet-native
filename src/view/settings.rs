//! The Settings rail (`SettingsRoute.vue`, `SettingsPage.vue`, `SettingsNavPanel.vue`, `ui/page-header/PageHeader.vue`,
//! `CenterContent.vue`): the menu in the left panel and the page in the sheet. A page that isn't built yet shows its header and
//! one note (decision 1 of docs/parity/16-settings.md); Machines is component 17's page.
pub mod appearance;
pub mod appearance_cards;
pub mod dream_changes;
pub mod dream_nightly;
pub mod dreaming_panel;
pub mod folders;
pub mod folders_browse;
pub mod folders_locations;
pub mod folders_naming;
pub mod folders_storage;
pub mod general;
pub mod harness_profile_editor;
pub mod harness_profiles;
pub mod harnesses;
pub mod kit;
pub mod kit_scene;
pub mod labs;
pub mod machine_header;
pub mod memory;
pub mod memory_clear;
pub mod memory_expired;
pub mod memory_notes;
pub mod memory_row;

use super::machines_page;
use crate::State;
use crate::fleet::settings_nav::Page;
use crate::icons::*;
use crate::theme::{RADIUS_BTN, RADIUS_PANEL, Theme, sans};
use crate::ui::rem;
use rune::{Cursor, El, Icon, ScrollSpec, col, el, icon, row, text};

/// The menu's groups, in Fleet's order (`coreGroups`); Plugins is its own group under them.
const GROUPS: [(&str, &[Page]); 5] = [
    ("You", &[Page::General, Page::Appearance]),
    ("Agents", &[Page::Harnesses, Page::Permissions, Page::Skills, Page::Memory, Page::Credentials]),
    ("Work", &[Page::Folders, Page::Machines]),
    ("Fleet", &[Page::Labs, Page::Updates]),
    ("Plugins", &[Page::Plugins]),
];

fn glyph(page: &Page) -> &'static Icon {
    match page {
        Page::General => &SLIDERS_HORIZONTAL,
        Page::Appearance => &PALETTE,
        Page::Harnesses => &CABLE,
        Page::Permissions => &SHIELD_CHECK,
        Page::Skills => &WRENCH,
        Page::Memory => &LIGHTBULB,
        Page::Folders => &FOLDER_GIT_2,
        Page::Machines => &SERVER,
        Page::Labs => &FLASK_CONICAL,
        Page::Updates => &REFRESH_CW,
        Page::Credentials => &KEY_ROUND,
        Page::Plugins | Page::Plugin(_) => &PUZZLE,
    }
}

fn heading(t: &Theme, words: &str) -> El {
    text(words.to_uppercase()).font(sans(rem(0.71429), 600.0, 1.3).tracking(0.5)).color(t.muted)
}

/// `.settings-nav__item`: 36 high, an icon and the title; the open page has the accent's border and fill.
fn entry(s: &State, t: &Theme, page: &Page) -> El {
    let on = s.settings.page.entry() == *page;
    let ink = if on { t.text } else { t.muted };
    let to = page.id();
    let mut it = row()
        .key(format!("settings-nav-{}", page.id()))
        .role("button")
        .label(page.title())
        .w_full()
        .min_h(36.0)
        .px(12.0)
        .gap(10.0)
        .center_y()
        .radius(RADIUS_BTN)
        .border(1.0, if on { t.accent.alpha(0.4) } else { rune::Color::TRANSPARENT })
        .cursor(Cursor::Pointer)
        .focus_ring(2.0, 2.0, t.accent)
        .child(icon(glyph(page), 16.0).color(ink))
        .child(text(page.title()).font(sans(rem(0.85714), 500.0, 1.4)).color(ink))
        .on_press(move |s: &mut State| s.open_settings(&to, None));
    it = if on { it.fill(t.mix(t.accent, t.panel_bg, 0.12)) } else { it.hover(t.main_bg) };
    it
}

/// `SettingsNavPanel`: "SETTINGS", then a labelled group per heading. It sits on the window's own colour, like the session list.
pub fn menu(s: &State, t: &Theme) -> El {
    let mut nav = col().w_full().px(12.0).pb(12.0);
    for (name, pages) in GROUPS {
        let mut group = col().key(format!("settings-group-{}", name.to_lowercase())).label(name).w_full().gap(4.0).mb(8.0);
        group = group.child(row().mt(8.0).mb(2.0).px(12.0).child(heading(t, name)));
        nav = nav.child(group.children(pages.iter().map(|p| entry(s, t, p))));
    }
    col()
        .key("settings-menu")
        .label("Settings menu")
        .w(s.sidebar_w)
        .h_full()
        .min_h(0.0)
        .child(row().pt(4.0).child(row().px(16.0).pt(14.0).pb(10.0).child(heading(t, "Settings"))))
        .children(machine_header::menu_switch(s, t))
        .child(col().w_full().grow(1.0).shrink().min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(nav))
}

/// `PageHeader`: the title (21, 600) and the line under it.
pub fn page_header(s: &State, t: &Theme, page: &Page) -> El {
    let title = text(page.title()).font(sans(rem(1.5), 600.0, 1.25).tracking(-0.525)).color(t.text);
    // The title row's meta slot: gap 10, the chip of the machine the page shows (`PageHeader`, `SettingsPage`).
    let title_row = row().wrap().center_y().gap(10.0).child(title).children(machine_header::title_chip(s, t, page));
    col()
        .key("settings-header")
        .w_full()
        .child(title_row)
        .child(row().mt(4.0).child(text(page.line()).font(sans(rem(0.875), 400.0, 1.5)).color(t.muted)))
}

/// What a page that isn't ported yet shows (`rounded-btn border px-4 py-3 text-sm text-muted`).
fn not_yet(t: &Theme, page: &Page) -> El {
    row()
        .key("settings-not-yet")
        .w_full()
        .px(14.0)
        .py(10.5)
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .child(text(format!("{} isn't in the native app yet.", page.title())).font(sans(rem(0.875), 400.0, 1.5)).color(t.muted))
}

/// The detail: the raised sheet, one scroll for the page, a column of `gap-6` (21).
pub fn detail(s: &State, t: &Theme) -> El {
    let page = &s.settings.page;
    let body = if s.skills.demo_diff {
        // Scene only: the skill diff before the dialog that shows it is built (piece U).
        super::skill_diff::demo(s, t)
    } else if s.settings_comes_later() {
        col().w_full().gap(21.0).child(page_header(s, t, page)).child(machine_header::later(s, t, page))
    } else if s.settings.demo_open {
        kit_scene::content(s, t)
    } else if *page == Page::General {
        general::content(s, t)
    } else if *page == Page::Labs {
        labs::content(s, t)
    } else if *page == Page::Appearance {
        appearance::content(s, t)
    } else if *page == Page::Memory {
        memory::content(s, t)
    } else if *page == Page::Harnesses {
        harnesses::content(s, t)
    } else if *page == Page::Folders {
        folders::content(s, t)
    } else if *page == Page::Machines {
        machines_page::content(s, t)
    } else {
        col().w_full().gap(21.0).child(page_header(s, t, page)).child(not_yet(t, page))
    };
    col()
        .key("settings-detail")
        .grow(1.0)
        .h_full()
        .min_w(0.0)
        .fill(t.panel_bg)
        .border(1.0, t.border)
        .border_layout()
        .radius(RADIUS_PANEL)
        .clip_rounded(RADIUS_PANEL)
        .child(
            el().w_full().grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).key("settings-scroll").child(body.padding(24.0)),
        )
}
