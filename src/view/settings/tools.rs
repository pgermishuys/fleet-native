//! Settings → Skills & tools → Tools (`ToolsSection.vue`): the intro line, the strip (Installed, Catalog, Custom) and the
//! picked tab. Installed is `tools_installed.rs`, Catalog is `tools_catalog.rs`, Custom is `tools_custom.rs`. The same shape as Skills' section in `skills_tools.rs`.
use super::{tools_catalog, tools_custom, tools_installed};
use crate::State;
use crate::fleet::settings::skills::Inner;
use crate::theme::{Theme, sans};
use crate::ui::controls::{TabsVariant, tab, tabs};
use crate::ui::rem;
use rune::{El, col, row, text};

/// `grid w-full grid-cols-3`: three equal columns, each tab's words in the middle of its column.
fn inner_strip(s: &State, t: &Theme) -> El {
    let page = &s.settings.tools;
    let items = Inner::ALL.iter().map(|&which| {
        tab(t, TabsVariant::Underline, &format!("tools-tab-{}", which.id()), which.label(), page.inner == which)
            .label(which.label().to_string())
            .on_press(move |s: &mut State| s.pick_tools_inner(which))
            .items_center()
            .grow(1.0)
            .w(0.0)
    });
    row().key("tools-inner-tabs").w_full().child(tabs(t, TabsVariant::Underline, items.collect()).w_full().gap(0.0))
}

/// `ToolsSection`: the intro line, the strip (`mt-5`), the picked tab (`mt-4`).
pub fn section(s: &State, t: &Theme) -> El {
    let intro = row().px(3.5).child(
        text("Manage installed tools, browse the catalog, or install from a custom source.")
            .font(sans(rem(0.75), 400.0, 1.33))
            .color(t.muted),
    );
    let body = match s.settings.tools.inner {
        Inner::Installed => tools_installed::tab(s, t),
        Inner::Catalog => tools_catalog::catalog_tab(s, t),
        Inner::Custom => tools_custom::custom_tab(s, t),
    };
    col()
        .key("tools-section")
        .w_full()
        .child(intro)
        .child(row().w_full().mt(17.5).child(inner_strip(s, t)))
        .child(col().w_full().mt(20.5).child(body))
}
