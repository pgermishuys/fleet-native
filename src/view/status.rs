//! The status bar (`StatusBar.vue`): the keyboard hints on the left, the harness and the open
//! session's tokens on the right.
use crate::State;
use crate::theme::{Theme, mono, sans};
use rune::{El, row, spacer, text};

pub fn bar(s: &State, t: &Theme) -> El {
    let cmd = if cfg!(target_os = "macos") { "⌘" } else { "Ctrl" };
    let label = sans(11.0, 400.0, 1.3);
    let kbd = |keys: &str| {
        text(keys).font(sans(10.0, 500.0, 1.0)).color(t.text).px(4.0).py(2.0).radius(5.0).fill(t.wash(6.0)).border(1.0, t.border)
    };
    let hint =
        |keys: Vec<El>, what: &str| row().center_y().gap(4.0).px(5.0).py(2.0).children(keys).child(text(what).font(label).color(t.muted));
    // Fleet draws these separators in the border colour, so they are barely there.
    let sep = || text("·").font(label).color(t.border);
    // In a narrow window the hints are cut off before they reach the session's numbers.
    let hints = row()
        .center_y()
        .gap(1.0)
        .shrink()
        .min_w(0.0)
        .clip_rounded(0.0)
        .child(
            hint(vec![kbd(&format!("{cmd} K"))], "Command palette")
                .key("palette-hint")
                .radius(5.0)
                .hover(t.wash(5.0))
                .cursor(rune::Cursor::Pointer)
                .label("Command palette")
                .on_press_tx(|s: &mut State, tx: &mut crate::Tx| crate::ui::palette::open(s, tx)),
        )
        .child(sep())
        .child(hint(vec![kbd(cmd), kbd("["), kbd("]")], "Prev / next session"))
        .child(sep())
        .child(hint(vec![kbd(&format!("{cmd} B"))], "Sidebar"))
        .child(sep())
        .child(hint(vec![kbd("Esc")], "Interrupt"))
        .child(sep())
        .child(hint(vec![kbd(&format!("{cmd} J"))], "Terminal"));
    let mut session = row().center_y().gap(8.0);
    // What's wrong with the server connection, in the colour Fleet gives errors.
    if let Some(note) = s.connection_note() {
        session = session.child(text(note).font(label).color(t.error).truncate()).child(text("|").font(label).color(t.border));
    }
    // On a server, nothing of the invented session shows: no agent and no token total until a session is open.
    if s.link.is_some() && !s.has_session() {
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
    let session = session
        .child(text("opencode").font(mono(11.0, 500.0, 1.3)).color(t.muted))
        .child(text("|").font(label).color(t.border))
        .child(text(format!("{} tokens", thousands(s.session().tokens))).font(label.features("'tnum' 1")).color(t.muted));
    row().key("status-bar").w_full().h(28.0).center_y().gap(10.0).px(12.0).pb(2.0).child(hints).child(spacer().min_w(0.0)).child(session)
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
