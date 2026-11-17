//! Updates & about › Machines (`MachineUpdatesGroup.vue`): the release strip, then a row per machine with its version line and
//! Restart. Machines does not follow the machine switch: it shows every machine and sends its changes to home.
use super::kit::{self, Row};
use super::{updates_strip, updates_version};
use crate::State;
use crate::fleet::updates::RestartOffer;
use crate::theme::{RADIUS_BTN, Theme, sans};
use crate::ui::button::disabled;
use crate::ui::rem;
use rune::{Cursor, El, col, row, text};

pub fn group(s: &State, t: &Theme) -> El {
    let rows = s.update_rows();
    let rollout = s.updates.rollout.as_ref();
    let mut kids = vec![col().key("machines-updates-head").w_full().px(14.0).pt(10.5).pb(10.5).child(updates_strip::view(s, t, &rows))];
    for r in &rows {
        let summary = if r.is_live { "The machine you are working in".to_string() } else { r.where_.clone() };
        let mut item =
            Row::new(&format!("machine-update-{}", r.key), &r.name).summary(&summary).below(updates_version::view(s, t, r, rollout));
        if let Some(RestartOffer::Button { label, disabled: off }) = &r.restart {
            let off = *off || rollout.is_some_and(|x| x.running()) || s.updates.restarting.is_some();
            let (key, from, to) = (r.key.clone(), r.version.clone(), r.state.target.clone());
            let button = restart_button(t, &format!("update-restart-{}", r.key), label);
            item = item.control(if off {
                disabled(button)
            } else {
                button.on_press(move |s: &mut State| s.restart_machine(&key, from.clone(), to.clone()))
            });
        }
        kids.push(item.build(s, t));
    }
    kit::group(t, "machines", "Machines", Some("Each machine's version and when it updates."), kids)
}

/// `rounded-btn border border-border bg-main-bg px-3 py-1.5 text-sm font-medium`.
fn restart_button(t: &Theme, key: &str, label: &str) -> El {
    row()
        .key(key.to_string())
        .role("button")
        .label(label.to_string())
        .center_y()
        .px(10.5)
        .py(5.25)
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.main_bg)
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 2.0, t.accent)
        .hover(t.card_bg)
        .child(text(label.to_string()).font(sans(rem(0.875), 500.0, 1.43)).color(t.text))
}
