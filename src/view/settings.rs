//! The Settings rail (`SettingsRoute.vue`, `SettingsPage.vue`, `SettingsNavPanel.vue`, `ui/page-header/PageHeader.vue`,
//! `CenterContent.vue`): the menu in the left panel and the page in the sheet. A page that isn't built yet shows its header and
//! one note (decision 1 of docs/parity/16-settings.md); Machines is component 17's page.
pub mod appearance;
pub mod appearance_cards;
pub mod browser;
pub mod dream_changes;
pub mod dream_nightly;
pub mod dreaming_panel;
pub mod folders;
pub mod folders_browse;
pub mod folders_locations;
pub mod folders_naming;
pub mod folders_storage;
pub mod general;
pub mod harness_install;
pub mod harness_path;
pub mod harness_profile_editor;
pub mod harness_profiles;
pub mod harness_sign_in;
pub mod harness_sign_in_flow;
pub mod harness_update;
pub mod harness_weave;
pub mod harnesses;
pub mod install_picker;
pub mod kit;
pub mod kit_scene;
pub mod labs;
pub mod machine_header;
pub mod memory;
pub mod memory_clear;
pub mod memory_expired;
pub mod memory_notes;
pub mod memory_row;
pub mod permissions;
pub mod permissions_rows;
pub mod skills_catalog;
pub mod skills_installed;
pub mod skills_tools;
pub mod plugin_page;
pub mod plugins;
pub mod plugins_scene;
pub mod updates;
pub mod updates_app;
pub mod updates_fleet;
pub mod updates_harnesses;
pub mod updates_machines;
pub mod updates_strip;
pub mod updates_version;
pub mod updates_whats_new;

use super::machines_page;
use crate::State;
use crate::fleet::settings_nav::Page;
use crate::icons::*;
use crate::theme::{RADIUS_BTN, RADIUS_PANEL, Theme, sans};
use crate::ui::rem;
use rune::{Cursor, El, Icon, ScrollSpec, col, icon, row, text};

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

/// `.settings-nav__item`: 36 high, an icon and the title; the open page has the accent's border and fill. Plugins also carries
/// the count and the fold button, and its children follow it.
fn entry(s: &State, t: &Theme, page: &Page) -> El {
    // Only the entry that is the page lights: a plugin's page lights its child, not Plugins (`modelValue === item.id`).
    let on = s.settings.page == *page;
    let to = page.id();
    let mut marks = if *page == Page::Updates { updates_marks(s, t) } else { Vec::new() };
    if *page == Page::Plugins {
        marks = plugins_marks(s, t);
    }
    let it = item(t, format!("settings-nav-{}", page.id()), page.title(), glyph(page), 16.0, 36.0, on, marks)
        .on_press(move |s: &mut State| s.open_settings(&to, None));
    if *page != Page::Plugins {
        return it;
    }
    let it = it.pr(36.0);
    // The fold button sits over the right end of the entry (`.settings-nav__fold`: 28 wide, 4 from the right).
    let unfolded = s.plugins_unfolded();
    let label = if unfolded { "Fold Plugins" } else { "Unfold Plugins" };
    let fold = row()
        .key("settings-nav-fold")
        .role("button")
        .label(label)
        .expanded(unfolded)
        .absolute(s.sidebar_w - 24.0 - 32.0, 0.0)
        .size(28.0, 36.0)
        .center()
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 0.0, t.accent)
        .child(icon(if unfolded { &CHEVRON_DOWN } else { &CHEVRON_RIGHT }, 14.0).color(t.muted))
        .on_press(|s: &mut State| s.toggle_plugins_fold());
    let mut block = col().w_full().gap(4.0).child(row().w_full().child(it).child(fold));
    if unfolded {
        for p in crate::fleet::settings::plugins::children(&s.pickers.prefs) {
            let id = format!("plugin:{}", p.id);
            let open = s.settings.page == Page::Plugin(p.id.to_string());
            let child = item(t, format!("settings-nav-{id}"), p.nav_label, plugins::glyph(p.id), 15.0, 32.0, open, Vec::new())
                .ml(14.0)
                .w(s.sidebar_w - 24.0 - 14.0)
                .on_press(move |s: &mut State| s.open_settings(&id, None));
            block = block.child(child);
        }
    }
    block
}

