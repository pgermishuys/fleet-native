//! The analytics-menu, `[analytics-menu]` (`AnalyticsMachinePanel.vue`, `AnalyticsNavPanel.vue`): the header, the machine switch,
//! the two groups of sections with their counts. The filters and Export CSV below them are pieces D and E.
use crate::State;
use crate::fleet::analytics::sections::{GROUPS, SECTIONS, Section};
use crate::theme::{RADIUS_BTN, Theme, sans};
use crate::ui::rem;
use crate::view::machine_switch;
use rune::{Color, Cursor, El, ScrollSpec, col, el, row, text};

/// The switch under the header: "Analytics for machine".
pub const SWITCH: machine_switch::Switch =
    machine_switch::Switch { label: "Analytics", testid: "analytics", choice: |s| &mut s.analytics.choice };

/// The switch's open menu, over the window.
pub fn overlay(s: &State, t: &Theme) -> Vec<El> {
    machine_switch::overlay(s, t, SWITCH, &s.analytics.choice)
}

/// How many rows a section lists under the filters; the spend sections have none (`useAnalyticsMenu` `counts`).
pub fn count(s: &State, section: Section) -> Option<usize> {
    let a = &s.analytics;
    match section {
        Section::Projects => Some(a.range.data.as_ref().map_or(0, |r| r.top_projects.len())),
        Section::Models => Some(a.models.data.len()),
        Section::Sessions => Some(a.sessions.data.len()),
        Section::Overview | Section::Daily => None,
    }
}

/// `.analytics-nav__header` and `__group-label`: 10 px, 600, upper case, tracking 0.05 em, muted.
fn label(t: &Theme, words: &str) -> El {
    text(words.to_uppercase()).font(sans(rem(0.71429), 600.0, 1.5).tracking(0.5)).color(t.muted)
}

/// `.analytics-nav__item`: 36 high, the label left and the count right; the open one has the accent's border and fill.
fn item(s: &State, t: &Theme, section: Section) -> El {
    let on = s.analytics_nav.section == section;
    let ink = if on { t.text } else { t.muted };
    let mut it = row()
        .key(format!("analytics-section-{}", section.id()))
        .role("button")
        .label(section.label())
        .w_full()
        .min_h(36.0)
        .px(12.0)
        .gap(10.0)
        .center_y()
        .radius(RADIUS_BTN)
        .border(1.0, if on { t.accent.alpha(0.4) } else { Color::TRANSPARENT })
        .border_layout()
        .cursor(Cursor::Pointer)
        .focus_ring(2.0, 2.0, t.accent)
        .child(text(section.label()).font(sans(rem(0.85714), 500.0, 1.4)).color(ink));
    if let Some(n) = count(s, section) {
        it = it.child(el().grow(1.0).w(0.0)).child(
            text(n.to_string())
                .key(format!("analytics-count-{}", section.id()))
                .font(sans(rem(0.78571), 400.0, 1.4).features("'tnum' 1"))
                .color(t.muted),
        );
    }
    let it = if on { it.fill(t.mix(t.accent, t.panel_bg, 0.12)) } else { it.hover(t.main_bg) };
    it.on_press(move |s: &mut State| s.open_analytics_section(section))
}

pub fn panel(s: &State, t: &Theme) -> El {
    // The machine switch sits inside the panel's own 12 px of padding with 12 of its own.
    let machine = machine_switch::chip(s, t, SWITCH, &s.analytics.choice).map(|chip| row().px(12.0).pb(6.0).child(chip));
    let mut sections = col().key("analytics-sections").label("Analytics sections").w_full();
    for group in GROUPS {
        let items = SECTIONS.iter().copied().filter(|x| x.group() == group).map(|x| item(s, t, x));
        sections = sections.child(
            col()
                .key(format!("analytics-group-{}", group.label().to_lowercase()))
                .role("group")
                .label(group.label())
                .w_full()
                .gap(4.0)
                .mb(8.0)
                .child(row().mt(8.0).mb(2.0).px(12.0).child(label(t, group.label())))
                .children(items),
        );
    }
    let body = col()
        .w_full()
        .px(12.0)
        .pb(12.0)
        .child(row().px(4.0).pt(18.0).pb(10.0).child(label(t, "Analytics").key("analytics-menu-header")))
        .children(machine)
        .child(sections);
    col()
        .key("analytics-menu")
        .label("Analytics navigation")
        .w(s.sidebar_w)
        .h_full()
        .min_h(0.0)
        .child(col().w_full().grow(1.0).shrink().min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(body))
}
