//! Settings → Skills & tools → Tools → Catalog (`tools/CatalogTab.vue`): the cards the catalog offers, with Install. The same
//! layout as Skills' catalog in `skills_catalog.rs`, with the type (Native or MCP) where Skills show the source.
use super::install_picker;
use super::skills_catalog::grid_of;
use super::skills_tools::{empty_state, error_box, loading};
use crate::State;
use crate::fleet::settings::install_target::Kind;
use crate::fleet::settings::tools::{CatalogEntry, is_installed};
use crate::icons::{DOWNLOAD, WRENCH};
use crate::theme::{RADIUS_CARD, Theme, sans};
use crate::ui::button::{Size, Variant, button, button_lead, disabled};
use crate::ui::confirm::spinner;
use crate::ui::controls::{BadgeVariant, badge};
use crate::ui::rem;
use rune::{El, col, icon, row, text};

/// The Install button, or Installed (outline, disabled) where the picker points, or Installing… while it runs.
fn action(s: &State, t: &Theme, entry: &CatalogEntry) -> El {
    let page = &s.settings.tools;
    if is_installed(&page.tools, &entry.name, &page.target) {
        return disabled(button(t, Variant::Outline, Size::Sm, None, "Installed").key(format!("tool-catalog-installed-{}", entry.name)));
    }
    let busy = page.installing_name.as_deref() == Some(entry.name.as_str());
    let lead = if busy { spinner(t.primary_fg) } else { icon(&DOWNLOAD, 16.0).color(t.primary_fg) };
    let b = button_lead(t, Variant::Default, Size::Sm, Some(lead), if busy { "Installing\u{2026}" } else { "Install" })
        .key(format!("tool-catalog-install-{}", entry.name));
    if busy {
        disabled(b)
    } else {
        let name = entry.name.clone();
        b.on_press(move |s: &mut State| s.install_catalog_tool(&name))
    }
}

/// `rounded-card border bg-main-bg p-4` with `gap-3` inside.
fn card(s: &State, t: &Theme, entry: &CatalogEntry) -> El {
    let mut title = col()
        .grow(1.0)
        .w(0.0)
        .min_w(0.0)
        .child(text(entry.title().to_string()).font(sans(rem(0.875), 600.0, 1.43)).color(t.text).truncate().shrink().min_w(0.0));
    if let Some(author) = entry.author.as_ref().filter(|a| !a.is_empty()) {
        title = title.child(row().mt(1.75).child(text(format!("by {author}")).font(sans(rem(0.75), 400.0, 1.33)).color(t.muted)));
    }
    let mut c = col()
        .key(format!("tool-catalog-card-{}", entry.name))
        .w_full()
        .gap(10.5)
        .padding(14.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.main_bg)
        .child(row().gap(7.0).items_start().child(icon(&WRENCH, 16.0).color(t.muted).mt(1.75)).child(title));
    if let Some(about) = entry.description.as_ref().filter(|d| !d.is_empty()) {
        c = c.child(text(about.clone()).font(sans(rem(0.875), 400.0, 1.43)).color(t.muted).max_lines(3).w_full());
    }
    if !entry.tags.is_empty() {
        c = c.child(row().gap(3.5).wrap().children(entry.tags.iter().map(|tag| badge(t, BadgeVariant::Outline, tag))));
    }
    let mut left = row().gap(7.0).center_y().child(badge(t, BadgeVariant::Secondary, entry.tool_type.label()));
    if let Some(v) = entry.version.as_ref().filter(|v| !v.is_empty()) {
        left = left.child(text(format!("v{v}")).font(sans(rem(0.75), 400.0, 1.33)).color(t.muted));
    }
    c.child(row().w_full().justify_between().center_y().gap(7.0).pt(7.0).child(left).child(action(s, t, entry)))
}

/// The Catalog tab: the picker, then loading, the error alone, the empty state or the grid; the install's error (else the
/// load's, when there is a catalog) under it.
pub fn catalog_tab(s: &State, t: &Theme) -> El {
    let page = &s.settings.tools;
    let has = !page.catalog.is_empty();
    let mut body = col().key("tools-catalog").w_full().gap(14.0).child(install_picker::picker(s, t, "tools-target", Kind::Tools));
    if page.catalog_loading {
        body = body.child(loading(t, "tools-catalog-loading", "Loading catalog\u{2026}"));
    } else if let (Some(e), false) = (&page.catalog_error, has) {
        body = body.child(error_box(t, "tools-catalog-error", e));
    } else if !has {
        body =
            body.child(empty_state(t, "tools-catalog-empty", &WRENCH, "No tools in catalog", "The tool catalog is empty or unavailable."));
    } else {
        body = body.child(grid_of("tools-catalog-grid", page.catalog.iter().map(|entry| card(s, t, entry)).collect()));
    }
    if let Some(e) = &page.install_error {
        body = body.child(error_box(t, "tools-install-error", e));
    } else if let (Some(e), true, false) = (&page.catalog_error, has, page.catalog_loading) {
        body = body.child(error_box(t, "tools-catalog-error", e));
    }
    body
}