/// One menu item: the icon and the title, lit when it is the open page.
#[allow(clippy::too_many_arguments)]
fn item(t: &Theme, key: String, title: &'static str, glyph: &'static Icon, size: f32, min_h: f32, on: bool, marks: Vec<El>) -> El {
    let ink = if on { t.text } else { t.muted };
    let it = row()
        .key(key)
        .role("button")
        .label(title)
        .w_full()
        .min_h(min_h)
        .px(12.0)
        .gap(10.0)
        .center_y()
        .radius(RADIUS_BTN)
        .border(1.0, if on { t.accent.alpha(0.4) } else { rune::Color::TRANSPARENT })
        .cursor(Cursor::Pointer)
        .focus_ring(2.0, 2.0, t.accent)
        .child(icon(glyph, size).color(ink))
        .child(text(title).font(sans(rem(0.85714), 500.0, 1.4)).color(ink))
        .children(marks);
    if on { it.fill(t.mix(t.accent, t.panel_bg, 0.12)) } else { it.hover(t.main_bg) }
}

/// The count on Plugins (`ml-auto`, 11 px, `--muted`): how many plugins are on.
fn plugins_marks(s: &State, t: &Theme) -> Vec<El> {
    let n = crate::fleet::settings::plugins::count_on(&s.pickers.prefs);
    vec![rune::el().grow(1.0).w(0.0), text(n.to_string()).key("settings-nav-count").font(sans(rem(0.78571), 400.0, 1.4)).color(t.muted)]
}

/// The dot and the badge on Updates & about: the dot while the shown machine has an update out, coming down or ready (accent,
/// or the idle colour once it is ready), and a count of the other machines that can update. The first of them takes the right
/// edge (`ml-auto`), the badge follows it (`ml-1.5`).
fn updates_marks(s: &State, t: &Theme) -> Vec<El> {
    use crate::fleet::settings::updates_page::{Dot, menu_dot, waiting_label};
    let key = s.settings.choice.key(&s.machines);
    let dot = menu_dot(s.updates.status.get(key).and_then(|st| st.status.as_ref()));
    let waiting = crate::fleet::updates::rows::summarize_rows(&s.update_rows()).waiting;
    let mut marks = Vec::new();
    if dot.is_some() || waiting > 0 {
        marks.push(rune::el().grow(1.0).w(0.0));
    }
    if let Some(d) = dot {
        let ink = if d == Dot::Idle { t.idle } else { t.accent };
        marks.push(rune::el().key("updates-dot").role("img").label("Update available").size(8.0, 8.0).radius(4.0).fill(ink));
    }
    if waiting > 0 {
        let badge = row()
            .key("updates-badge")
            .label(waiting_label(waiting))
            .ml(if dot.is_some() { 5.25 } else { 0.0 })
            .px(5.25)
            .radius(1000.0)
            .fill(t.accent)
            .child(text(waiting.to_string()).font(sans(rem(0.71429), 600.0, 1.6)).color(t.primary_fg));
        marks.push(badge);
    }
    marks
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

/// The detail: the raised sheet, one scroll for the page, a column of `gap-6` (21). The page's own column is the scroll box:
/// inside a plain scroll box a single child column was squeezed to the window's height and the page never scrolled.
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
    } else if *page == Page::Skills {
        col().w_full().gap(21.0).child(page_header(s, t, page)).child(skills_tools::content(s, t))
    } else if *page == Page::Harnesses {
        harnesses::content(s, t)
    } else if *page == Page::Permissions {
        permissions::content(s, t)
    } else if *page == Page::Updates {
        updates::content(s, t)
    } else if *page == Page::Folders {
        folders::content(s, t)
    } else if *page == Page::Machines {
        machines_page::content(s, t)
    } else if *page == Page::Plugins {
        plugins::content(s, t)
    } else if let Page::Plugin(id) = page {
        plugin_page::content(s, t, id)
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
        .child(body.padding(24.0).scroll(ScrollSpec { stick_bottom: false }).w_full().grow(1.0).min_h(0.0))
}
