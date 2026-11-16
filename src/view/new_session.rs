//! The New session page's shell (`NewSessionComposer.vue`'s header and stage, `routes/sessions.new.tsx`): the header with its
//! "Not started" pill and the empty stage. Fleet draws no content panel here (A1.6), so this replaces the whole sheet. The strip,
//! chips and message box are pieces E and L; until then `box_slot` holds their place at the size the stage needs.
use crate::State;
use crate::theme::{Theme, sans};
use rune::{El, col, row, text};

/// The page's width limit for the box and the strip (`max-width: 760px`).
pub const COMPOSER_W: f32 = 760.0;

pub fn page(s: &State, t: &Theme) -> El {
    let mut page = col()
        .key("new-session-form")
        .label("New session")
        .grow(1.0)
        .min_w(0.0)
        .h_full()
        .min_h(0.0)
        .fill(t.panel_bg)
        .border(1.0, t.border)
        .border_layout()
        .radius(12.0)
        .clip_rounded(12.0)
        .child(header(t))
        .child(stage(s, t))
        .child(composer(s, t));
    for shadow in t.sheet_shadows() {
        page = page.box_shadow(shadow);
    }
    page
}

/// `.new-session__header`: 56px, a rule beneath, the title and the pill.
fn header(t: &Theme) -> El {
    row()
        .key("new-session-header")
        .w_full()
        .min_h(56.0)
        .center_y()
        .gap(10.0)
        .px(14.0)
        .py(8.0)
        .border_sides([0.0, 0.0, 1.0, 0.0], t.border)
        .child(text("New session").key("new-session-title").font(sans(14.0, 600.0, 1.3).tracking(-0.07)).color(t.text))
        .child(
            text("Not started")
                .key("new-session-pill")
                .font(sans(11.5, 500.0, 1.3))
                .color(t.muted)
                .px(8.0)
                .py(1.0)
                .radius(999.0)
                .border(1.0, t.border),
        )
}

/// `.new-session__stage` with its `.new-session__empty` block, centred both ways.
fn stage(s: &State, t: &Theme) -> El {
    // The starters line while they show (A1.10), else the line that points at the chips (A1.11).
    let starters = s.starters_visible();
    let sub = if starters { "Type below, or start from one of these." } else { "Type below. The chips under the box say where it runs." };
    let mut empty = col()
        .key("new-session-empty")
        .w_full()
        .max_w(640.0)
        .items_center()
        .gap(4.0)
        .child(text("What should we work on?").font(sans(17.0, 600.0, 1.3).tracking(-0.17)).color(t.text))
        .child(text(sub).font(sans(13.5, 400.0, 1.4)).color(t.muted).align(rune::Align::Center).wrap().w_full());
    if starters {
        empty = empty.child(super::starters::line(s, t));
    }
    col().key("new-session-stage").w_full().grow(1.0).min_h(0.0).center().px(24.0).pt(20.0).pb(8.0).child(empty)
}

/// `.new-session__composer`: `4px 24px 18px`, its children at most 760 wide and centred. The message box (piece L) and the plan
/// line (L) are blank slots of about their height, so the stage's centre sits where Fleet's does once they exist (A1.15).
fn composer(s: &State, t: &Theme) -> El {
    col().key("new-session-box").w_full().items_center().px(24.0).pt(4.0).pb(18.0).child(
        col()
            .w_full()
            .max_w(COMPOSER_W)
            .children(super::new_session_strip::no_harness_notice(s, t))
            .child(col().key("new-session-box-slot").w_full().h(66.0).justify_end().child(super::new_session_strip::toolbar_chips(s, t)))
            .child(super::new_session_strip::strip(s, t))
            .child(super::new_session_strip::plan_line(s, t)),
    )
}
