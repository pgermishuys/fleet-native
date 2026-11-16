//! `SessionAnalyticsPopover.vue`: the header's chart button and the card under it with two numbers, the tokens and the
//! cost of the open session. The strings ("—" for a number the server doesn't have) are `fleet::header::usage_strings`.
use crate::State;
use crate::theme::{Theme, sans};
use crate::ui::overlay::{anchored, scrim};
use rune::{Align, El, Side, col, row, text};

/// The button's key, which the card is placed under.
pub const TRIGGER: &str = "analytics";
/// Measured on Fleet's own popover: `width: 240px` plus the content's 14px padding and the 1px border leave 222px for the
/// grid, so the box is 252px (Fleet's `padding: 0` loses to the popover's own `p-4`) and has no radius.
const WIDTH: f32 = 252.0;

/// One cell: a label, a value, a helper (`.analytics-popover__metric`).
fn cell(t: &Theme, key: &str, label: &str, value: &str, helper: &str) -> El {
    col()
        .w(1.0)
        .grow(1.0)
        .min_w(0.0)
        .px(10.5)
        .py(9.1)
        .fill(t.card_bg)
        .child(text(label.to_uppercase()).font(sans(9.0, 650.0, 1.2).tracking(0.405)).color(t.muted).truncate())
        .child(text(value.to_string()).key(format!("{key}-value")).font(sans(14.0, 700.0, 1.2)).color(t.text).mt(2.8).truncate())
        .child(text(helper.to_string()).font(sans(9.0, 400.0, 1.25)).color(t.muted).mt(2.1).truncate())
}

/// The scrim and the card under the button (`side="bottom" align="end"`).
pub fn overlay(s: &State, t: &Theme) -> Vec<El> {
    let m = s.header_model();
    let head = col()
        .w_full()
        .px(10.5)
        .pt(9.1)
        .pb(7.7)
        .border_sides([0.0, 0.0, 1.0, 0.0], t.border)
        .child(text("SESSION ANALYTICS").font(sans(9.0, 700.0, 1.2).tracking(0.63)).color(t.muted))
        .child(text("Usage summary").font(sans(12.04, 650.0, 1.25)).color(t.text).mt(2.1));
    // The 1px `--border` showing between the cells is the grid's gap.
    let cells = row()
        .w_full()
        .gap(1.0)
        .fill(t.border)
        .child(cell(t, "analytics-tokens", "Total tokens", &m.tokens_text, "Across this session"))
        .child(cell(t, "analytics-cost", "Total cost", &m.cost_text, "Estimated spend"));
    let card = col()
        .key("analytics-popover")
        .role("dialog")
        .label("Session analytics")
        .w(WIDTH)
        .items_stretch()
        .fill(t.card_bg)
        .border(1.0, t.border)
        .border_layout()
        .padding(14.0)
        .child(head)
        .child(cells);
    let card = t
        .menu_shadows()
        .into_iter()
        .fold(card, |e, shadow| e.box_shadow(shadow))
        .focus_return(TRIGGER.to_string())
        .modal(true, |s: &mut State| s.ui.open = None);
    vec![scrim("analytics-scrim", |s: &mut State| s.ui.open = None), anchored(card, TRIGGER, Side::Bottom, Align::End, 4.0)]
}
