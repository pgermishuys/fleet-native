//! Settings → Plugins (`PluginsOverview.vue`): "All plugins", then one row per built-in plugin with its switch, what it adds and a
//! way into its page. The rows come from the table in `fleet::settings::plugins`; no view here names a plugin.
use super::kit::{self, SwitchState};
use crate::State;
use crate::fleet::settings::plugins::{self, Plugin};
use crate::fleet::settings_nav::Page;
use crate::icons::*;
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, sans};
use crate::ui::rem;
use rune::{Cursor, El, Icon, col, icon, row, text};

/// The plugin's config page icon, else `Puzzle` (`manifest.contributions.configPage.icon ?? Puzzle`).
pub fn glyph(id: &str) -> &'static Icon {
    match id {
        "github" => &GITHUB,
        "mods" => &WAND_SPARKLES,
        "built-in-skills" => &SPARKLES,
        _ => &PUZZLE,
    }
}

/// The "Built in" chip: `rounded-full border px-1.5`, 10 px semibold.
pub fn built_in(t: &Theme, key: String) -> El {
    row()
        .key(key)
        .px(5.25)
        .radius(1000.0)
        .border(1.0, t.border)
        .border_layout()
        .child(text("Built in").font(sans(rem(0.71429), 600.0, 1.4)).color(t.muted))
}

/// A plugin's switch, for the row and for its page. The Mods switch is its own preference and shows its saving; while the
/// panel asks whether to install Bun it stays off with a ring (`pending`), and pressing it again cancels.
pub fn switch(s: &State, t: &Theme, p: &Plugin, key: String) -> El {
    let on = p.enabled(&s.pickers.prefs);
    let saving = p.own_switch && s.setting_saving(plugins::MODS_KEY);
    let pending = p.own_switch && s.settings.mods_runtime.confirming;
    let state = SwitchState { saving, disabled: s.preferences_loading() || saving, pending };
    let id = p.id;
    kit::setting_switch(t, &key, &format!("{} on or off", p.name), on, state, move |s: &mut State| s.plugin_press(id))
}

/// What stands under a Mods row, in the slot Fleet gives the plugin's own panel (`basis-full pl-12`): the question about Bun,
/// the install, why it failed, or the Bun in use.
pub fn own_panel(s: &State, t: &Theme, p: &Plugin, key: String) -> Option<El> {
    if !p.own_switch {
        return None;
    }
    super::mods_panel::panel(s, t, key)
}

fn chip(t: &Theme, key: String, words: &str) -> El {
    row()
        .key(key)
        .px(5.25)
        .py(1.75)
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.main_bg)
        .child(text(words.to_string()).font(sans(rem(0.75), 400.0, 1.33)).color(t.text))
}

fn open_page(id: &'static str) -> impl Fn(&mut State) + 'static {
    move |s: &mut State| s.open_settings(&format!("plugin:{id}"), None)
}

fn plugin_row(s: &State, t: &Theme, p: &Plugin, last: bool) -> El {
    let id = p.id;
    let tile = row()
        .size(rem(2.25), rem(2.25))
        .center()
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.main_bg)
        .child(icon(glyph(id), 18.0).color(t.text));
    let name = text(p.name)
        .font(sans(rem(0.875), 600.0, 1.43))
        .color(t.text)
        .key(format!("plugin-name-{id}"))
        .cursor(Cursor::Pointer)
        .role("button")
        .label(p.name)
        .on_press(open_page(id));
    let mut body = col()
        .grow(1.0)
        .shrink()
        .min_w(0.0)
        .child(row().items_center().gap(7.0).child(name).child(built_in(t, format!("plugin-built-in-{id}"))));
    body = body.child(
        text(p.description).key(format!("plugin-description-{id}")).font(sans(rem(0.875), 400.0, 1.43)).color(t.muted).wrap().mt(1.75),
    );
    let mut adds = row().key(format!("plugin-adds-{id}")).label(format!("{} adds to", p.name)).flex_wrap().items_center().gap(5.25).mt(7.0);
    adds = adds.child(text("Adds to").font(sans(rem(0.75), 400.0, 1.33)).color(t.muted));
    for area in p.areas() {
        adds = adds.child(chip(t, format!("plugin-adds-{id}-{}", area.label()), area.label()));
    }
    body = body.child(adds).child(text("Built in to Fleet").font(sans(rem(0.75), 400.0, 1.33)).color(t.muted).mt(7.0));
    let mut r = row().key(format!("plugin-row-{id}")).w_full().flex_wrap().items_start().gap(10.5).padding(14.0).child(tile).child(body);
    if !last {
        r = r.border_sides([0.0, 0.0, 1.0, 0.0], t.border).border_layout();
    }
    if !p.switchless {
        r = r.child(switch(s, t, p, format!("plugin-switch-{id}")));
    } else {
        r = r.child(
            row()
                .key(format!("plugin-open-{id}"))
                .role("button")
                .label("Open")
                .px(8.75)
                .py(3.5)
                .radius(RADIUS_BTN)
                .border(1.0, t.border)
                .border_layout()
                .hover(t.main_bg)
                .cursor(Cursor::Pointer)
                .focusable(true)
                .focus_ring(2.0, 2.0, t.accent)
                .child(text("Open").font(sans(rem(0.75), 400.0, 1.33)).color(t.text))
                .on_press(open_page(id)),
        );
    }
    if let Some(panel) = own_panel(s, t, p, format!("plugin-panel-{id}")) {
        r = r.child(row().w_full().pl(42.0).child(panel));
    }
    r
}

/// The page: the header, then the overview.
pub fn content(s: &State, t: &Theme) -> El {
    let heading = row()
        .w_full()
        .items_center()
        .justify_between()
        .gap(14.0)
        .child(text("All plugins").font(sans(rem(0.875), 600.0, 1.43)).color(t.text))
        .child(text("Each switch adds or removes everything below").font(sans(rem(0.75), 400.0, 1.33)).color(t.muted));
    // Scene and test only: Fleet's "No plugins are installed" can't happen with the three built in.
    let list = if s.settings.plugins_none { &[] } else { plugins::PLUGINS.as_slice() };
    let mut card = col()
        .key("plugins-list")
        .w_full()
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.card_bg)
        .clip_rounded(RADIUS_CARD);
    if list.is_empty() {
        card = card.child(super::plugin_page::empty(t, "plugins-empty", "No plugins are installed", false));
    }
    for (i, p) in list.iter().enumerate() {
        card = card.child(plugin_row(s, t, p, i + 1 == list.len()));
    }
    col()
        .key("plugins-overview")
        .w_full()
        .gap(21.0)
        .child(super::page_header(s, t, &Page::Plugins))
        .child(col().w_full().gap(10.5).child(heading).child(card))
}
