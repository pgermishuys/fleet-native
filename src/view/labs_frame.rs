//! The frame the three Labs views share (component 23, piece A): while the switch is off the off notice fills the whole
//! context (`noPanel`, so no other view's panel sits beside it); while it is on, the view's panel, the gutter and the page.
//! Nothing is drawn before the preferences have loaded, so a view that is on never flashes "is off".
use crate::State;
use crate::fleet::labs::Feature;
use crate::theme::Theme;
use crate::view::{feature_off, gutter, workflows::sheet};
use rune::{El, col};

/// An empty panel `w` wide, where the view's own panel will go (the pieces after this one fill each in).
pub fn panel(s: &State, key: &str, label: &str) -> El {
    col().key(key.to_string()).label(label.to_string()).w(s.sidebar_w).h_full().min_h(0.0)
}

/// The notice, in the sheet the page sits in.
fn off(s: &State, t: &Theme, feature: Feature, key: &str) -> El {
    let notice = match feature {
        Feature::Fleet => feature_off::notice(t, key, feature.title(), feature.description(), s.labs.turning_on(feature), |s| {
            s.labs_turn_on(Feature::Fleet)
        }),
        Feature::Jobs => feature_off::notice(t, key, feature.title(), feature.description(), s.labs.turning_on(feature), |s| {
            s.labs_turn_on(Feature::Jobs)
        }),
        Feature::Board => feature_off::notice(t, key, feature.title(), feature.description(), s.labs.turning_on(feature), |s| {
            s.labs_turn_on(Feature::Board)
        }),
    };
    sheet(t, &format!("{key}-pane")).child(notice)
}

/// The context of a Labs view: `panel` and `page` when it is on (the panel with Ctrl+B not having hidden it), the notice when
/// it is off, an empty sheet until the switch is known.
pub fn page(s: &State, t: &Theme, feature: Feature, key: &str, panel: El, page: El) -> Vec<El> {
    match s.labs_feature(feature) {
        None => vec![sheet(t, &format!("{key}-pane"))],
        Some(false) => vec![off(s, t, feature, key)],
        Some(true) if s.context_panel_shown() => vec![panel, gutter(t), page],
        Some(true) => vec![page],
    }
}

/// The page's own sheet while it is on, with nothing in it yet.
pub fn empty_page(t: &Theme, key: &str) -> El {
    sheet(t, key)
}
