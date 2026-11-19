//! Settings → Skills & tools (`SkillsToolsPage.vue`, `SkillsSection.vue`): the page's tab strip (Skills, Tools and, while the
//! Mods switch is on, Mods), and Skills' own: the intro line and Installed, Catalog and Custom. The Installed tab is
//! `skills_installed.rs`, Catalog and Custom are `skills_catalog.rs`; Tools is `tools.rs`; Mods is `mods_tab.rs`. Also here, for them to
//! share: the error note and the loading line every tab of the page uses (`ErrorNote`, `Spinner`).
use super::{skills_catalog, skills_installed};
use crate::State;
use crate::fleet::settings::skills::{Inner, Tab};
use crate::icons::CIRCLE_ALERT;
use crate::theme::{RADIUS_CARD, Theme, sans};
use crate::ui::controls::{TabsVariant, tab, tabs};
use crate::ui::{confirm::spinner, rem};
use rune::{El, col, icon, row, text};

/// `text-sm text-muted` with a spinner in front: "Loading skills…".
pub fn loading(t: &Theme, key: &str, words: &str) -> El {
    row()
        .key(key.to_string())
        .gap(7.0)
        .center_y()
        .child(spinner(t.muted))
        .child(text(words.to_string()).font(sans(rem(0.875), 400.0, 1.43)).color(t.muted))
}

/// `ErrorNote`: the red tint, the alert, the message (`rounded-card border border-error/30 bg-error/10 px-3 py-2 text-sm`).
pub fn error_box(t: &Theme, key: &str, message: &str) -> El {
    row()
        .key(key.to_string())
        .role("alert")
        .w_full()
        .gap(7.0)
        .items_start()
        .px(10.5)
        .py(7.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.error.alpha(0.3))
        .border_layout()
        .fill(t.error.alpha(0.1))
        .child(icon(&CIRCLE_ALERT, 16.0).color(t.error_text).mt(1.0))
        .child(text(message.to_string()).font(sans(rem(0.875), 400.0, 1.43)).color(t.error_text).wrap().shrink().min_w(0.0))
}

/// `EmptyState` with `bordered`: a dashed box with a glyph, a title and a line under it.
pub fn empty_state(t: &Theme, key: &str, glyph: &'static rune::Icon, title: &str, description: &str) -> El {
    col()
        .key(key.to_string())
        .w_full()
        .center()
        .gap(3.5)
        .padding(21.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .border_dash(3.0, 3.0)
        .child(icon(glyph, 28.0).color(t.muted).mb(7.0))
        .child(text(title.to_string()).font(sans(14.0, 600.0, 1.4)).color(t.text))
        .child(text(description.to_string()).font(sans(rem(0.875), 400.0, 1.43)).color(t.muted).wrap())
}

fn outer_strip(s: &State, t: &Theme) -> El {
    let page = &s.settings.skills;
    let mut items = vec![("skills", "Skills", Tab::Skills), ("tools", "Tools", Tab::Tools)];
    if s.skills_mods_on() {
        items.push(("mods", "Mods", Tab::Mods));
    }
    let strip = items.into_iter().map(|(id, label, which)| {
        tab(t, TabsVariant::Underline, &format!("skills-tools-tab-{id}"), label, page.tab == which)
            .label(label.to_string())
            .on_press(move |s: &mut State| s.pick_skills_tab(which))
    });
    // `TabsList class="mb-4"`.
    row().w_full().mb(21.0).child(tabs(t, TabsVariant::Underline, strip.collect()).key("skills-tools-tabs"))
}

/// `grid w-full grid-cols-3`: three equal columns, each tab's words in the middle of its column.
fn inner_strip(s: &State, t: &Theme) -> El {
    let page = &s.settings.skills;
    let items = Inner::ALL.iter().map(|&which| {
        tab(t, TabsVariant::Underline, &format!("skills-tab-{}", which.id()), which.label(), page.inner == which)
            .label(which.label().to_string())
            .on_press(move |s: &mut State| s.pick_skills_inner(which))
            .items_center()
            .grow(1.0)
            .w(0.0)
    });
    row().key("skills-inner-tabs").w_full().child(tabs(t, TabsVariant::Underline, items.collect()).w_full().gap(0.0))
}

/// `SkillsSection`: the intro line, the strip (`mt-5`), the picked tab (`mt-4`).
fn skills_section(s: &State, t: &Theme) -> El {
    let page = &s.settings.skills;
    let intro = row().px(3.5).child(
        text("Manage installed skills, browse the catalog, or install from a custom source.")
            .font(sans(rem(0.75), 400.0, 1.33))
            .color(t.muted),
    );
    let body = match page.inner {
        Inner::Installed => skills_installed::tab(s, t),
        Inner::Catalog => skills_catalog::catalog_tab(s, t),
        Inner::Custom => skills_catalog::custom_tab(s, t),
    };
    col()
        .key("skills-section")
        .w_full()
        .child(intro)
        .child(row().w_full().mt(17.5).child(inner_strip(s, t)))
        .child(col().w_full().mt(20.5).child(body))
}

/// The page body under the header.
pub fn content(s: &State, t: &Theme) -> El {
    let page = &s.settings.skills;
    let body = match page.tab {
        Tab::Skills => skills_section(s, t),
        Tab::Tools => super::tools::section(s, t),
        Tab::Mods => super::mods_tab::tab(s, t),
    };
    col().w_full().child(outer_strip(s, t)).child(body)
}
