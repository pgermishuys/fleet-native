//! The banners under the header (ports `SessionDetailHeader.vue`'s stopped and archived banners and the delegated-subagent
//! bar of `routes/sessions.$id.tsx`). What they say comes from `HeaderModel`; the Restore button is `toolbar_restore` (piece D).
use crate::State;
use crate::fleet::groups::LineageKind;
use crate::fleet::header::HeaderModel;
use crate::icons::*;
use crate::theme::{Theme, sans};
use rune::{Cursor, El, col, el, icon, row, text};

/// Nothing at all when there is nothing to say, so the header's rule is the last line.
pub fn banners(s: &State, t: &Theme, m: &HeaderModel) -> Option<El> {
    let mut items: Vec<El> = Vec::new();
    if matches!(m.lifecycle, "stopped" | "completed" | "disconnected") {
        items.push(stopped(t, m.lifecycle == "disconnected"));
    }
    if m.archived {
        items.push(archived(s, t, m.actions.can_restore));
    }
    // The page puts the subagent bar under the header chrome, not inside the banner column.
    let delegated = delegated(s, t, m);
    if items.is_empty() && delegated.is_none() {
        return None;
    }
    let mut out = col().key("session-banners").w_full();
    if !items.is_empty() {
        out = out.child(col().gap(12.0).pt(12.0).px(16.0).children(items));
    }
    Some(out.children(delegated))
}

fn stopped(t: &Theme, disconnected: bool) -> El {
    let words = if disconnected {
        "Connection to this session was lost. Weave will reconnect automatically when the backend becomes reachable again, or send a message to reconnect now."
    } else {
        "This session isn't running. Send a message to pick it up again."
    };
    col()
        .key("session-stopped-banner")
        .w_full()
        .px(16.0)
        .py(12.0)
        .border(1.0, t.border)
        .fill(t.secondary())
        .child(text(words).font(sans(14.0, 400.0, 1.4)).color(t.muted).wrap())
}

fn archived(s: &State, t: &Theme, can_restore: bool) -> El {
    let mut banner = row()
        .key("session-archived-banner")
        .w_full()
        .center_y()
        .gap(12.0)
        .px(16.0)
        .py(8.0)
        .border_sides([0.0, 0.0, 1.0, 0.0], t.idle.alpha(0.30))
        .fill(t.idle.alpha(0.10))
        .child(text("This session is archived and read-only.").font(sans(13.0, 400.0, 1.4)).color(t.text).grow(1.0).wrap());
    if can_restore {
        // A large spinner in the icon's place while the restore is on its way, and the pill at 60 % (`disabled:opacity-60`).
        let restoring = s.head.bar.restoring.is_some();
        let glyph = if restoring {
            icon(&LOADER_CIRCLE, 16.0).color(t.muted).cycle(crate::view::ring_popover::spinner())
        } else {
            icon(&ARCHIVE_RESTORE, 13.0).color(t.text)
        };
        let mut pill = row()
            .key("session-restore-button")
            .h(28.0)
            .px(12.0)
            .gap(6.0)
            .center_y()
            .radius(999.0)
            .border(1.0, t.border)
            .fill(t.panel_bg)
            .hover(t.wash(5.0))
            .cursor(Cursor::Pointer)
            .label("Restore")
            .on_press_tx(|s: &mut State, tx: &mut crate::Tx| s.toolbar_restore(tx))
            .child(glyph)
            .child(text("Restore").font(sans(12.0, 500.0, 1.3)).color(t.text));
        if restoring {
            pill = pill.opacity(0.6);
        }
        banner = banner.child(pill);
    }
    banner
}

/// "Delegated subagent session": for a session with a `parentSessionId`. A link when the list holds the parent.
fn delegated(s: &State, t: &Theme, m: &HeaderModel) -> Option<El> {
    let (parent, kind) = m.lineage.clone()?;
    if kind != LineageKind::Subagent {
        return None;
    }
    let ink = t.text;
    let label = s.parent_title(&parent).unwrap_or_else(|| "Parent session".into());
    let caption = text("DELEGATED SUBAGENT SESSION").font(sans(11.5, 700.0, 1.3).tracking(0.92)).color(t.muted);
    let named = if s.parent_listed(&parent) {
        let to = parent.clone();
        text(format!("From {label}"))
            .key("session-parent-link")
            .font(sans(13.3, 600.0, 1.3))
            .color(ink)
            .truncate()
            .cursor(Cursor::Pointer)
            .help(label.clone())
            .on_press_tx(move |s: &mut State, tx: &mut rune::Tx| s.open_parent(&to, tx))
    } else {
        text("Opened from a parent session").key("session-parent-label").font(sans(13.3, 600.0, 1.3)).color(ink).truncate().help(label)
    };
    let to = parent.clone();
    Some(
        row()
            .key("session-delegated-banner")
            .w_full()
            .center_y()
            .gap(10.5)
            .px(12.25)
            .py(10.5)
            .border_sides([0.0, 0.0, 1.0, 0.0], t.border.alpha(0.82))
            .fill(t.muted.alpha(0.32))
            .child(
                row()
                    .grow(1.0)
                    .min_w(0.0)
                    .center_y()
                    .gap(10.5)
                    .child(
                        el().size(28.0, 28.0)
                            .center()
                            .radius(14.0)
                            .fill(t.accent.alpha(0.16))
                            .child(icon(&BOT, 14.0).color(t.mix(t.accent, t.text, 0.7))),
                    )
                    .child(col().min_w(0.0).gap(1.75).child(caption).child(named)),
            )
            .child(
                row()
                    .key("session-back-to-parent")
                    .h(28.0)
                    .px(10.5)
                    .gap(7.0)
                    .center_y()
                    .radius(8.0)
                    .border(1.0, t.border)
                    .fill(t.main_bg)
                    .hover(t.wash(5.0))
                    .cursor(Cursor::Pointer)
                    .label("Back to parent")
                    .on_press_tx(move |s: &mut State, tx: &mut rune::Tx| s.open_parent(&to, tx))
                    .child(icon(&ARROW_LEFT, 14.0).color(t.text))
                    .child(text("Back to parent").font(sans(14.0, 500.0, 1.3)).color(t.text)),
            ),
    )
}
