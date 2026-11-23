//! The Board's left panel, `BoardControlsPanel.vue` (component 23, piece Q): Project, Status, Agents, Group by, Sort and the
//! Summary with its four quick stats. No header, 12 px of padding at the sides and under, scrolling. Nothing here changes the
//! lanes (FB3): only the counts and the Summary tab follow it.
use crate::State;
use crate::fleet::board::cards::{SortBy, Status};
use crate::fleet::board::filters::{ALL_PROJECTS, GroupBy};
use crate::theme::{RADIUS_BTN, Theme, sans};
use crate::ui::button::{Size, Variant, button};
use crate::ui::controls::checkbox;
use crate::ui::{Open, select};
use rune::{Cursor, El, ScrollSpec, col, el, icon, row, text};
use std::rc::Rc;

/// The two selects' names for `Open::Select` and their lists.
pub const PROJECT: &str = "board-project";
pub const SORT: &str = "board-sort";

/// `.bc-label`: 10 px, 600, upper case, tracked, muted, 6 px under.
fn label(t: &Theme, words: &str) -> El {
    row().mb(6.0).child(text(words.to_uppercase()).font(sans(10.0, 600.0, 1.5).tracking(0.5)).color(t.muted))
}

/// `.bc-section`: 16 px under it.
fn section(t: &Theme, name: &str, words: &str) -> El {
    col().key(format!("board-controls-{name}")).w_full().mb(16.0).child(label(t, words))
}

/// `.bc-select`: full width, `--card-bg`, `--border`, `--radius-btn`, 7 10 of padding, `--text`.
fn picker(t: &Theme, key: &'static str, aria: &'static str, shown: &str) -> El {
    row()
        .key(key)
        .role("combobox")
        .label(aria)
        .w_full()
        // 7 px above and under the browser's own 21 px line of 14 px text.
        .h(35.0)
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
        .child(text(shown.to_string()).font(sans(14.0, 400.0, 1.3)).color(t.text).truncate().shrink())
        .child(icon(&crate::icons::CHEVRON_DOWN, 14.0).color(t.muted).opacity(0.5))
        .on_press(move |s: &mut State| s.ui.open = Some(Open::Select(key)))
}

/// `.bc-check`: a labelled checkbox, 11 px `--text`, 8 apart, 3 px above and under; `lead` is the status's dot.
fn check(t: &Theme, key: String, words: &str, on: bool, lead: Option<El>, flip: impl Fn(&mut State) + 'static) -> El {
    let mut r = row()
        .key(key)
        .role("checkbox")
        .label(words.to_string())
        .checked(on)
        .w_full()
        // 3 px above and under a 16.5 px line, as the browser's checkbox margins make it.
        .min_h(22.5)
        .gap(8.0)
        .items_center()
        .cursor(Cursor::Pointer)
        .focus_ring(2.0, 2.0, t.accent)
        .child(checkbox(t, on));
    if let Some(dot) = lead {
        r = r.child(dot);
    }
    r.child(text(words.to_string()).font(sans(11.0, 400.0, 1.4)).color(t.text).single_line()).on_press(move |s: &mut State| flip(s))
}

fn project(s: &State, t: &Theme) -> El {
    let f = &s.board.filters;
    let shown = if f.project == ALL_PROJECTS { "All projects" } else { f.project.as_str() };
    section(t, "project", "Project").child(picker(t, PROJECT, "Filter by project", shown))
}

fn status(s: &State, t: &Theme) -> El {
    let rows = Status::ALL.iter().map(|&st| {
        let dot = el().size(8.0, 8.0).radius(999.0).fill(st.color(t));
        check(t, format!("board-status-{}", st.value()), st.label(), s.board.filters.status_on(st), Some(dot), move |s| {
            let on = s.board.filters.status_on(st);
            s.board.filters.set_status(st, !on);
        })
    });
    section(t, "status", "Status").children(rows)
}

fn agents(s: &State, t: &Theme) -> El {
    let rows = s.board.available_agents().into_iter().map(|a| {
        let name = a.clone();
        check(t, format!("board-agent-{a}"), &a, s.board.filters.agent_on(&a), None, move |s| {
            let on = s.board.filters.agent_on(&name);
            s.board.filters.set_agent(&name, !on);
        })
    });
    section(t, "agents", "Agents").children(rows)
}

