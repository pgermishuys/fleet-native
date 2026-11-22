//! `components/mods/review/ModDraftCards.vue` and `ModDraftCard.vue` (component 22, piece H): the drafts the agent wrote in this
//! session, as cards under its last reply. Each says where the draft runs, what is wrong with it, whether the agent asked to keep
//! it, and offers Keep… (or Review…) and Turn off. The dialog they open is piece I's.
use super::draft_mark::draft_mark;
use crate::State;
use crate::fleet::mods::drafts::ModDraft;
use crate::icons::LOADER_CIRCLE;
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, mono, sans};
use crate::ui::button::{Size, Variant, button, button_lead};
use crate::ui::error_note;
use crate::view::conversation::COLUMN;
use rune::{Cycle, El, col, icon, row, text};

/// Where they sit (`.activity-mod-drafts`): as wide as a message, centred, 16 below.
pub fn cards(s: &State, t: &Theme, session: &str) -> El {
    // The cards' 8 px margins collapse in Fleet (a block parent): one 8 between them and the wrapper's 16 below; above them, 28 is what Fleet leaves under a reply (measured).
    let mut c = col().key("mod-draft-cards").w_full().max_w(COLUMN).gap(8.0).pt(28.0).mb(16.0);
    for draft in s.draft_cards(session) {
        c = c.child(card(s, t, draft));
    }
    c
}

fn spinner(t: &Theme) -> El {
    static FADE: [(f32, f32); 3] = [(0.0, 1.0), (0.5, 0.35), (1.0, 1.0)];
    icon(&LOADER_CIRCLE, 14.0).color(t.muted).cycle(Cycle { period: 1.2, phase: 0.0, stops: &FADE, still: 1.0, fps: 30.0 })
}

/// A line under the head (`.mod-draft-card__line`), muted or not.
fn line(t: &Theme, key: &str, words: &str, muted: bool) -> El {
    text(words.to_string()).key(key.to_string()).font(sans(13.0, 400.0, 1.5)).color(if muted { t.muted } else { t.text }).wrap().w_full()
}

/// One card (`mod-draft-card`): grid gap 6, padding `10 12`, border, card radius, 13 px.
pub fn card(s: &State, t: &Theme, d: &ModDraft) -> El {
    let (session, name) = (d.session_id.clone(), d.name.clone());
    let writing = d.still_writing();
    let mut c = col()
        .key(format!("mod-draft-card:{name}"))
        .label(format!("Draft mod {name}"))
        .w_full()
        .gap(6.0)
        .px(12.0)
        .py(10.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout();

    if let Some(request) = &d.keep_request {
        let (se, na) = (session.clone(), name.clone());
        let mut words = col()
            .grow(1.0)
            .shrink()
            .min_w(0.0)
            .child(text("The agent asks you to keep this").font(sans(13.0, 600.0, 1.4)).color(t.text).wrap().w_full());
        if let Some(note) = &request.note {
            words =
                words.child(text(note.clone()).key("mod-draft-note").font(sans(13.0, 400.0, 1.4)).color(t.muted).wrap().w_full().mt(2.0));
        }
        c = c.child(
            row()
                .key("mod-draft-request")
                .items_center()
                .justify_between()
                .gap(12.0)
                .mx(-4.0)
                .mt(-2.0)
                .mb(4.0)
                .px(10.0)
                .py(8.0)
                .radius(RADIUS_BTN)
                .fill(t.accent.alpha(0.12))
                .child(words)
                .child(
                    button(t, Variant::Default, Size::Sm, None, "Review…")
                        .key("mod-draft-review")
                        .on_press(move |s: &mut State| s.open_draft_review(&se, &na)),
                ),
        );
    }

    c = c.child(
        row()
            .w_full()
            .flex_wrap()
            .items_center()
            .gap(8.0)
            .child(draft_mark(t, false))
            .child(text(name.clone()).key("mod-draft-name").font(mono(13.0, 600.0, 1.4)).color(t.text).wrap())
            .child(text(d.status()).key("mod-draft-status").font(sans(12.0, 400.0, 1.4)).color(t.muted)),
    );

    if let Some(why) = d.off.as_ref().filter(|o| o.by_strikes()).and_then(|o| o.error.as_deref()) {
        c = c.child(error_note::compact(t, "mod-draft-strikes", why));
    }
    if writing {
        c = c.child(line(t, "mod-draft-writing", "Still being written", true));
    } else if let Some(description) = d.description.as_deref().filter(|d| !d.is_empty()) {
        c = c.child(line(t, "mod-draft-description", description, false));
    }
    if let Some(n) = d.kept {
        c = c.child(line(t, "mod-draft-replaces", &format!("Replaces your kept v{n} in this session"), true));
    }
    if let Some(problem) = d.problem_line() {
        c = c.child(error_note::compact(t, "mod-draft-check", &problem));
    }
    if let Some(why) = s.mod_drafts.action_error(&session, &name) {
        c = c.child(error_note::compact(t, "mod-draft-action-error", why));
    }

    let mut actions = row().w_full().flex_wrap().items_center().gap(8.0).mt(2.0);
    // The agent's request already offers Review…, which opens the same dialog.
    if d.keep_request.is_none() {
        let (se, na) = (session.clone(), name.clone());
        let mut keep = button(t, Variant::Default, Size::Sm, None, "Keep…")
            .key("mod-draft-keep")
            .on_press(move |s: &mut State| s.open_draft_review(&se, &na));
        if writing {
            keep = keep.opacity(0.5).disabled(true);
        }
        actions = actions.child(keep);
    }
    let busy = s.mod_drafts.is_toggling(&session, &name);
    let label = if d.off.is_some() { "Turn on again" } else { "Turn off" };
    let mut toggle = button_lead(t, Variant::Outline, Size::Sm, busy.then(|| spinner(t)), label)
        .key("mod-draft-toggle")
        .on_press(move |s: &mut State| s.toggle_mod_draft(&session, &name));
    if busy {
        toggle = toggle.opacity(0.5).disabled(true);
    }
    c.child(actions.child(toggle))
}
