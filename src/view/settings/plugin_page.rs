//! A plugin's own page (`PluginPage.vue`): the breadcrumb, the header with its switch, the plugin's own settings while it is on
//! (or a note while it is off), what it adds to Fleet, and where it comes from. It is read from the table in
//! `fleet::settings::plugins`. Each plugin's own settings are its piece's: GitHub's connection page (the plugin's `view::connect`), Mods (`mods_plugin`) and the
//! Built-in skills tab (`built_in_skills`).
use super::plugins::{built_in, glyph, own_panel, switch};
use crate::State;
use crate::fleet::settings::plugins::{self, Plugin};
use crate::icons::*;
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, sans};
use crate::ui::rem;
use rune::{Cursor, El, col, icon, row, text};

/// `EmptyState`: the 28 px icon, the title in 14 px semibold, padding 21. `bordered` is the dashed card Settings' lists use.
pub(super) fn empty(t: &Theme, key: &str, title: &str, bordered: bool) -> El {
    let mut c = col()
        .key(key.to_string())
        .w_full()
        .center()
        .gap(3.5)
        .padding(21.0)
        .child(icon(&PUZZLE, 28.0).color(t.muted).mb(7.0))
        .child(text(title.to_string()).font(sans(14.0, 600.0, 1.4)).color(t.text).wrap().align(rune::Align::Center));
    if bordered {
        c = c.radius(RADIUS_CARD).border(1.0, t.border).border_layout().border_dash(3.0, 3.0);
    }
    c
}

fn breadcrumb(t: &Theme, name: &str) -> El {
    let small = sans(rem(0.75), 400.0, 1.33);
    let plugins_link = row()
        .key("plugin-breadcrumb-plugins")
        .role("button")
        .label("Plugins")
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 0.0, t.accent)
        .child(text("Plugins").font(small).color(t.muted))
        .on_press(|s: &mut State| s.open_settings("plugins", None));
    row()
        .key("plugin-breadcrumb")
        .label("Breadcrumb")
        .items_center()
        .gap(3.5)
        .child(text("Settings").font(small).color(t.muted))
        .child(icon(&CHEVRON_RIGHT, 12.0).color(t.muted))
        .child(plugins_link)
        .child(icon(&CHEVRON_RIGHT, 12.0).color(t.muted))
        .child(text(name.to_string()).font(small).color(t.text))
}

/// The sub line: On or Off (a switchless plugin has neither), the status while it is on, then the description.
fn sub_line(s: &State, t: &Theme, p: &Plugin) -> El {
    let font = sans(rem(0.875), 400.0, 1.43);
    let on = p.enabled(&s.pickers.prefs);
    let mut line = row().key("plugin-page-line").flex_wrap().items_center();
    if !p.switchless {
        line = line.child(text(if on { "On" } else { "Off" }).font(font).color(if on { t.running_text } else { t.muted }));
        let status = s.plugins.statuses.iter().find(|(id, _)| id == p.id).map(|(_, st)| st.as_str());
        if let (true, Some(word)) = (on, Plugin::status_word(status)) {
            line = line.child(text(format!(" \u{b7} {word}")).font(font).color(t.muted));
        }
    }
    if !p.description.is_empty() {
        let lead = if p.switchless { "" } else { " \u{b7} " };
        line = line.child(text(format!("{lead}{}", p.description)).font(font).color(t.muted));
    }
    line
}

fn header(s: &State, t: &Theme, p: &Plugin) -> El {
    let tile = row()
        .size(rem(2.5), rem(2.5))
        .center()
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.card_bg)
        .child(icon(glyph(&s.places, p.id), 20.0).color(t.text));
    let title = row()
        .items_center()
        .gap(7.0)
        .child(text(p.name).font(sans(rem(1.25), 600.0, 1.4).tracking(-0.0125 * rem(1.25))).color(t.text))
        .child(built_in(t, "plugin-page-built-in".into()));
    let words = col().grow(1.0).shrink().min_w(0.0).child(title).child(sub_line(s, t, p));
    let mut h = row().key("plugin-page-header").w_full().items_start().gap(10.5).child(tile).child(words);
    if !p.switchless {
        h = h.child(switch(s, t, p, "plugin-page-switch".into()));
    }
    h
}

fn heading_row(t: &Theme, key: &str, left: &str, right: String) -> El {
    row()
        .key(key.to_string())
        .w_full()
        .items_center()
        .justify_between()
        .gap(14.0)
        .child(text(left.to_string()).font(sans(rem(0.875), 600.0, 1.43)).color(t.text))
        .child(text(right).font(sans(rem(0.75), 400.0, 1.33)).color(t.muted))
}

