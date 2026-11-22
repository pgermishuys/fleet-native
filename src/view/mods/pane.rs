//! `components/canvas/ModPaneCanvas.vue` and `components/mods/ModPaneOff.vue` (component 22, piece G): the body of a pane a mod
//! opened. Its tree, or one of three quiet states: waiting for the mod to draw it, nothing to show, and the mod being off.
use super::tree::Look;
use crate::fleet::canvases::CanvasInstance;
use crate::fleet::mods::panes::pane_key;
use crate::fleet::mods::resolve::Resolved;
use crate::fleet::mods::types::Site;
use crate::theme::{Theme, sans};
use crate::ui::button::{Size, Variant, button};
use crate::ui::empty_state;
use crate::{State, icons::PUZZLE};
use rune::{Align, El, col, text};

/// The body (`.mod-pane`, `mod-pane`): padding 12, scrolls.
pub fn pane(s: &State, t: &Theme, canvas: &CanvasInstance) -> El {
    let Some(mod_pane) = canvas.mod_pane.as_ref() else { return col().key("mod-pane") };
    let session = s.canvas_session();
    let (mod_name, pane_id) = (mod_pane.mod_name.as_str(), mod_pane.pane_id.as_str());
    let view = s.mod_panes.get(&session, mod_name, pane_id);
    let inner = if s.is_mod_off(&session, mod_name) {
        off(s, t, &session, mod_name, pane_id)
    } else {
        let tree = view.as_ref().and_then(|v| match s.mod_resolver.resolve(Some(v), Site::Pane) {
            // A null tree is a valid answer that draws nothing: the same quiet state as no tree.
            Resolved::Draws { tree, .. } if !tree.is_null() => Some(tree),
            _ => None,
        });
        match tree {
            Some(tree) => {
                let key = pane_key(&session, mod_name, pane_id);
                super::tree::tree_keyed(s, t, &key, &tree, Site::Pane, Look::of(Site::Pane), false, None)
            }
            None => {
                let who = view.as_ref().map_or(mod_name, |v| v.mod_id.as_str());
                let title =
                    if view.is_some() { "Nothing to show here yet".to_string() } else { format!("Waiting for {who} to draw this pane") };
                empty_state::compact(t, "mod-pane-empty", &PUZZLE, &title)
            }
        }
    };
    col().key("mod-pane").w_full().grow(1.0).min_h(0.0).px(12.0).py(12.0).child(inner).scroll(rune::ScrollSpec { stick_bottom: false })
}

/// What a pane says while its mod is off (`ModPaneOff`, `mod-pane-off`): centred, padding `56 24 24`, gap 8.
fn off(s: &State, t: &Theme, session: &str, mod_name: &str, pane_id: &str) -> El {
    let busy = s.mod_panes.is_turning(session, mod_name, pane_id);
    let words = |w: String| text(w).font(sans(13.0, 400.0, 1.5)).color(t.muted).wrap().max_w(280.0).align(Align::Center).mb(6.0);
    let (session_id, mod_id, pane) = (session.to_string(), mod_name.to_string(), pane_id.to_string());
    let mut turn_on = button(t, Variant::Default, Size::Sm, None, "Turn on").key("mod-pane-turn-on").on_press(move |s: &mut State| {
        s.turn_mod_on(&session_id, &mod_id, &pane);
    });
    if busy {
        turn_on = turn_on.opacity(0.5).disabled(true);
    }
    let mut c = col()
        .key("mod-pane-off")
        .w_full()
        .items_center()
        .gap(8.0)
        .pt(56.0)
        .px(24.0)
        .pb(24.0)
        .child(text(format!("{mod_name} is off")).font(sans(15.0, 600.0, 1.5)).color(t.text).wrap().align(Align::Center))
        .child(words(format!("This pane is drawn by the {mod_name} mod. Turn it on to see it here.")))
        .child(turn_on);
    if s.mod_panes.turn_failed(session, mod_name, pane_id) {
        c = c.child(
            words(format!("Couldn't turn {mod_name} on. Try again, or use Settings → Skills & tools → Mods."))
                .key("mod-pane-turn-on-failed")
                .role("alert"),
        );
    }
    c
}
