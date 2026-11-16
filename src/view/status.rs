//! The status bar (`StatusBar.vue`): the keyboard hints on the left, the harness and the open
//! session's tokens on the right.
use crate::State;
use crate::composer::status::ComposerStatus;
use crate::fleet::status_hints::{self as hints, Hint};
use crate::theme::{Theme, mono, sans};
use crate::ui::commands;
use rune::{El, row, spacer, text};

pub fn bar(s: &State, t: &Theme) -> El {
    let cmd = if cfg!(target_os = "macos") { "⌘" } else { "Ctrl" };
    let label = sans(11.0, 400.0, 1.3);
    let kbd = |keys: &str| {
        text(keys).font(sans(10.0, 500.0, 1.0)).color(t.text).px(4.0).py(2.0).radius(5.0).fill(t.wash(6.0)).border(1.0, t.border)
    };
    // Fleet draws these separators in the border colour, so they are barely there.
    let sep = || text("·").font(label).color(t.border);
    let find = |id: &str| commands::find(s, id);
    let session_open = s.has_session();
    let prev = hints::command_hint(find("nav-prev-session").as_ref(), &format!("{cmd} ["));
    let next = hints::command_hint(find("nav-next-session").as_ref(), &format!("{cmd} ]"));
    let sidebar = hints::command_hint(find("toggle-sidebar").as_ref(), &format!("{cmd} B"));
    let terminal = hints::command_hint(find("toggle-terminal").as_ref(), &format!("{cmd} J"));
    let interrupt = hints::interrupt_hint(find("interrupt-session").as_ref(), session_open, s.composer_status() == ComposerStatus::Busy);
    let palette = Hint { disabled: false, label: "Command palette".into(), title: format!("Command palette ({cmd} K)") };
    // Prev / next is one hint with `[` and `]` as their own small buttons; the key under the pointer lights up.
    let key_button = |key: &'static str, glyph: &str, h: &Hint, id: &'static str| {
        let mut b = kbd(glyph).key(key).mx(1.0).help(h.title.clone()).label(h.label.clone());
        if h.disabled {
            b = b.opacity(DISABLED).disabled(true);
        } else {
            b = b
                .hover(t.wash(14.0))
                .cursor(rune::Cursor::Pointer)
                .focusable(false)
                .preserve_focus_on_press(true)
                .on_press_tx(move |s: &mut State, tx: &mut crate::Tx| commands::run(s, tx, id));
        }
        b
    };
    let pair = row()
        .key("status-hint-prev-next")
        .center_y()
        .gap(2.0)
        .px(5.0)
        .py(2.0)
        .child(kbd(cmd))
        .child(key_button("status-hint-prev-session", "[", &prev, "nav-prev-session").label("Previous session"))
        .child(key_button("status-hint-next-session", "]", &next, "nav-next-session").label("Next session"))
        .child(text("Prev / next session").font(label).color(t.muted));
    let mut hints_row = row()
        .center_y()
        .gap(1.0)
        .shrink()
        .min_w(0.0)
        .clip_rounded(0.0)
        .child(hint_button(
            t,
            "palette-hint",
            vec![kbd(&format!("{cmd} K"))],
            "Command palette",
            &palette,
            |s: &mut State, tx: &mut crate::Tx| crate::ui::palette::toggle(s, tx),
        ))
        .child(sep())
        .child(pair)
        .child(sep())
        .child(hint_button(
            t,
            "status-hint-sidebar",
            vec![kbd(&format!("{cmd} B"))],
            "Sidebar",
            &sidebar,
            |s: &mut State, tx: &mut crate::Tx| commands::run(s, tx, "toggle-sidebar"),
        ))
        .child(sep())
        .child(hint_button(t, "status-hint-interrupt", vec![kbd("Esc")], "Interrupt", &interrupt, |s: &mut State, tx: &mut crate::Tx| {
            commands::run(s, tx, "interrupt-session")
        }));
    if hints::terminal_shown(s.terminals_allowed, s.link.is_some()) {
        hints_row = hints_row.child(sep()).child(hint_button(
            t,
            "status-hint-terminal",
            vec![kbd(&format!("{cmd} J"))],
            "Terminal",
            &terminal,
            |s: &mut State, tx: &mut crate::Tx| commands::run(s, tx, "toggle-terminal"),
        ));
    }
    let hints = hints_row;
    let mut session = row().center_y().gap(8.0);
    // A newer build is waiting: an item in the same style as the hints, nothing that pops up.
    if let Some(update) = super::update::item(s, t) {
        session = session.child(update).child(text("|").font(label).color(t.border));
    }
    // What's wrong with the server connection, in the colour Fleet gives errors.
    if let Some(note) = s.connection_note() {
        session = session.child(text(note).font(label).color(t.error).truncate()).child(text("|").font(label).color(t.border));
    }
    // On a server, nothing of the invented session shows: no agent and no token total until a session is open.
    if (s.link.is_some() || s.live) && !s.has_session() {
        return row()
            .key("status-bar")
            .w_full()
            .h(28.0)
            .center_y()
            .gap(10.0)
            .px(12.0)
            .pb(2.0)
            .child(hints)
            .child(spacer().min_w(0.0))
            .child(session);
    }
    // `harnessType`, else Fleet's own stand-in; the invented sessions are opencode's.
    let harness = s.session().row.harness_type.clone().unwrap_or_else(|| if s.is_mock() { "opencode" } else { "claude-opus-4" }.into());
    let session = session
        .child(text(harness).font(mono(11.0, 500.0, 1.3)).color(t.muted))
        .child(text("|").font(label).color(t.border))
        .child(text(format!("{} tokens", thousands(s.session().tokens))).font(label.features("'tnum' 1")).color(t.muted));
    row().key("status-bar").w_full().h(28.0).center_y().gap(10.0).px(12.0).pb(2.0).child(hints).child(spacer().min_w(0.0)).child(session)
}

/// `.shortcut-hint:disabled`.
const DISABLED: f32 = 0.45;

/// One hint as Fleet's button (`button.shortcut-hint`): quiet at rest, tinted under the pointer, dim while its command
/// can't run. A press leaves the keyboard where it was (`mousedown.prevent`), so the next shortcut still does what it says.
/// A disabled hint gets no press handler. They stay out of the Tab order: a focusable press takes the keyboard to itself, which
/// is what Fleet's `mousedown.prevent` is there to stop (docs/RUNE.md).
fn hint_button(t: &Theme, key: &str, keys: Vec<El>, what: &str, h: &Hint, run: impl Fn(&mut State, &mut crate::Tx) + 'static) -> El {
    let ink = text(what.to_string()).font(sans(11.0, 400.0, 1.3)).color(t.muted).group_color(t.text);
    let b = row().key(key.to_string()).center_y().gap(4.0).px(5.0).py(2.0).radius(5.0).children(keys).child(ink);
    let b = b.help(h.title.clone()).label(if h.label.is_empty() { what.to_string() } else { h.label.clone() });
    if h.disabled {
        return b.opacity(DISABLED).disabled(true);
    }
    b.hover(t.wash(6.0)).hover_group().cursor(rune::Cursor::Pointer).focusable(false).preserve_focus_on_press(true).on_press_tx(run)
}

/// 12345 as "12,345".
fn thousands(n: u32) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::thousands;

    #[test]
    fn thousands_groups_digits_in_threes() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1_000), "1,000");
        assert_eq!(thousands(12_345), "12,345");
        assert_eq!(thousands(4_294_967_295), "4,294,967,295");
    }
}
