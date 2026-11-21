//! Ports `NSD/MachinePicker.vue`: the chip that names the machine a session starts on, and the menu above it (rows B5.1-B5.19).
//! The automation composer draws the same chip as Runs on (component 19, row 11.1). Which machines there are, and what each
//! says, is `fleet::new_session::machines`.
use crate::fleet::new_session::machines::{MACHINE_CHIP, MachineRow};
use crate::icons::*;
use crate::theme::{Theme, sans};
use crate::ui::chip::{Chip, Option_, option, pop, pop_label, pop_separator};
use crate::{State, Tx};
use rune::{Align, El, Side, el};

const WIDTH: f32 = 300.0;
const ADD_KEY: &str = "new-session-machine-add";

fn row_key(key: &str) -> String {
    format!("{MACHINE_CHIP}-{key}")
}

/// B5.3: a 7px dot, green while the machine answers.
fn dot(t: &Theme, reachable: bool) -> El {
    el().size(7.0, 7.0).radius(999.0).fill(if reachable { t.running } else { t.error })
}

/// B5.2-B5.4: the dot, the name in the UI font at 13px, a chevron. Red while the selected machine can't be reached.
pub fn chip(s: &State, t: &Theme) -> El {
    let down = s.target_is_down();
    let disabled = s.picker_busy();
    let chip = Chip::new(MACHINE_CHIP, s.target_name())
        .dot(if down { t.error } else { t.running })
        .label_font(sans(13.0, 500.0, 1.2))
        .open(s.machine_menu_open())
        .disabled(disabled);
    let chip = if down { chip.ink(t.error_text) } else { chip };
    chip.view(t, |s: &mut State, tx: &mut Tx| {
        s.open_machine_menu();
        let at = s.machine_rows().into_iter().find(|r| r.selected).map_or_else(|| ADD_KEY.to_string(), |r| row_key(&r.key));
        tx.focus(at, false);
    })
}

fn machine_option(t: &Theme, r: MachineRow) -> El {
    let key = r.key.clone();
    let spec = Option_ {
        detail: Some(r.detail),
        checked: r.selected,
        disabled: !r.reachable,
        mono_title: true,
        ..Option_::new(row_key(&r.key), r.name)
    };
    let row = option(t, spec, Some(dot(t, r.reachable)), move |s, _| s.pick_machine(&key));
    match r.heard_at {
        Some(at) => row.help(at),
        None => row,
    }
}

/// B5.5-B5.15: the heading, every machine, a rule, Add a machine….
pub fn overlay(s: &State, t: &Theme) -> Vec<El> {
    let mut body = vec![pop_label(t, s.machine_menu_heading())];
    body.extend(s.machine_rows().into_iter().map(|r| machine_option(t, r)));
    body.push(pop_separator(t));
    body.push(option(
        t,
        Option_ { icon: Some(&PLUS), detail: Some("Settings → Machines".into()), ..Option_::new(ADD_KEY, "Add a machine…") },
        None,
        |s, tx| s.add_machine_from_menu(tx),
    ));
    let panel = pop(t, "new-session-machine-pop", WIDTH, body)
        .focus_return(s.picker_focus_return(MACHINE_CHIP))
        .modal(true, |s: &mut State| s.close_machine_menu());
    vec![
        crate::ui::overlay::scrim("new-session-machine-scrim", |s: &mut State| s.close_machine_menu()),
        crate::ui::overlay::anchored(panel, MACHINE_CHIP, Side::Top, Align::Start, 6.0),
    ]
}
