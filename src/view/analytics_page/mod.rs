//! The Analytics page (component 20): `[analytics-menu][analytics-detail]`. Ports `components/analytics/AnalyticsContent.vue`
//! (the page: its `PageHeader` and the section's body) and the context's layout (`AppShell.vue`). The menu is `menu`; each
//! section's body is an empty slot until its own piece (F to J).
use crate::State;
use crate::fleet::analytics::sections::Section;
use crate::theme::Theme;
use crate::ui::page_header;
use crate::view::workflows::sheet;
use rune::{ScrollSpec, col};

pub mod menu;

pub const TITLE: &str = "Analytics";
pub const LINE: &str = "Tokens and spend across your sessions, by project and model. Costs are estimates at list prices, not a bill.";

/// The whole context: the menu, its gutter and the page; with the panel collapsed (Ctrl+B), the page alone.
pub fn page(s: &State, t: &Theme) -> Vec<rune::El> {
    if s.context_panel_shown() { vec![menu::panel(s, t), super::gutter(t), detail(s, t)] } else { vec![detail(s, t)] }
}

/// The page in the centre sheet: 24 px of padding all round (`.center__body`), the parts stacked 20 apart (`.analytics-page`),
/// scrolling as one. The body is keyed by its section so a switch starts it fresh (`:key="activeTab"`).
fn detail(s: &State, t: &Theme) -> rune::El {
    let section = s.analytics_nav.section;
    let page = col()
        .key("analytics-page")
        .label("Analytics dashboard")
        .w_full()
        .min_w(0.0)
        .gap(20.0)
        .padding(24.0)
        .child(page_header::plain(t, "analytics-header", TITLE, Some(LINE), Vec::new()))
        .child(body(s, t, section));
    sheet(t, "analytics-detail").child(page.scroll(ScrollSpec { stick_bottom: false }).w_full().grow(1.0).min_h(0.0))
}

/// A section's body. Nothing is drawn yet: the pieces after this one fill each in.
fn body(_s: &State, _t: &Theme, section: Section) -> rune::El {
    col().key(format!("analytics-body-{}", section.id())).w_full()
}
