//! The rail (`IconRail.vue`, `RailButton.vue`): 48px wide, 40px icon buttons, the logo on top, then the buttons
//! `fleet::rails` lists (a Labs rail only while its switch is on) and Help at the bottom.
use crate::fleet::rails::{self, RailDef};
use crate::icons::*;
use crate::theme::Theme;
use crate::theme::sans;
use crate::{Rail, State};
use rune::{Align, Color, Cursor, El, Icon, Side, Tx, col, el, icon, spacer, text};

pub fn rail(s: &State, t: &Theme) -> El {
    // The logo file is 511×234, drawn `object-fit: contain` in a 32px box.
    let mut logo = el().size(32.0, 32.0).mb(4.0).center();
    if let Some(image) = &s.logo {
        logo = logo.child(el().size(32.0, 32.0 * 234.0 / 511.0).pixels(image.pixels()));
    }
    let list = s.rail_list();
    let lit = s.active_rail();
    let mut r = col().w(48.0).h_full().py(8.0).items_center().child(logo);
    for def in rails::top(&list) {
        r = r.child(item(t, &def, lit));
    }
    r = r.child(divider(t));
    for def in rails::secondary(&list) {
        r = r.child(item(t, &def, lit));
    }
    r = r.child(spacer()).child(divider(t));
    for def in rails::bottom(&list) {
        r = r.child(item(t, &def, lit));
    }
    r.child(plain(t, None, &CIRCLE_QUESTION_MARK, "Help", false, false))
}

fn divider(t: &Theme) -> El {
    el().size(24.0, 1.0).my(4.0).fill(t.border)
}

fn glyph_of(rail: Rail) -> &'static Icon {
    match rail {
        Rail::Fleet => &RADAR,
        Rail::Sessions => &MESSAGE_SQUARE,
        Rail::Board => &LAYOUT_GRID,
        Rail::Workflows => &WORKFLOW,
        Rail::Jobs => &LIST_TODO,
        Rail::Automations => &ZAP,
        Rail::Analytics => &CHART_COLUMN,
        Rail::GitHub => &GITHUB,
        Rail::Settings => &SETTINGS,
    }
}

fn item(t: &Theme, def: &RailDef, lit: Option<Rail>) -> El {
    plain(t, Some(def.rail), glyph_of(def.rail), &rails::tooltip(def), lit == Some(def.rail), def.off)
}

/// One rail button. `which` is the rail it opens; Help (no rail) opens its menu. A rail that is off has
/// its glyph dimmed (not the button, so hover and active keep their fills) and a muted dot.
fn plain(t: &Theme, which: Option<Rail>, glyph: &'static Icon, label: &str, active: bool, off: bool) -> El {
    let mut face = el()
        .size(18.0, 18.0)
        .child(icon(glyph, 18.0).color(if active { t.text } else { t.muted }).group_color(t.text).opacity(if off { 0.55 } else { 1.0 }));
    if off {
        face = face.child(el().key("rail-off-dot").absolute(17.0, -3.0).size(6.0, 6.0).radius(3.0).fill(t.muted));
    }
    let name = label.split(" \u{b7} ").next().unwrap_or(label);
    let mut button = el()
        .key(format!("rail-{}", name.split(' ').next().unwrap_or(name).to_lowercase()))
        .size(40.0, 40.0)
        .mb(2.0)
        .center()
        .radius(8.0)
        .fill(if active { t.wash(9.0) } else { Color::TRANSPARENT })
        .hover(if active { Color::TRANSPARENT } else { t.wash(5.0) })
        .hover_group()
        .cursor(Cursor::Pointer)
        .label(name);
    if let Some(rail) = which {
        // Fleet's tooltip is its own CSS one, not the system's: drawn by `tip` from here.
        button = button
            .on_hover_change(move |s: &mut State, inside: bool, _: &mut Tx| {
                if inside {
                    s.rail_tip = Some(rail);
                } else if s.rail_tip == Some(rail) {
                    s.rail_tip = None;
                }
            })
            .on_press(move |s: &mut State| s.press_rail(rail));
    } else {
        // Help opens a menu (`ui::menus::help`), not a page.
        button = button.help(label).on_press(|s: &mut State| s.ui.open = Some(crate::ui::Open::Menu("help")));
    }
    button.child(face)
}

/// `.rail-item[data-tooltip]::after`: left of 52 and 4 more once shown, so 16px clear of the button, centred down it,
/// `--color-popover` on `--color-popover-foreground`, 11px, padding 4 10, square.
pub fn tip(s: &State, t: &Theme) -> Option<El> {
    let which = s.rail_tip?;
    let def = s.rail_list().into_iter().find(|d| d.rail == which && !d.hidden)?;
    let key = format!("rail-{}", def.label.to_lowercase());
    let bubble = el()
        .key("rail-tip")
        .role("tooltip")
        .px(10.0)
        .py(4.0)
        .fill(t.card_bg)
        .child(text(rails::tooltip(&def)).font(sans(11.0, 400.0, 1.3)).color(t.text));
    Some(crate::ui::overlay::anchored(bubble, &key, Side::Right, Align::Center, 16.0))
}
