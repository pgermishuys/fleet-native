//! Settings → Skills & tools → Tools → Installed (`tools/InstalledToolsTab.vue`): a card per install with its badges, what it
//! runs or where it came from, the file Fleet wrote, and Remove (which asks first).
use super::skills_installed::{small_mono, unreachable_note};
use super::skills_tools::{empty_state, error_box, loading};
use crate::State;
use crate::fleet::settings::tools::{InstalledTool, ToolType, ToolsPage};
use crate::icons::{TRASH_2, WRENCH};
use crate::theme::{RADIUS_CARD, Theme, sans};
use crate::ui::button::{Size, Variant, button, button_ink, disabled};
use crate::ui::confirm::spinner;
use crate::ui::controls::{BadgeVariant, badge};
use crate::ui::rem;
use rune::{El, col, icon, row, text};

/// The title row: the name and its badges (type, target).
fn title(t: &Theme, tool: &InstalledTool, id: &str) -> El {
    let kind = match tool.tool_type {
        ToolType::Native => BadgeVariant::Secondary,
        ToolType::Mcp => BadgeVariant::Default,
    };
    row()
        .gap(7.0)
        .center_y()
        .child(icon(&WRENCH, 16.0).color(t.muted))
        .child(text(tool.title().to_string()).font(sans(rem(0.875), 600.0, 1.43)).color(t.text).truncate().shrink().min_w(0.0))
        .child(badge(t, kind, tool.tool_type.label()).key(format!("tool-type-{id}")))
        // The tooltip of Fleet's badge (the repository's path, or "Every session") is its label here: Rune has no tooltips.
        .child(badge(t, BadgeVariant::Outline, &tool.target.label()).key(format!("tool-target-{id}")).label(tool.target.tooltip()))
}

fn buttons(s: &State, t: &Theme, tool: &InstalledTool, id: &str) -> El {
    let page = &s.settings.tools;
    let key = tool.key();
    let mine = |slot: &Option<String>| slot.as_deref() == Some(key.as_str());
    let (confirming, removing) = (mine(&page.confirm_remove), mine(&page.removing));
    let idle = if confirming { "Confirm remove" } else { "Remove" };
    // `variant="outline"` with `text-error-text`: unlike a skill's, a tool's Remove is not a solid red button.
    let ink = t.error_text;
    let lead = if removing { spinner(ink) } else { icon(&TRASH_2, 16.0).color(ink) };
    let b = button_ink(t, Variant::Outline, Size::Sm, Some(lead), if removing { "Removing\u{2026}" } else { idle }, Some(ink))
        .key(format!("tool-remove-{id}"));
    let mut r = row().gap(7.0).wrap();
    r = r.child(if removing {
        disabled(b)
    } else {
        let key = key.clone();
        b.on_press(move |s: &mut State| s.press_remove_tool(&key))
    });
    if confirming && !removing {
        r = r.child(
            button(t, Variant::Outline, Size::Sm, None, "Cancel")
                .key(format!("tool-cancel-{id}"))
                .on_press(|s: &mut State| s.cancel_remove_tool()),
        );
    }
    r
}

/// One install: `rounded-card border bg-main-bg p-4`, the words on the left and the buttons on the right.
fn card(s: &State, t: &Theme, tool: &InstalledTool) -> El {
    let id = tool.key();
    let mut words = col().w(0.0).grow(1.0).min_w(0.0).child(title(t, tool, &id));
    if let Some(description) = tool.description.as_ref().filter(|d| !d.is_empty()) {
        words = words.child(
            row().mt(7.0).child(text(description.clone()).font(sans(rem(0.875), 400.0, 1.43)).color(t.muted).wrap().shrink().min_w(0.0)),
        );
    }
    if let Some(line) = tool.source_line() {
        words = words.child(row().key(format!("tool-source-{id}")).mt(10.5).child(small_mono(t, &line)));
    }
    match tool.installed_path.as_deref().filter(|p| !p.is_empty()) {
        Some(path) => words = words.child(row().key(format!("tool-path-{id}")).mt(3.5).child(small_mono(t, &format!("\u{2192} {path}")))),
        None => {
            words = words.child(unreachable_note(
                t,
                format!("tool-no-file-{id}"),
                "Not anywhere OpenCode looks. Remove it and install it again.",
            ))
        }
    }
    row()
        .key(format!("tool-{id}"))
        .w_full()
        .gap(14.0)
        .items_start()
        .justify_between()
        .padding(14.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.main_bg)
        .child(words)
        .child(buttons(s, t, tool, &id))
}

/// The tab: a loading line, the error alone when there is nothing to list, the empty state, or the cards with the error under.
pub fn tab(s: &State, t: &Theme) -> El {
    let page: &ToolsPage = &s.settings.tools;
    let has = !page.tools.is_empty();
    let mut body = col().key("tools-installed").w_full().gap(14.0);
    if page.loading {
        body = body.child(loading(t, "tools-loading", "Loading tools\u{2026}"));
    } else if let (Some(e), false) = (&page.error, has) {
        body = body.child(error_box(t, "tools-error", e));
    } else if !has {
        body = body.child(empty_state(t, "tools-empty", &WRENCH, "No tools installed", "Install a tool from the Catalog or Custom tab."));
    } else {
        let mut list = col().key("tools-list").w_full().gap(10.5);
        for tool in &page.tools {
            list = list.child(card(s, t, tool));
        }
        body = body.child(list);
    }
    if let (Some(e), true) = (&page.error, has) {
        body = body.child(error_box(t, "tools-error", e));
    }
    body
}
