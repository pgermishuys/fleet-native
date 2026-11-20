//! Ports `NSD/StarterChips.vue`: the quiet line of pills under "What should we work on?" (rows B9, A15). A pill fills the box
//! and sends nothing; the "…" button opens a menu with Hide starters.
use crate::State;
use crate::fleet::new_session::starters::BUILT_IN;
use crate::icons::*;
use crate::theme::{Theme, sans};
use crate::ui::Open;
use crate::ui::chip::{Option_, option, pop, pop_note};
use rune::{Align, Cursor, El, Icon, Side, icon, row, text};

/// The menu's width. B9.12 says 260 (`.ns-starters__pop`), but Fleet's own client draws it 344 wide, the plain `.ns-pop`: the
/// scoped rule doesn't reach the portaled menu. The picture decides (see the parity checklist).
const MENU_W: f32 = 344.0;
const MENU: &str = "new-session-starters-menu";

/// `.ns-starter`: 26 high, `0 10px`, a pill with a 1px border, 12.5px `--muted`; hover and open wash `--text` 6% and
/// `--text`; its icon is 12px at 70%.
fn pill(t: &Theme, key: &str, glyph: &'static Icon, label: Option<&str>, open: bool) -> El {
    let menu = label.is_none();
    let mut b = row()
        .key(key.to_string())
        .role("button")
        .h(26.0)
        .px(if menu { 8.0 } else { 10.0 })
        .gap(5.0)
        .center_y()
        .radius(999.0)
        .border(1.0, if menu { rune::Color::TRANSPARENT } else { t.border })
        .cursor(Cursor::Pointer)
        .hover(t.wash(6.0))
        .hover_group()
        .focus_ring(2.0, -1.0, t.accent);
    if open {
        b = b.fill(t.wash(6.0));
    }
    b = b.child(icon(glyph, 12.0).color(t.muted).group_color(t.text).opacity(0.7));
    match label {
        Some(l) => {
            b.label(l.to_string()).child(text(l.to_string()).font(sans(12.5, 400.0, 1.2)).color(if open { t.text } else { t.muted }))
        }
        None => b.label("Starter options"),
    }
}

/// `.ns-starters`: gap 6, 16 above. Centred while it fits, from the start once it scrolls.
pub fn line(s: &State, t: &Theme) -> El {
    let menu_open = s.ui.open == Some(Open::Popover("starters"));
    let glyph = |id: &str| match id {
        "explore" => &COMPASS,
        "plan" => &LIST_CHECKS,
        _ => &SEARCH,
    };
    let mut r = row().key("new-session-starters").label("Starters").role("group").max_w(640.0).gap(6.0).mt(16.0).justify_center();
    for starter in &BUILT_IN {
        let id = starter.id;
        r = r.child(
            pill(t, &format!("new-session-starter-{id}"), glyph(id), Some(starter.label), false)
                .help("Fills the box. Sends nothing.")
                .on_press(move |s: &mut State| s.fill_starter(id)),
        );
    }
    if s.show_workflow_starter() {
        r = r.child(
            pill(t, "new-session-starter-fix-a-bug", &WORKFLOW, Some("Fix a bug"), false)
                .help("Opens the workflow's Run box. Starts nothing.")
                .child(
                    row()
                        .key("new-session-starter-tag")
                        .ml(2.0)
                        .px(4.0)
                        .radius(4.0)
                        .border(1.0, t.border)
                        .child(text("workflow").font(sans(10.5, 400.0, 1.4)).color(t.muted)),
                )
                .on_press(|s: &mut State| s.open_fix_a_bug()),
        );
    }
    r.child(pill(t, MENU, &ELLIPSIS, None, menu_open).on_press(|s: &mut State| s.ui.open = Some(Open::Popover("starters"))))
}

/// `ns-pop ns-starters__pop`: 260 wide, 6px under the button, its right edge on the button's.
pub fn menu(t: &Theme) -> Vec<El> {
    let hide = option(t, Option_ { icon: Some(&EYE_OFF), ..Option_::new("new-session-starters-hide", "Hide starters") }, None, |s, _| {
        s.ui.open = None;
        s.hide_starters();
    });
    let panel = pop(t, "new-session-starters-pop", MENU_W, vec![hide, pop_note(t, "Settings → General → Show starters brings them back.")])
        .focus_return(MENU.to_string())
        .modal(true, |s: &mut State| s.ui.open = None);
    vec![
        crate::ui::overlay::scrim("new-session-starters-scrim", |s: &mut State| s.ui.open = None),
        crate::ui::overlay::anchored(panel, MENU, Side::Bottom, Align::End, 6.0),
    ]
}