/// The plugin's own settings slot: GitHub's connection page, Mods' box and the Built-in skills body.
fn own_settings(s: &State, t: &Theme, p: &Plugin) -> El {
    if let Some(body) = s.places.settings_body(p.id, &s.pickers.prefs) {
        return body(s, t);
    }
    if p.id == "mods" {
        return super::mods_plugin::page(t);
    }
    if p.id == "built-in-skills" {
        return super::built_in_skills::body(s, t);
    }
    row()
        .key(format!("plugin-config-{}", p.id))
        .w_full()
        .px(14.0)
        .py(10.5)
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .border_layout()
        .child(text(format!("{} isn't in the native app yet.", p.page_title)).font(sans(rem(0.875), 400.0, 1.5)).color(t.muted))
}

fn off_note(t: &Theme, p: &Plugin) -> El {
    let tail = if p.own_switch {
        " Turn it on with the switch above."
    } else {
        " Its rail icon, settings and pickers are gone until you turn it on."
    };
    row()
        .key("plugin-off-note")
        .w_full()
        .padding(14.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .border_dash(3.0, 3.0)
        .fill(t.card_bg)
        .child(text(format!("{} is off.{tail}", p.name)).font(sans(rem(0.875), 400.0, 1.43)).color(t.muted).wrap().shrink().min_w(0.0))
}

fn additions(t: &Theme, p: &Plugin) -> El {
    let n = p.adds.len();
    let mut list = col()
        .key("plugin-additions")
        .w_full()
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.card_bg)
        .clip_rounded(RADIUS_CARD);
    for (i, a) in p.adds.iter().enumerate() {
        let detail = rune::rich(
            format!("{} {}", a.detail, a.point),
            vec![rune::Span::new(a.detail.len() + 1, a.detail.len() + 1 + a.point.len()).mono().size(rem(0.75)).color(t.muted)],
        )
        .font(sans(rem(0.875), 400.0, 1.43))
        .color(t.text)
        .wrap()
        .shrink()
        .min_w(0.0);
        let mut r = row()
            .key(format!("plugin-addition-{}", i))
            .w_full()
            .items_start()
            .gap(10.5)
            .px(14.0)
            .py(8.75)
            .child(text(a.area.label()).font(sans(rem(0.875), 400.0, 1.43)).color(t.muted).w(rem(8.0)))
            .child(detail);
        if i + 1 < n {
            r = r.border_sides([0.0, 0.0, 1.0, 0.0], t.border).border_layout();
        }
        list = list.child(r);
    }
    col()
        .w_full()
        .child(
            heading_row(t, "plugin-additions-heading", "Adds to Fleet", format!("{n} {}", if n == 1 { "thing" } else { "things" })).mb(7.0),
        )
        .child(list)
}

fn source(t: &Theme) -> El {
    col().w_full().child(text("Source").font(sans(rem(0.875), 600.0, 1.43)).color(t.text).mb(7.0)).child(
        row()
            .key("plugin-source")
            .w_full()
            .px(14.0)
            .py(8.75)
            .radius(RADIUS_CARD)
            .border(1.0, t.border)
            .border_layout()
            .fill(t.card_bg)
            .child(
                text("Built in to Fleet. It ships with the app and updates with it.")
                    .font(sans(rem(0.875), 400.0, 1.43))
                    .color(t.muted)
                    .wrap()
                    .shrink()
                    .min_w(0.0),
            ),
    )
}

/// The page: the header over it (the plugin's name and description, or "Plugin" for an id nobody knows), then this.
pub fn content(s: &State, t: &Theme, id: &str) -> El {
    let page = super::page_header(s, t, &crate::fleet::settings_nav::Page::Plugin(id.to_string()));
    let Some(p) = plugins::find(&s.places, id) else {
        return col().key(format!("plugin-page-{id}")).w_full().gap(21.0).child(page).child(
            col().w_full().gap(17.5).child(breadcrumb(t, id)).child(empty(
                t,
                "plugin-not-found",
                &format!("No plugin called \"{id}\" is installed"),
                true,
            )),
        );
    };
    let on = p.enabled(&s.pickers.prefs);
    let mut body = col().w_full().gap(17.5).child(breadcrumb(t, p.name)).child(header(s, t, &p));
    if let Some(panel) = own_panel(s, t, &p, "plugin-page-panel".into()) {
        body = body.child(panel);
    }
    if on {
        body = body
            .child(heading_row(t, "plugin-settings-heading", "Settings", format!("{}'s own page", p.name)))
            .child(own_settings(s, t, &p));
    } else {
        body = body.child(off_note(t, &p));
    }
    if !p.adds.is_empty() {
        body = body.child(additions(t, &p));
    }
    col().key(format!("plugin-page-{id}")).w_full().gap(21.0).child(page).child(body.child(source(t)))
}
