//! The sources form in manage mode (`BoardSourceConfig.vue`): the header with the plugin's words, the error band, the form that
//! adds a source from a bookmarked repository, and the list of sources the board syncs from. Sits under the tiles; the state it
//! reads is `fleet::board::source_form`, and the place that offers the repositories is `fleet::places::BoardSourcePlace`.
use crate::State;
use crate::fleet::board::source_form::SourceForm;
use crate::fleet::board::sources::{self as read, SourceRow};
use crate::fleet::places::BoardSourcePlace;
use crate::icons::{CHECK, CHEVRON_DOWN};
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, sans};
use crate::ui::button::{Size, Variant, button, disabled};
use crate::ui::{Open, age, empty_state, select};
use crate::view::board::manage::{Look, field};
use crate::view::session_row::RowClock;
use rune::{Color, Cursor, El, Key, KeyEvent, Tx, col, el, icon, row, text};
use std::rc::Rc;

/// The repository select's name in `Open::Select`, and its anchor.
pub const REPO_SELECT: &str = "board-source-repo";
const REPO_KEY: &str = "board-source-repo-select";

/// `.board-source-config__label`: 10 px capitals.
fn label(t: &Theme, key: &str, words: &str) -> El {
    text(words.to_uppercase()).key(key.to_string()).font(sans(10.0, 600.0, 1.5).tracking(0.4)).color(t.muted)
}

/// The 12 px muted paragraphs (`__description`, `__helper`, `__meta`, `__empty`).
fn muted(t: &Theme, key: &str, words: &str) -> El {
    text(words.to_string()).key(key.to_string()).font(sans(12.0, 400.0, 1.5)).color(t.muted).wrap()
}

/// `.board-source-config__form` and friends: the card that holds the form, the list, the notice and the band.
fn card(t: &Theme, key: &str, base: El) -> El {
    base.key(key.to_string()).w_full().padding(12.0).radius(RADIUS_CARD).border(1.0, t.border).border_layout().fill(t.card_bg)
}

fn header(t: &Theme, place: Option<&BoardSourcePlace>) -> El {
    let title = text("Sync board cards from bookmarked repositories".to_string())
        .key("board-source-title")
        .font(sans(18.0, 700.0, 1.5))
        .color(t.text)
        .wrap();
    let eyebrow = label(t, "board-source-eyebrow", "Sources").mb(6.0);
    let mut copy = col().w_full().child(eyebrow).child(title);
    if let Some(place) = place {
        copy = copy.child(muted(t, "board-source-description", place.description));
    }
    copy
}

/// `.board-source-config__banner` (`role=alert`): the source error, else the bookmarks', and a retry for the bookmarks'.
fn banner(t: &Theme, error: &str, retry: bool) -> El {
    let (ink, border, fill) = t.board_banner();
    let copy = text(error.to_string()).key("board-source-error").font(sans(12.0, 400.0, 1.5)).color(ink).wrap();
    let mut b = row()
        .key("board-source-banner")
        .role("alert")
        .w_full()
        .center_y()
        .justify_between()
        .gap(12.0)
        .padding(12.0)
        .radius(RADIUS_CARD)
        .border(1.0, border)
        .border_layout()
        .fill(fill)
        .child(copy.shrink().min_w(0.0));
    if retry {
        let again =
            button(t, Variant::Ghost, Size::Sm, None, "Try loading bookmarks again").key("board-source-retry").on_press(|s: &mut State| {
                if let Some(place) = s.board_source() {
                    (place.refresh)(s);
                }
            });
        b = b.child(again);
    }
    b
}

/// The native `<select>`: the picked name or the prompt, a chevron, in the form's field look.
fn repo_select(t: &Theme, form: &SourceForm, loading_bookmarks: bool, off: bool) -> El {
    let shown = if form.repository.is_empty() {
        if loading_bookmarks { "Loading bookmarks…" } else { "Select a repository" }.to_string()
    } else {
        form.repository.clone()
    };
    let b = row()
        .key(REPO_KEY)
        .role("combobox")
        .label("Bookmarked repository".to_string())
        .w_full()
        .min_w(0.0)
        .center_y()
        .justify_between()
        .gap(8.0)
        .px(12.0)
        .py(10.0)
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.board_faint_fill())
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(1.0, 0.0, t.accent)
        .child(text(shown).font(sans(14.0, 400.0, 1.4)).color(t.text).truncate().shrink().min_w(0.0))
        .child(icon(&CHEVRON_DOWN, 14.0).color(t.muted));
    if off {
        return b.opacity(0.6).disabled(true).cursor(Cursor::Default);
    }
    b.on_press(|s: &mut State| s.ui.open = Some(Open::Select(REPO_SELECT)))
}

