//! The folder browser behind Browse (`DirectoryPickerPopover.vue`, `mode="navigate"`): the roots or a breadcrumb bar, a search
//! box, the folders, and "Use this directory". Clicking a folder goes into it; Select on its row picks it.
use crate::State;
use crate::fleet::settings::locations::{self, Browse};
use crate::fleet::settings::locations_actions::BROWSE_KEY;
use crate::icons::{CHECK, CHEVRON_RIGHT, FOLDER, FOLDER_GIT_2, GIT_BRANCH, REFRESH_CW};
use crate::theme::{Theme, mono, sans};
use crate::ui::overlay::{anchored, scrim};
use crate::ui::text_menu::text_input;
use crate::ui::{confirm, rem};
use rune::{Align, Cursor, El, ScrollSpec, Side, col, icon, row, text};

fn crumb(t: &Theme, key: String, label: &str, to: Option<String>, last: bool) -> El {
    text(label.to_string())
        .key(key)
        .font(sans(rem(0.75), if last { 500.0 } else { 400.0 }, 1.33))
        .color(if last { t.text } else { t.muted })
        .cursor(Cursor::Pointer)
        .max_w(80.0)
        .truncate()
        .on_press(move |s: &mut State| s.browse_locations(to.clone()))
}

/// The bar over the list: "Roots" and the path as crumbs, or "Locations" and a refresh at the top level.
fn bar(b: &Browse, t: &Theme) -> El {
    let line = row().w_full().border_sides([0.0, 0.0, 1.0, 0.0], t.border).center_y();
    let Some(current) = &b.current else {
        let refresh = row()
            .key("locations-browse-refresh")
            .role("button")
            .label("Refresh")
            .size(28.0, 28.0)
            .center()
            .radius(6.0)
            .hover(t.main_bg)
            .cursor(Cursor::Pointer)
            .opacity(if b.loading { 0.5 } else { 1.0 })
            .child(icon(&REFRESH_CW, 16.0).color(t.text))
            .on_press(|s: &mut State| s.browse_locations(None));
        return line
            .justify_between()
            .padding(7.0)
            .child(text("Locations").font(mono(rem(0.75), 400.0, 1.33)).color(t.text))
            .child(refresh);
    };
    let crumbs = locations::breadcrumbs(current, &b.roots);
    let last = crumbs.len().saturating_sub(1);
    let mut r = line.gap(3.5).px(10.5).py(7.0).child(crumb(t, "locations-browse-roots".into(), "Roots", None, false));
    for (i, (name, path)) in crumbs.into_iter().enumerate() {
        r = r.child(icon(&CHEVRON_RIGHT, 12.0).color(t.muted)).child(crumb(
            t,
            format!("locations-browse-crumb-{i}"),
            &name,
            Some(path),
            i == last,
        ));
    }
    r
}

fn entry(t: &Theme, i: usize, e: &locations::Entry) -> El {
    let (go, pick) = (e.path.clone(), e.path.clone());
    let select = text("Select")
        .key(format!("locations-browse-select-{i}"))
        .font(sans(10.0, 400.0, 1.4))
        .color(rune::Color::TRANSPARENT)
        .group_color(t.muted)
        .px(5.25)
        .py(1.75)
        .cursor(Cursor::Pointer)
        .on_press(move |s: &mut State| s.pick_location_folder(&pick));
    let mut r = row()
        .key(format!("locations-browse-entry-{i}"))
        .role("button")
        .label(e.name.clone())
        .w_full()
        .gap(7.0)
        .px(7.0)
        .py(5.25)
        .center_y()
        .hover(t.main_bg)
        .hover_group()
        .cursor(Cursor::Pointer)
        .child(icon(if e.git { &FOLDER_GIT_2 } else { &FOLDER }, 12.25).color(t.text))
        .child(text(e.name.clone()).font(mono(rem(0.875), 400.0, 1.43)).color(t.text).truncate().grow(1.0).shrink().min_w(0.0));
    if e.git {
        r = r.child(icon(&GIT_BRANCH, 10.5).color(t.muted));
    }
    r.child(select).on_press(move |s: &mut State| s.browse_locations(Some(go.clone())))
}

