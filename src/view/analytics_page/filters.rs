//! `AnalyticsFilters.vue` (stacked, as the menu draws it): the range box with its two date fields, the project select and
//! Reset, and Export CSV under them. The calendar glyph in each field opens `ui::calendar` under it.
use crate::State;
use crate::fleet::analytics::calendar::{End, POPOVER};
use crate::fleet::analytics::fields::Fields;
use crate::fleet::analytics::with_selected_project;
use crate::icons::{CALENDAR, DOWNLOAD, ROTATE_CCW};
use crate::theme::{RADIUS_BTN, Theme, sans};
use crate::ui::button::disabled;
use crate::ui::text_menu::text_input;
use crate::ui::{Open, calendar, select};
use rune::{Cursor, El, TextEdit, col, icon, row, text};
use std::rc::Rc;

/// The select's name for `Open::Select` and its list.
pub const PROJECT: &str = "analytics-project";
const ALL: &str = "All projects";

/// `.analytics-filters__date`: 30 tall, 6 px padding, 12.5 px tabular, transparent in the box; the browser's calendar glyph
/// (`::-webkit-calendar-picker-indicator`, 55 %) at its right edge opens the calendar.
fn date_field(t: &Theme, end: End, label: &'static str, edit: &TextEdit, pick: fn(&mut State) -> &mut TextEdit) -> El {
    let key = format!("analytics-filter-{}", end.key());
    let input = text_input(edit, sans(12.5, 400.0, 1.3).features("'tnum' 1"))
        .single_line()
        .key(key.clone())
        .label(label)
        .placeholder("mm/dd/yyyy", t.muted)
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .on_edit(move |s: &mut State| pick(s))
        .grow(1.0)
        .w(0.0)
        .h(30.0)
        .pl(6.0)
        .py((30.0 - 12.5 * 1.3) / 2.0);
    let glyph = row()
        .key(format!("{key}-calendar"))
        .role("button")
        .label(format!("Open {label} calendar"))
        .w(24.0)
        .h(30.0)
        .center()
        .cursor(Cursor::Pointer)
        .focus_ring(2.0, 0.0, t.accent)
        .child(icon(&CALENDAR, 14.0).color(t.text).opacity(0.55))
        .on_press(move |s: &mut State| s.analytics_open_calendar(end));
    row().w(112.0).h(30.0).center_y().child(input).child(glyph)
}

/// `.analytics-filters__range`: one 32 px box with the two fields and the dash, spread apart in the stacked menu.
fn range(s: &State, t: &Theme) -> El {
    let f: &Fields = &s.analytics.fields;
    row()
        .key("analytics-filter-range")
        .role("group")
        .label("Date range")
        .w_full()
        .h(32.0)
        .px(4.0)
        .center_y()
        .justify_between()
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.card_bg)
        .child(date_field(t, End::From, "From", &f.from, |s| &mut s.analytics.fields.from))
        .child(text("\u{2013}").font(sans(12.0, 400.0, 1.0)).color(t.muted))
        .child(date_field(t, End::To, "To", &f.to, |s| &mut s.analytics.fields.to))
}

/// The project the select shows: its name, or "All projects".
fn project_label(s: &State) -> (String, bool) {
    let id = s.analytics.filters.project_id.as_str();
    if id.is_empty() { (ALL.to_string(), true) } else { (id.to_string(), false) }
}

/// `.analytics-filters__project`: 32 tall, 10 px padding, 12.5 px, `--card-bg`, `--border`, the full width of the menu.
fn project_select(s: &State, t: &Theme) -> El {
    let (label, placeholder) = project_label(s);
    row()
        .key(PROJECT)
        .role("combobox")
        .label("Project")
        .w_full()
        // Fleet's stacked rule asks for the full width, but the plain rule after it caps the select at 220.
        .max_w(220.0)
        .h(32.0)
        .px(10.0)
        .gap(8.0)
        .center_y()
        .justify_between()
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.card_bg)
        .cursor(Cursor::Pointer)
        .focus_ring(3.0, 0.0, t.accent.alpha(0.5))
        .child(text(label).font(sans(12.5, 400.0, 1.3)).color(if placeholder { t.muted } else { t.text }).truncate().shrink())
        .child(icon(&crate::icons::CHEVRON_DOWN, 14.0).color(t.muted).opacity(0.5))
        .on_press(|s: &mut State| s.ui.open = Some(Open::Select(PROJECT)))
}

