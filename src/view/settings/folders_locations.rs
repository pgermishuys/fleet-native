//! Settings → Folders, the Folders card (`WorkspaceSection.vue`): where Fleet looks for repositories, one card per location with
//! its chips, and the box that adds another with the folder browser beside it (`folders_browse.rs`).
use crate::State;
use crate::fleet::settings::locations::{LocationsPage, Root};
use crate::icons::{FOLDER, FOLDER_GIT_2, PLUS, REFRESH_CW, TRASH_2};
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, sans};
use crate::ui::text_menu::text_input;
use crate::ui::{confirm, rem};
use rune::{Align, Cursor, El, col, icon, row, text};

const TEXT_SM: f32 = 12.25;

fn small(t: &Theme, words: &str) -> El {
    text(words.to_string()).font(sans(rem(0.75), 400.0, 1.33)).color(t.muted).wrap().shrink().min_w(0.0)
}

fn word(t: &Theme, words: &str, weight: f32) -> El {
    text(words.to_string()).font(sans(TEXT_SM, weight, 1.43)).color(t.text)
}

/// `buttonSecondaryClass` / `buttonPrimaryClass` / Remove: `rounded-btn px-3 py-1.5 text-sm font-medium`, a gap of 8 around the glyph.
pub(super) fn pill(base: El, key: &str, label: &str, disabled: bool, lead: El, words: El, press: impl Fn(&mut State) + 'static) -> El {
    base.key(key.to_string())
        .role("button")
        .label(label.to_string())
        .gap(7.0)
        .px(10.5)
        .py(5.25)
        .center()
        .radius(RADIUS_BTN)
        .focusable(true)
        .focus_ring(2.0, 1.0, rune::Color::TRANSPARENT)
        .cursor(if disabled { Cursor::Default } else { Cursor::Pointer })
        .opacity(if disabled { 0.6 } else { 1.0 })
        .disabled(disabled)
        .child(lead)
        .child(words)
        .on_press(press)
}

pub(super) fn secondary(t: &Theme) -> El {
    row().border(1.0, t.border).border_layout().fill(t.main_bg).hover(t.main_bg)
}

fn refresh(page: &LocationsPage, t: &Theme) -> El {
    let off = page.loading || page.refreshing;
    let lead = if page.refreshing { confirm::spinner(t.text) } else { icon(&REFRESH_CW, 16.0).color(t.text) };
    pill(secondary(t), "locations-refresh", "Refresh", off, lead, word(t, "Refresh", 500.0), |s: &mut State| s.refresh_locations())
}

fn loading(t: &Theme) -> El {
    row()
        .key("locations-loading")
        .mt(14.0)
        .gap(7.0)
        .center_y()
        .child(confirm::spinner(t.muted))
        .child(text("Loading your locations…").font(sans(TEXT_SM, 400.0, 1.43)).color(t.muted))
}

