//! `SmartLinkRow.vue`: one link in the Context tab as a row, an icon, the title (a link) over a detail line, and the buttons
//! Pin to header, Unpin and Dismiss.
use super::header_context::link_icon;
use crate::State;
use crate::fleet::context_tab::{row_detail, target_key};
use crate::fleet::smart_links::{Relationship, SmartLink, link_href, link_number, link_title};
use crate::icons::{PIN, PIN_OFF, X};
use crate::theme::{RADIUS_BTN, Theme, sans};
use rune::{Cursor, El, Icon, Shadow, Tx, col, icon, row, text};

/// A 22px square button with a 12px glyph (`.link-row__btn`).
pub(super) fn action(t: &Theme, key: String, glyph: &'static Icon, tip: String, on: impl Fn(&mut State) + Send + Sync + 'static) -> El {
    row()
        .key(key)
        .role("button")
        .label(tip.clone())
        .help(tip)
        .size(22.0, 22.0)
        .center()
        .radius(5.0)
        .cursor(Cursor::Pointer)
        .hover(t.wash(7.0))
        .hover_group()
        .focus_ring(2.0, -1.0, t.accent)
        .on_press(on)
        .child(icon(glyph, 12.0).color(t.muted).group_color(t.text))
}

/// The ring a header chip's request flashes around its target (`context-flash`): the accent at 55% fading out.
pub fn flash(e: El, t: &Theme, s: &State, target: &str) -> El {
    match s.context.flash_ring(target, s.clock) {
        Some(alpha) => e.box_shadow(Shadow { x: 0.0, y: 0.0, blur: 0.0, spread: 2.0, color: t.accent.alpha(alpha), inset: false }),
        None => e,
    }
}

pub fn link_row(s: &State, t: &Theme, link: &SmartLink, note: Option<&str>) -> El {
    let number = link_number(link).map_or("null".to_string(), |n| n.to_string());
    let href = link_href(link);
    let (session, id) = (link.session_id.clone(), link.id.clone());
    let title = text(link_title(link))
        .key(format!("context-link-title-{}", link.id))
        .role("link")
        .font(sans(13.0, 500.0, 1.3))
        .color(t.text)
        .truncate()
        .min_w(0.0)
        .cursor(Cursor::Pointer)
        .help(href.clone())
        .on_press_tx(move |_: &mut State, tx: &mut Tx| {
            let _ = tx.open_url(&href);
        });
    let mut actions = row().gap(2.0);
    match link.relationship {
        Relationship::Mentioned => {
            let (session, id) = (session.clone(), id.clone());
            actions = actions.child(action(t, format!("context-pin-{id}"), &PIN, "Pin to header".into(), move |s: &mut State| {
                s.set_link_pinned(&session, &id, true)
            }));
        }
        Relationship::Pinned => {
            let (session, id) = (session.clone(), id.clone());
            actions = actions.child(action(t, format!("context-unpin-{id}"), &PIN_OFF, "Unpin".into(), move |s: &mut State| {
                s.set_link_pinned(&session, &id, false)
            }));
        }
        _ => {}
    }
    if link.relationship != Relationship::Origin {
        actions = actions
            .child(action(t, format!("context-dismiss-{id}"), &X, "Dismiss".into(), move |s: &mut State| s.dismiss_link(&session, &id)));
    }
    let main = col()
        .grow(1.0)
        .shrink()
        .min_w(0.0)
        .gap(2.0)
        .child(title)
        .child(text(row_detail(link, note)).font(sans(11.5, 400.0, 1.3)).color(t.muted).wrap());
    let line = row()
        .key(target_key(&link.id))
        .label(format!("{} #{number}", link_title(link)))
        .ml(-6.0)
        .mr(-6.0)
        .padding(6.0)
        .gap(8.0)
        .items_start()
        .radius(RADIUS_BTN)
        .child(el_icon(t, link))
        .child(main)
        .child(actions);
    flash(line, t, s, &link.id)
}

/// `SmartLinkIcon` in a 16px column, two pixels down (`.link-row__icon`).
fn el_icon(t: &Theme, link: &SmartLink) -> El {
    row().w(16.0).mt(2.0).child(link_icon(t, link, 14.0))
}
