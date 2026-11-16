//! A machine's heading in the session list (`MachineHeader.vue`): chevron (when it folds), a dot that says the machine
//! answers, its name in coral, and one note on the right. The right-click menu is `ui::menus::machine_heading_context`; the
//! tooltip on a down note is piece H's. Home's heading is this too, only it has no group of its own to fold (its tree is the list's).
use crate::icons::CHEVRON_DOWN;
use crate::theme::{Theme, mono, sans};
use crate::{State, Tx};
use rune::{Cursor, El, el, icon, row, text};

pub struct Heading<'a> {
    pub key: String,
    pub name: &'a str,
    /// The machine can't be reached right now: the dot is red and a note says so in red.
    pub unreachable: bool,
    /// "…", "unreachable" or "unreachable · cached"; it replaces the count.
    pub note: Option<&'a str>,
    /// `Some` makes the heading a button with a chevron; `None` is a plain row that lines its dot up with the others.
    pub expanded: Option<bool>,
    pub count: Option<usize>,
    /// Right-click opens the heading's menu (New session, New project, Work here); without it nothing opens.
    pub menu: bool,
}

pub fn machine_header(t: &Theme, h: Heading) -> El {
    let button = h.expanded.is_some();
    let dot = el().key(format!("{}-dot", h.key)).size(7.0, 7.0).radius(3.5).fill(if h.unreachable { t.error } else { t.running });
    let name = text(h.name.to_string())
        .key(format!("{}-name", h.key))
        .font(mono(11.5, 600.0, 1.3).tracking(0.02 * 11.5))
        .color(t.coral_text)
        .truncate()
        .grow(1.0);
    // The note: first match wins, as in the template (a note, else the count).
    let note_font = sans(11.0, 400.0, 1.3).features("'tnum' 1");
    let note = match (h.note, h.count) {
        (Some(note), _) => {
            let ink = if h.unreachable { t.error_text } else { t.muted.alpha(0.8) };
            Some(text(note.to_string()).key(format!("{}-note", h.key)).font(note_font).color(ink))
        }
        (None, Some(n)) => Some(text(n.to_string()).key(format!("{}-note", h.key)).font(note_font).color(t.muted.alpha(0.8))),
        (None, None) => None,
    };
    let mut line =
        row().key(h.key.clone()).w_full().min_h(28.0).center_y().gap(7.0).mt(6.0).pr(10.0).pl(if button { 4.0 } else { 23.0 }).radius(8.0);
    if let Some(open) = h.expanded {
        let turn = if open { 0.0 } else { -std::f32::consts::FRAC_PI_2 };
        line = line
            .hover(t.wash(5.0))
            .cursor(Cursor::Pointer)
            .focusable(true)
            .focus_ring(2.0, -2.0, t.accent)
            .child(icon(&CHEVRON_DOWN, 12.0).color(t.muted.alpha(0.7)).rotate(turn));
    }
    if h.menu {
        let key = h.key.strip_prefix("machine-").unwrap_or(&h.key).to_string();
        line = line.on_secondary_press(move |s: &mut State, at, _: &mut Tx| s.show_machine_heading_menu(&key, at.x, at.y));
    }
    line = line.child(dot).child(name);
    if let Some(note) = note {
        line = line.child(note);
    }
    line
}
