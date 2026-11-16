//! The buttons a row shows while the pointer is over it (`SessionItem.vue` `.session-row-action`, rows 3.60-3.65): Pin (or Unpin)
//! and Archive (or Restore) on the right edge, 24x24, in place of the status word. They sit beside the row, not in it, so
//! pressing one never opens the session. The same buttons serve the live machine's rows and another machine's, which act on
//! their own machine (`machine`).
use crate::icons::{ARCHIVE, ARCHIVE_RESTORE, PIN, PIN_OFF};
use crate::mock::Session;
use crate::theme::Theme;
use crate::{State, Tx};
use rune::{Cursor, El, el, icon};

/// Which buttons a row has (`canPin`, `canArchive`, `canRestore`).
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Buttons {
    pub pin: bool,
    pub pinned: bool,
    pub archive: bool,
    pub restore: bool,
}

impl Buttons {
    /// `read_only`: a machine that isn't answering, whose rows only open. An archived row (the Archived view's) offers Restore.
    pub fn of(session: &Session, read_only: bool) -> Buttons {
        let archived = session.row.archived;
        Buttons {
            pin: !read_only && !archived,
            pinned: session.listing.pin_order.is_some(),
            // The retention state wins over capabilities, which only refresh with the list.
            archive: !read_only && !archived && session.row.capabilities.can_archive.unwrap_or(true),
            restore: !read_only && archived,
        }
    }

    /// `session-item--has-action`: the word and the ring give way.
    pub fn any(self) -> bool {
        self.pin || self.archive || self.restore
    }

    /// `session-item--has-two-actions`: the pull request and the running chip give way too.
    pub fn two(self) -> bool {
        self.archive && self.pin
    }
}

#[allow(clippy::too_many_arguments)]
fn button(t: &Theme, key: String, label: String, tip: &str, glyph: &'static rune::Icon, ink: rune::Color, right: f32, off: bool) -> El {
    el().key(key)
        .role("button")
        .label(label)
        .help(tip.to_string())
        .size(24.0, 24.0)
        .center()
        .radius(6.0)
        // Centred on the 32px row inside its 1px of padding.
        .pin(None, Some(5.0), Some(right), None)
        .cursor(Cursor::Pointer)
        .focus_ring(2.0, -2.0, t.accent)
        .hover(t.wash(8.0))
        .hover_group()
        // `.session-item-shell:hover .session-row-action { display: grid }`: shown while the row has the pointer or focus.
        .group_opacity(0.0, if off { 0.5 } else { 1.0 })
        .disabled(off)
        .child(icon(glyph, 14.0).color(ink).group_color(t.text))
}

/// The buttons of one row, to sit beside the row's own element in a `hover_group()` shell. `key` is the row's key; `title` is
/// what the labels name; `machine` is the machine of an other machine's row.
pub fn actions(s: &State, t: &Theme, key: &str, session: &Session, title: &str, b: Buttons, machine: Option<&str>) -> Vec<El> {
    let mut out = Vec::new();
    let busy = s.is_restoring(&session.id);
    let id = session.id.clone();
    let on = machine.map(str::to_string);
    if b.pin {
        let (id, on) = (id.clone(), on.clone());
        let (label, tip, glyph, ink) = if b.pinned {
            (format!("Unpin {title}"), "Unpin", &PIN_OFF, t.accent)
        } else {
            (format!("Pin {title}"), "Pin to top", &PIN, t.muted)
        };
        // Pin sits left of Archive when both show; alone it takes the edge.
        let right = if b.archive { 31.0 } else { 5.0 };
        out.push(button(t, format!("session-pin-{key}"), label, tip, glyph, ink, right, false).on_press_tx(
            move |s: &mut State, _: &mut Tx| {
                if let Some(m) = &on {
                    s.machines.remember_sessions(m, std::slice::from_ref(&id));
                }
                s.toggle_pin(&id);
            },
        ));
    }
    if b.restore {
        let off = busy;
        out.push(
            button(t, format!("session-restore-{key}"), format!("Restore {title}"), "Restore", &ARCHIVE_RESTORE, t.muted, 5.0, off)
                .on_press_tx(move |s: &mut State, _: &mut Tx| s.restore_session(&id, on.as_deref())),
        );
    } else if b.archive {
        out.push(
            button(t, format!("session-archive-{key}"), format!("Archive {title}"), "Archive", &ARCHIVE, t.muted, 5.0, busy)
                .on_press_tx(move |s: &mut State, tx: &mut Tx| s.archive_sessions(std::slice::from_ref(&id), on.as_deref(), tx)),
        );
    }
    out
}