/// `EmptyState bordered`: a dashed card, the icon, the title and the line.
fn empty(t: &Theme) -> El {
    col()
        .key("locations-empty")
        .w_full()
        .mt(14.0)
        .center()
        .gap(3.5)
        .padding(21.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .border_dash(3.0, 3.0)
        .child(icon(&FOLDER_GIT_2, 28.0).color(t.muted).mb(7.0))
        .child(text("No locations yet").font(sans(14.0, 600.0, 1.4)).color(t.text))
        .child(
            text("Add the folder you keep your code in, such as ~/source, and Fleet lists the repositories inside it.")
                .font(sans(TEXT_SM, 400.0, 1.5))
                .color(t.muted)
                .wrap()
                .align(Align::Center)
                .max_w(455.0),
        )
}

fn chip(t: &Theme, key: String, words: &str, missing: bool) -> El {
    let (border, fill, ink) =
        if missing { (t.error.alpha(0.3), t.error.alpha(0.1), t.error_text) } else { (t.border, rune::Color::TRANSPARENT, t.muted) };
    row()
        .key(key)
        .px(7.0)
        .py(3.5)
        .radius(999.0)
        .border(1.0, border)
        .border_layout()
        .fill(fill)
        .child(text(words.to_string()).font(sans(rem(0.75), 400.0, 1.33)).color(ink))
}

fn remove(page: &LocationsPage, t: &Theme, i: usize, id: &str) -> El {
    let busy = page.removing.as_deref() == Some(id);
    let lead = if busy { confirm::spinner(t.error_text) } else { icon(&TRASH_2, 16.0).color(t.error_text) };
    let label = if busy { "Removing…" } else { "Remove" };
    let id = id.to_string();
    let base = row().border(1.0, t.error.alpha(0.4)).border_layout().fill(t.error.alpha(0.1)).hover(t.error.alpha(0.2));
    let words = text(label.to_string()).font(sans(TEXT_SM, 500.0, 1.43)).color(t.error_text);
    pill(base, &format!("location-remove-{i}"), "Remove", busy, lead, words, move |s: &mut State| s.remove_location(&id))
}

/// `article`: the path, the chips, and Remove for a location that was added.
fn card(page: &LocationsPage, t: &Theme, i: usize, root: &Root) -> El {
    let mut chips = row().mt(3.5).flex_wrap().center_y().gap(7.0).child(chip(t, format!("location-source-{i}"), root.source_word(), false));
    if !root.exists {
        chips = chips.child(chip(t, format!("location-missing-{i}"), "Missing on disk", true));
    }
    let words = col()
        .grow(1.0)
        .shrink()
        .min_w(0.0)
        .child(text(root.path.clone()).key(format!("location-path-{i}")).font(sans(TEXT_SM, 500.0, 1.43)).color(t.text).truncate())
        .child(chips);
    let mut line = row()
        .key(format!("location-{i}"))
        .w_full()
        .gap(10.5)
        .center_y()
        .justify_between()
        .px(14.0)
        .py(14.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.main_bg)
        .child(words);
    if let Some(id) = root.removable() {
        line = line.child(remove(page, t, i, id));
    }
    line
}

fn list(page: &LocationsPage, t: &Theme) -> El {
    let mut c = col().key("locations-list").w_full().mt(14.0).gap(10.5);
    for (i, root) in page.roots.iter().enumerate() {
        c = c.child(card(page, t, i, root));
    }
    c
}

/// `inputClass`: `rounded-btn border bg-main-bg px-3 py-2 text-sm`, the border goes to the accent colour on focus.
fn path_box(page: &LocationsPage, t: &Theme) -> El {
    text_input(&page.path, sans(TEXT_SM, 400.0, 1.43))
        .key("locations-path")
        .label("Folder to add")
        .single_line()
        .placeholder("/Users/you/projects".to_string(), t.muted)
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .grow(1.0)
        .min_w(0.0)
        .px(10.5)
        .py(7.0)
        .radius(RADIUS_BTN)
        .border(1.0, if page.path.focused { t.accent } else { t.border })
        .border_layout()
        .fill(t.main_bg)
        .disabled(page.adding)
        .on_edit(|s: &mut State| &mut s.settings.locations.path)
        .on_submit(|s: &mut State, _| s.add_location())
}

fn add_card(page: &LocationsPage, t: &Theme) -> El {
    let browse = pill(
        secondary(t),
        crate::fleet::settings::locations_actions::BROWSE_KEY,
        "Browse for a folder",
        page.adding,
        icon(&FOLDER, 16.0).color(t.text),
        row(),
        |s: &mut State| s.open_location_browser(),
    )
    .help("Browse for a folder".to_string());
    let (lead, label) =
        if page.adding { (confirm::spinner(t.primary_fg), "Adding…") } else { (icon(&PLUS, 16.0).color(t.primary_fg), "Add location") };
    let add = pill(
        row().fill(t.accent),
        "locations-add",
        label,
        page.adding,
        lead,
        text(label.to_string()).font(sans(TEXT_SM, 500.0, 1.43)).color(t.primary_fg),
        |s: &mut State| s.add_location(),
    );
    let mut c = col()
        .key("locations-add-card")
        .w_full()
        .mt(21.0)
        .gap(10.5)
        .px(14.0)
        .py(14.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.main_bg)
        .child(
            col()
                .gap(3.5)
                .child(word(t, "Add a location", 600.0))
                .child(small(t, "A folder Fleet looks in for repositories, such as the one you clone into.")),
        )
        .child(row().w_full().gap(10.5).child(path_box(page, t)).child(browse).child(add));
    if let Some(why) = &page.add_error {
        c = c.child(confirm::error_note(t, "locations-add-error", why, None));
    }
    c
}

/// The card (`WorkspaceSection`).
pub fn card_view(s: &State, t: &Theme) -> El {
    let page = &s.settings.locations;
    let mut c = col()
        .key("locations-card")
        .w_full()
        .px(14.0)
        .py(14.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.card_bg)
        .child(
            col().w_full().gap(3.5).child(word(t, "Folders", 600.0)).child(small(
                t,
                "Fleet lists the repositories it finds in these locations, up to three folders deep. New folders and clones go in the one you used last. A folder you open or create anywhere else is added here.",
            )),
        )
        .child(row().w_full().mt(21.0).gap(10.5).center_y().justify_between().child(word(t, "Locations", 600.0)).child(refresh(page, t)));
    c = if page.loading {
        c.child(loading(t))
    } else if let Some(why) = &page.error {
        c.child(col().w_full().mt(14.0).child(confirm::error_note(t, "locations-error", why, None)))
    } else if page.roots.is_empty() {
        c.child(empty(t))
    } else {
        c.child(list(page, t))
    };
    c.child(add_card(page, t))
}