/// The repositories the select offers, sorted by name (`localeCompare`, here a case-folded compare).
fn options(place: &BoardSourcePlace, s: &State) -> Vec<String> {
    let mut all = (place.repositories)(s);
    all.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()).then_with(|| a.cmp(b)));
    all
}

/// The open list of the select: "Select a repository" (empty) then each bookmark.
pub fn repo_list(s: &State, t: &Theme) -> Vec<El> {
    let Some(place) = s.board_source() else { return Vec::new() };
    let prompt = if (place.is_loading)(s) { "Loading bookmarks…" } else { "Select a repository" };
    let names = options(&place, s);
    let mut rows: Vec<(&str, &str)> = vec![("", prompt)];
    rows.extend(names.iter().map(|n| (n.as_str(), n.as_str())));
    let choose: select::Choose = Rc::new(|s: &mut State, v: &str| s.board.source_form.repository = v.to_string());
    select::content(t, "board-source-repo-list", REPO_KEY, 360.0, &rows, Some(&s.board.source_form.repository), choose)
}

fn labels_field(t: &Theme, form: &SourceForm, off: bool) -> El {
    field(
        t,
        &form.labels,
        "board-source-label-filter".into(),
        "Label filter (optional)",
        "bug, priority:high",
        Look { pad: (10.0, 12.0), fill: t.board_faint_fill() },
        off,
    )
    .w_full()
    .min_w(0.0)
    .opacity(if off { 0.6 } else { 1.0 })
    .on_edit(|s: &mut State| &mut s.board.source_form.labels)
    .on_input_key(|s: &mut State, ev: &KeyEvent, _: &mut Tx| {
        if ev.key == Key::Enter {
            s.board_source_add();
            return true;
        }
        false
    })
}

/// `.board-source-config__toggle-field`: a 16 px checkbox beside "Assigned to me" and its line.
fn assigned_field(t: &Theme, on: bool, off: bool) -> El {
    let mark = el()
        .size(16.0, 16.0)
        .center()
        .radius(3.0)
        .border(1.0, if on { t.accent } else { t.border })
        .fill(if on { t.accent } else { Color::TRANSPARENT })
        .children(on.then(|| icon(&CHECK, 12.0).color(t.primary_fg)));
    let copy = col()
        .gap(2.0)
        .child(label(t, "board-source-assigned-label", "Assigned to me"))
        .child(text("Only sync issues assigned to you.".to_string()).font(sans(11.0, 400.0, 1.4)).color(t.muted).wrap());
    let b = row()
        .key("board-source-assigned-to-me")
        .role("checkbox")
        .label("Assigned to me".to_string())
        .checked(on)
        .w(0.0)
        .grow(0.9)
        .min_w(0.0)
        .min_h(42.0)
        .center_y()
        .gap(10.0)
        .px(12.0)
        .py(10.0)
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.board_faint_fill())
        .focusable(true)
        .focus_ring(1.0, 0.0, t.accent)
        .child(mark)
        .child(copy.shrink().min_w(0.0));
    if off {
        return b.opacity(0.6).disabled(true).cursor(Cursor::Default);
    }
    b.cursor(Cursor::Pointer).on_press(|s: &mut State| s.board_source_toggle_assigned())
}

fn form(s: &State, t: &Theme, place: &BoardSourcePlace) -> El {
    let f = &s.board.source_form;
    let off = f.fields_off();
    let select_off = (place.is_loading)(s) || off;
    let repo = col()
        .w(0.0)
        .grow(1.4)
        .min_w(0.0)
        .gap(6.0)
        .child(label(t, "board-source-repo-label", "Bookmarked repository"))
        .child(repo_select(t, f, (place.is_loading)(s), select_off));
    let labels = col()
        .w(0.0)
        .grow(1.0)
        .min_w(0.0)
        .gap(6.0)
        .child(label(t, "board-source-labels-label", "Label filter (optional)"))
        .child(labels_field(t, f, off));
    let add = button(t, Variant::Default, Size::Sm, None, if f.creating { "Adding…" } else { "Add source" }).key("board-source-submit");
    let add = if f.can_submit() { add.on_press(|s: &mut State| s.board_source_add()) } else { disabled(add) };
    card(t, "board-source-form", row())
        .gap(12.0)
        .items_end()
        .child(repo)
        .child(labels)
        .child(assigned_field(t, f.assigned_to_me, off))
        .child(add)
}

