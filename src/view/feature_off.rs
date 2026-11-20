//! Ports `components/pages/FeatureOffNotice.vue`: what a Labs page says while its switch is off, so a newcomer knows what they
//! would be turning on. Shared by Workflows and, with component 23, Board, Jobs and Fleet. It fills the whole context area.
use crate::State;
use crate::theme::{Theme, sans};
use crate::ui::button::{Size, Variant, button, disabled};
use crate::ui::rem;
use rune::{Align, El, col, text};

/// "<title> is off", what the feature is for, Turn on (disabled while it saves) and where to change it later.
/// `turn_on` runs on the press; `turning_on` says it is saving. `key` prefixes the elements' keys.
pub fn notice(t: &Theme, key: &str, title: &str, description: &str, turning_on: bool, turn_on: fn(&mut State)) -> El {
    let turn = button(t, Variant::Default, Size::Default, None, "Turn on").key(format!("{key}-turn-on")).mt(rem(1.0));
    let turn = if turning_on { disabled(turn) } else { turn.on_press(turn_on) };
    let inner = col()
        .w_full()
        .max_w(rem(42.0))
        .padding(rem(2.0))
        .items_center()
        .child(text(format!("{title} is off")).font(sans(rem(1.5), 600.0, 1.33)).color(t.text).align(Align::Center))
        .child(text(description.to_string()).font(sans(rem(0.875), 400.0, 1.43)).color(t.muted).wrap().align(Align::Center).mt(rem(0.5)))
        .child(turn)
        .child(text("You can change this any time in Settings.").font(sans(rem(0.75), 400.0, 1.33)).color(t.muted).mt(rem(0.5)));
    col().key(format!("{key}-off")).label(format!("{title} is off")).w_full().h_full().center().child(inner)
}
