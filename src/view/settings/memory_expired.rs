//! The folded "Expired" list under Settings → Memory's notes (`MemorySection.vue`): learned notes that ran out of days of
//! use. Sessions no longer read them; Keep brings one back for good.
use super::memory_notes::{icon_button, link};
use super::memory_row::with_icon;
use crate::State;
use crate::fleet::settings::memory::{self, List};
use crate::icons::{CHEVRON_RIGHT, FOLDER_GIT_2, HOURGLASS, PIN, SERVER, X};
use crate::theme::{RADIUS_CARD, Theme, sans};
use crate::ui::rem;
use rune::{El, col, icon, row, text};

const EXPLAINER: &str = "Sessions no longer read these. A learned note lasts a week of use, so a lesson whose cause is gone drops out. If the problem \
                         comes back, the agent learns it again and the note returns for twice as long. Keep one you know will stay true.";

/// Nothing when no expired note matches the search; else the toggle, and open the explainer and the notes.
pub fn expired(s: &State, t: &Theme) -> Option<El> {
    let page = &s.settings.memory;
    let notes = page.expired(&s.settings.memory_ui.search.text);
    if notes.is_empty() {
        return None;
    }
    let open = s.settings.memory_ui.show_expired;
    let toggle = row()
        .key("memory-expired-toggle")
        .role("button")
        .label(format!("Expired {}", notes.len()))
        .expanded(open)
        .focusable(true)
        .center_y()
        .gap(7.0)
        .focus_ring(2.0, 1.0, t.accent)
        .on_press(|s: &mut State| s.memory_toggle_expired())
        .child(icon(&CHEVRON_RIGHT, 13.0).color(t.muted).rotate(if open { std::f32::consts::FRAC_PI_2 } else { 0.0 }))
        .child(text("EXPIRED").font(sans(11.0, 600.0, 1.3).tracking(0.025 * 11.0)).color(t.muted))
        .child(text(notes.len().to_string()).font(sans(11.0, 400.0, 1.3)).color(t.muted));
    let mut e = col().key("memory-expired").w_full().mt(17.5).items_start().child(toggle);
    if open {
        let mut list =
            col().w_full().mt(7.0).radius(RADIUS_CARD).border(1.0, t.border).border_layout().fill(t.main_bg).clip_rounded(RADIUS_CARD);
        for (at, note) in notes.iter().enumerate() {
            let (glyph, place) = match note.list {
                List::Machine => (&SERVER, "This machine".to_string()),
                List::Repository => (&FOLDER_GIT_2, page.repository_name().to_string()),
            };
            let (keep, forget) = (note.id.clone(), note.id.clone());
            let busy = page.saving;
            let meta = row()
                .w_full()
                .mt(3.5)
                .flex_wrap()
                .center_y()
                .gaps(3.5, 7.0)
                .child(
                    row()
                        .center_y()
                        .gap(3.5)
                        .child(icon(glyph, 11.0).color(t.muted))
                        .child(text(place).font(sans(11.0, 400.0, 1.4)).color(t.muted)),
                )
                .child(with_icon(t, None, &HOURGLASS, &memory::life_label(note).unwrap_or_default()));
            let left = col()
                .w(0.0)
                .grow(1.0)
                .min_w(0.0)
                .child(text(note.text.clone()).font(sans(rem(0.875), 400.0, 1.43)).color(t.muted).wrap_anywhere().w_full())
                .child(meta);
            let buttons = row()
                .center_y()
                .gap(3.5)
                .child(link(
                    t,
                    format!("memory-keep-{}", note.id),
                    Some(&PIN),
                    "Keep",
                    &format!("Bring back and keep: {}", note.text),
                    busy,
                    move |s: &mut State| s.memory_keep(&keep),
                ))
                .child(icon_button(
                    t,
                    format!("memory-forget-{}", note.id),
                    &X,
                    &format!("Forget note: {}", note.text),
                    None,
                    busy,
                    move |s: &mut State| s.memory_forget(&forget),
                ));
            let mut r = row().key(format!("memory-note-{}", note.id)).w_full().items_start().justify_between().gap(10.5).px(10.5).py(10.5);
            if at > 0 {
                r = r.border_sides([1.0, 0.0, 0.0, 0.0], t.border);
            }
            list = list.child(r.child(left).child(buttons));
        }
        e = e.child(text(EXPLAINER).font(sans(rem(0.75), 400.0, 1.33)).color(t.muted).wrap().w_full().mt(7.0)).child(list);
    }
    Some(e)
}