/// `.radio-pills`: the filter-variant small buttons, 4 apart.
fn group_by(s: &State, t: &Theme) -> El {
    let pills = GroupBy::ALL.iter().map(|&g| {
        let on = s.board.filters.group_by == g;
        button(t, Variant::Filter { active: on }, Size::Sm, None, g.label())
            .key(format!("board-group-{}", g.label().to_lowercase()))
            .role("button")
            .checked(on)
            .on_press(move |s: &mut State| s.board.filters.group_by = g)
    });
    section(t, "group", "Group by")
        .child(row().key("board-group-by").role("radiogroup").label("Group board cards by").gap(4.0).children(pills))
}

fn sort(s: &State, t: &Theme) -> El {
    section(t, "sort", "Sort").child(picker(t, SORT, "Sort board cards", s.board.filters.sort_by.label()))
}

/// One of the quick stats: the value 15 px 600 over its label 10 px, muted.
fn stat(t: &Theme, key: &str, value: usize, words: &str) -> El {
    col()
        .key(format!("board-stat-{key}"))
        .gap(2.0)
        .min_w(0.0)
        .child(text(value.to_string()).font(sans(15.0, 600.0, 1.4)).color(t.text))
        .child(text(words.to_string()).font(sans(10.0, 400.0, 1.4)).color(t.muted))
}

fn summary(s: &State, t: &Theme) -> El {
    let q = s.board.quick_stats();
    let stats = row()
        .key("board-quick-stats")
        .label("Board quick stats")
        .w_full()
        .gap(12.0)
        .pt(10.0)
        .mt(8.0)
        .border_sides([1.0, 0.0, 0.0, 0.0], t.border)
        .border_layout()
        .child(stat(t, "visible", q.visible, "Visible"))
        .child(stat(t, "active", q.active, "Active"))
        .child(stat(t, "done", q.completed, "Done"))
        .child(stat(t, "projects", q.projects, "Projects"));
    section(t, "summary", "Summary")
        .child(text(s.board.filter_summary()).key("board-summary").font(sans(11.0, 400.0, 1.4)).color(t.muted))
        .child(stats)
}

/// The panel: `[board-controls]`, as wide as the sessions list.
pub fn panel(s: &State, t: &Theme) -> El {
    let body = col()
        .w_full()
        .px(12.0)
        .pb(12.0)
        .child(project(s, t))
        .child(status(s, t))
        .child(agents(s, t))
        .child(group_by(s, t))
        .child(sort(s, t))
        .child(summary(s, t));
    col()
        .key("board-controls")
        .label("Board controls panel")
        .w(s.sidebar_w)
        .h_full()
        .min_h(0.0)
        .child(col().w_full().grow(1.0).shrink().min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(body))
}

/// The open project list: "All projects", then the board's name.
pub fn project_list(s: &State, t: &Theme) -> Vec<El> {
    let projects = s.board.available_projects();
    let mut options: Vec<(&str, &str)> = vec![(ALL_PROJECTS, "All projects")];
    options.extend(projects.iter().map(|p| (p.as_str(), p.as_str())));
    let choose: select::Choose = Rc::new(|s: &mut State, v: &str| s.board.filters.project = v.to_string());
    select::content(t, "board-project-list", PROJECT, s.sidebar_w - 24.0, &options, Some(s.board.filters.project.as_str()), choose)
}

/// The open sort list.
pub fn sort_list(s: &State, t: &Theme) -> Vec<El> {
    let options: Vec<(&str, &str)> = SortBy::ALL.iter().map(|o| (o.value(), o.label())).collect();
    let choose: select::Choose = Rc::new(|s: &mut State, v: &str| {
        if let Some(o) = SortBy::ALL.iter().find(|o| o.value() == v) {
            s.board.filters.sort_by = *o;
        }
    });
    select::content(t, "board-sort-list", SORT, s.sidebar_w - 24.0, &options, Some(s.board.filters.sort_by.value()), choose)
}