fn list(b: &Browse, t: &Theme) -> El {
    let needle = b.search.text.trim().to_lowercase();
    let shown: Vec<&locations::Entry> =
        b.entries.iter().filter(|e| needle.is_empty() || format!("{} {}", e.name, e.path).to_lowercase().contains(&needle)).collect();
    let mut c = col().w_full().padding(3.5);
    for (i, e) in shown.iter().enumerate() {
        c = c.child(entry(t, i, e));
    }
    let note = |key: &str, words: &str, ink| {
        text(words.to_string()).key(key.to_string()).font(sans(TEXT_SM, 400.0, 1.43)).color(ink).px(7.0).py(14.0)
    };
    if b.loading {
        c = c.child(
            row()
                .key("locations-browse-loading")
                .px(7.0)
                .py(14.0)
                .gap(7.0)
                .center_y()
                .child(confirm::spinner(t.muted))
                .child(text("Loading…").font(sans(TEXT_SM, 400.0, 1.43)).color(t.muted)),
        );
    } else if let Some(why) = &b.error {
        c = c.child(note("locations-browse-error", why, t.error_text));
    } else if shown.is_empty() && b.current.is_none() {
        c = c.child(note("locations-browse-none", "No locations yet. Add one in Settings → Folders.", t.muted));
    } else if shown.is_empty() {
        c = c.child(note("locations-browse-none", "No subdirectories found.", t.muted));
    }
    col().w_full().h(252.0).child(c).scroll(ScrollSpec { stick_bottom: false }).key("locations-browse-list")
}

const TEXT_SM: f32 = 12.25;

/// The popover and its scrim, under the Browse button.
pub fn overlay(s: &State, t: &Theme) -> Vec<El> {
    let Some(b) = &s.settings.locations.browse else { return vec![] };
    let search = row().w_full().px(7.0).py(5.25).border_sides([0.0, 0.0, 1.0, 0.0], t.border).child(
        text_input(&b.search, sans(TEXT_SM, 400.0, 1.43))
            .key("locations-browse-search")
            .label("Search directories")
            .single_line()
            .placeholder("Search directories…".to_string(), t.muted)
            .color(t.text)
            .caret(t.text, t.accent.alpha(0.35))
            .w_full()
            .px(10.5)
            .py(5.25)
            .radius(8.0)
            .border(1.0, if b.search.focused { t.accent } else { t.border })
            .border_layout()
            .fill(t.main_bg)
            .on_edit(|s: &mut State| s.settings.locations.browse.as_mut().map(|b| &mut b.search).expect("an open browser")),
    );
    let mut p = col()
        .key("locations-popover")
        .role("dialog")
        .label("Choose a folder")
        .w(448.0)
        .fill(t.card_bg)
        .border(1.0, t.border)
        .border_layout()
        .child(bar(b, t))
        .child(search)
        .child(list(b, t))
        .modal(true, |s: &mut State| s.ui.open = None)
        .focus_return(BROWSE_KEY.to_string());
    if let Some(current) = b.current.clone() {
        p = p.child(
            row().w_full().padding(7.0).border_sides([1.0, 0.0, 0.0, 0.0], t.border).child(
                row()
                    .key("locations-browse-use")
                    .role("button")
                    .label("Use this directory")
                    .w_full()
                    .h(31.5)
                    .gap(7.0)
                    .center()
                    .radius(8.0)
                    .fill(t.accent)
                    .cursor(Cursor::Pointer)
                    .child(icon(&CHECK, 16.0).color(t.primary_fg))
                    .child(text("Use this directory").font(sans(TEXT_SM, 500.0, 1.43)).color(t.primary_fg))
                    .on_press(move |s: &mut State| s.pick_location_folder(&current)),
            ),
        );
    }
    for sh in t.menu_shadows() {
        p = p.box_shadow(sh);
    }
    vec![scrim("locations-browse-scrim", |s: &mut State| s.ui.open = None), anchored(p, BROWSE_KEY, Side::Bottom, Align::End, 4.0)]
}