/// `.board-source-config__helper`: the place's hint with no repositories (and none loading), else the duplicate line.
fn helper(s: &State, t: &Theme, place: &BoardSourcePlace) -> Option<El> {
    if (place.repositories)(s).is_empty() && !(place.is_loading)(s) {
        return Some(muted(t, "board-source-helper", place.empty_hint));
    }
    s.board.source_form.is_duplicate().then(|| muted(t, "board-source-helper", "This repository and label filter is already configured."))
}

/// "Labels: bug", "Assignee: @me" and "Last sync: Nov 4, 9:00 AM", each with its other words, between bullets. Fleet's spans sit
/// with no space round the bullets (the template's whitespace between elements is condensed away), so they are drawn so.
fn meta(r: &SourceRow, clock: &RowClock) -> String {
    let labels = r.labels.as_ref().map_or("No label filter".to_string(), |l| format!("Labels: {l}"));
    let assignee = r.assignee.as_ref().map_or("Any assignee".to_string(), |a| format!("Assignee: {a}"));
    let sync = match &r.last_sync_at {
        None => "Not synced yet".to_string(),
        Some(at) => {
            Some(age::exact(at, clock.now, clock.offset_secs)).filter(|t| !t.is_empty()).unwrap_or_else(|| "Unknown sync time".into())
        }
    };
    format!("{labels}•{assignee}•Last sync: {sync}")
}

fn item(s: &State, t: &Theme, r: &SourceRow, clock: &RowClock) -> El {
    let f = &s.board.source_form;
    let id = r.id.clone();
    let pill = text(r.provider_type.to_uppercase())
        .key(format!("board-source-{id}-provider"))
        .font(sans(10.0, 700.0, 1.5).tracking(0.4))
        .color(t.muted);
    let pill = row().center_y().px(8.0).py(2.0).radius(999.0).fill(Color::WHITE.alpha(0.06)).child(pill);
    let name = text(r.repository.clone()).key(format!("board-source-{id}-repository")).font(sans(13.0, 600.0, 1.5)).color(t.text);
    let copy = col().gap(4.0).grow(1.0).min_w(0.0).child(row().flex_wrap().center_y().gap(8.0).child(pill).child(name)).child(muted(
        t,
        &format!("board-source-{id}-meta"),
        &meta(r, clock),
    ));
    let removing = f.removing.as_deref() == Some(&r.id);
    let remove =
        button(t, Variant::Ghost, Size::Sm, None, if removing { "Removing…" } else { "Remove" }).key(format!("board-source-{id}-remove"));
    let remove = if removing || f.creating {
        disabled(remove)
    } else {
        let id = r.id.clone();
        remove.on_press(move |s: &mut State| s.board_source_remove(&id))
    };
    row()
        .key(format!("board-source-{id}"))
        .w_full()
        .center_y()
        .justify_between()
        .gap(12.0)
        .padding(12.0)
        .radius(10.0)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.board_faint_fill())
        .child(copy)
        .child(remove)
}

fn list(s: &State, t: &Theme) -> El {
    let f = &s.board.source_form;
    if f.loading {
        return card(t, "board-source-loading", col()).child(muted(t, "board-source-loading-text", "Loading configured sources…"));
    }
    let rows = read::rows(&f.sources);
    if rows.is_empty() {
        return empty_state::bordered_compact(t, "board-source-empty", "No configured sources yet");
    }
    let clock = RowClock::now(s);
    card(t, "board-source-list", col()).gap(10.0).children(rows.iter().map(|r| item(s, t, r, &clock)))
}

/// `BoardSourceConfig`, in manage mode only.
pub fn view(s: &State, t: &Theme) -> Option<El> {
    if !s.board.is_manage_mode() {
        return None;
    }
    let place = s.board_source();
    let f = &s.board.source_form;
    let bookmarks = place.as_ref().and_then(|p| (p.error)(s));
    let mut body = col()
        .key("board-sources")
        .role("region")
        .label("Board source configuration".to_string())
        .w_full()
        .px(24.0)
        .pt(12.0)
        .gap(12.0)
        .child(header(t, place.as_ref()));
    if let Some(error) = f.error.clone().or_else(|| bookmarks.clone()) {
        body = body.child(banner(t, &error, bookmarks.is_some()));
    }
    if !s.board.has_board() {
        return Some(body.child(empty_state::compact_bare(
            t,
            "board-source-no-board",
            "Create a board lane first to enable source syncing",
        )));
    }
    if let Some(place) = &place {
        body = body.child(form(s, t, place));
        body = body.children(helper(s, t, place));
    }
    Some(body.child(list(s, t)))
}