/// `.analytics-filters__reset`: 32 tall, the glyph and "Reset", muted, no border; a 6 % tint on hover; faded while there is
/// nothing to reset.
fn reset(s: &State, t: &Theme) -> El {
    let now = s.now_ms.unwrap_or_else(crate::fleet::link::now_ms);
    let nothing = s.analytics.filters.is_default(now);
    let button = row()
        .key("analytics-filter-reset")
        .role("button")
        .label("Reset")
        .h(32.0)
        .px(10.0)
        .gap(6.0)
        .center_y()
        .radius(RADIUS_BTN)
        .cursor(Cursor::Pointer)
        .focus_ring(2.0, 2.0, t.accent)
        .child(icon(&ROTATE_CCW, 13.0).color(t.muted))
        .child(text("Reset").font(sans(12.5, 400.0, 1.3)).color(t.muted))
        .help("Back to the last 30 days, every project");
    if nothing { disabled(button) } else { button.hover(t.wash(6.0)).on_press(|s: &mut State| s.analytics_reset_filters()) }
}

/// `.analytics-nav__export`: Download 13 and "Export CSV", 32 tall, muted, a 6 % tint and the full ink on hover.
fn export(t: &Theme) -> El {
    row()
        .key("analytics-export")
        .role("button")
        .label("Export CSV")
        .h(32.0)
        .px(10.0)
        .gap(6.0)
        .center_y()
        .radius(RADIUS_BTN)
        .cursor(Cursor::Pointer)
        .hover_group()
        .hover(t.wash(6.0))
        .focus_ring(2.0, 2.0, t.accent)
        .child(icon(&DOWNLOAD, 13.0).color(t.muted).group_color(t.text))
        .child(text("Export CSV").font(sans(12.5, 400.0, 1.3)).color(t.muted).group_color(t.text))
        .on_press(|s: &mut State| s.analytics_export())
}

/// The block under the sections: 4 px above, 8 px of padding under a 1 px rule, the controls 8 apart.
pub fn block(s: &State, t: &Theme) -> El {
    col()
        .key("analytics-filters")
        .role("group")
        .label("Analytics filters")
        .w_full()
        .mt(4.0)
        .pt(8.0)
        .gap(8.0)
        .border_sides([1.0, 0.0, 0.0, 0.0], t.border)
        .border_layout()
        .child(row().px(4.0).child(super::menu::label(t, "Filters")))
        .child(range(s, t))
        .child(project_select(s, t))
        .child(reset(s, t))
        .child(export(t))
}

/// The open project list: "All projects", then the range's top projects and the picked one if it isn't among them.
pub fn list(s: &State, t: &Theme) -> Vec<El> {
    let projects = s.analytics.range.data.as_ref().map(|r| r.top_projects.as_slice()).unwrap_or_default();
    let picked = &s.analytics.filters.project_id;
    let options = with_selected_project(projects, picked);
    let mut all: Vec<(&str, &str)> = vec![("", ALL)];
    all.extend(options.iter().map(|o| (o.name.as_str(), o.name.as_str())));
    let choose: select::Choose = Rc::new(|s: &mut State, name: &str| s.analytics_pick_project(name));
    select::content(t, "analytics-project-list", PROJECT, (s.sidebar_w - 24.0).min(220.0), &all, Some(picked.as_str()), choose)
}

/// The open calendar of one of the date fields, under its glyph.
pub fn calendar_overlay(s: &State, t: &Theme) -> Vec<El> {
    let Some(c) = s.analytics.calendar.filter(|_| s.ui.open == Some(Open::Popover(POPOVER))) else { return Vec::new() };
    let f = &s.analytics.filters;
    let now = s.now_ms.unwrap_or_else(crate::fleet::link::now_ms);
    let today = crate::fleet::summary::iso_day(crate::fleet::analytics::filters::today(now));
    // From can't go past To and To can't go before From: the picker's limit only, typing is not limited (row 3.6).
    let (min, max) = match c.end {
        End::From => ("", f.to.as_str()),
        End::To => (f.from.as_str(), ""),
    };
    let spec = calendar::Spec { key: POPOVER, month: c.month, picked: c.end.of(f), today: &today, min, max };
    let panel = calendar::view(
        t,
        &spec,
        Rc::new(|s: &mut State| s.analytics_calendar_shift(-1)),
        Rc::new(|s: &mut State| s.analytics_calendar_shift(1)),
        Rc::new(|s: &mut State, day: &str| s.analytics_calendar_pick(day)),
    );
    let anchor = format!("analytics-filter-{}-calendar", c.end.key());
    calendar::popover(t, panel, &anchor, "analytics-calendar-scrim", Rc::new(|s: &mut State| s.analytics_close_calendar()))
}
