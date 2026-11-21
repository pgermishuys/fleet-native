//! The Projects section (`components/analytics/tabs/ProjectsTab.vue`): a grid of cards, one per project, by cost; before the
//! first answer the wait, then the failure, then the empty words. Its projects are the page's list (`scopedTopProjects`).
use super::cost_bars::loading_row;
use crate::State;
use crate::fleet::analytics::tables::{ProjectCard, project_cards};
use crate::fleet::analytics::{projects_empty, with_selected_project};
use crate::theme::{RADIUS_CARD, Theme, sans};
use crate::ui::empty_state;
use crate::ui::error_note::error_note;
use rune::{El, col, el, row, text};

pub fn projects(s: &State, t: &Theme) -> El {
    let a = &s.analytics;
    let selected = a.filters.project_id.trim();
    let list = with_selected_project(a.summary.data.as_ref().map(|x| x.top_projects.as_slice()).unwrap_or_default(), selected);
    let cards = project_cards(&list);
    let body = if !cards.is_empty() {
        // `repeat(auto-fit, minmax(240px, 1fr))`, gap 12: the cards wrap and share the row.
        row().key("analytics-projects-grid").w_full().flex_wrap().gap(12.0).children(cards.iter().map(|c| card(t, c)))
    } else if a.projects_loading() {
        loading_row(t, "analytics-loading", "Loading project analytics…")
    } else if let Some(reason) = &a.summary.error {
        error_note(t, "analytics-error", "Unable to load analytics.", reason)
    } else {
        empty_state::bordered(t, "analytics-empty", projects_empty(!selected.is_empty()), 157.5)
    };
    col().key("analytics-projects").label("Projects analytics").w_full().min_w(0.0).child(body)
}

fn stat(t: &Theme, label: &str, value: &str) -> El {
    col()
        .grow(1.0)
        .w(0.0)
        .gap(6.0)
        .child(text(label.to_string()).font(sans(11.0, 400.0, 1.4)).color(t.muted))
        .child(text(value.to_string()).font(sans(18.0, 600.0, 1.4).features("'tnum' 1")).color(t.text))
}

fn card(t: &Theme, p: &ProjectCard) -> El {
    let key = format!("analytics-project-{}", p.name);
    let head = row()
        .w_full()
        .items_start()
        .justify_between()
        .gap(16.0)
        .child(text(p.name.clone()).font(sans(14.0, 600.0, 1.35)).color(t.text).wrap().grow(1.0).min_w(0.0))
        .child(text(p.cost.clone()).font(sans(14.0, 600.0, 1.4).features("'tnum' 1")).color(t.text));
    let track = row()
        .key(format!("{key}-track"))
        .role("img")
        .label(format!("{} relative cost {} percent", p.name, p.percent))
        .w_full()
        .h(6.0)
        .radius(999.0)
        .clip_rounded(999.0)
        .fill(t.text.alpha(0.08))
        .child(el().grow(p.width as f32).w(0.0).h_full().radius(999.0).fill(t.accent))
        .child(el().grow((100.0 - p.width) as f32).w(0.0));
    let bar = col()
        .w_full()
        .gap(8.0)
        .child(
            row()
                .w_full()
                .center_y()
                .justify_between()
                .gap(12.0)
                .child(text("Relative spend").font(sans(11.0, 500.0, 1.4)).color(t.muted))
                .child(
                    text(format!("{}%", p.percent))
                        .key(format!("{key}-percent"))
                        .font(sans(11.0, 500.0, 1.4).features("'tnum' 1"))
                        .color(t.muted),
                ),
        )
        .child(track);
    col()
        .key(key)
        .min_w(240.0)
        .grow(1.0)
        .padding(16.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.card_bg)
        .child(head)
        .child(row().w_full().mt(16.0).gap(12.0).child(stat(t, "Tokens", &p.tokens)).child(stat(t, "Cost", &p.cost)))
        // `margin-top: auto`: the bar sits at the bottom of a card stretched to its row, 16 under the stats at least.
        .child(el().grow(1.0).min_h(16.0))
        .child(bar)
}
