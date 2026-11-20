//! The cards under the Dreaming panel's summary (`memory/DreamingPanel.vue`): flags (the user's own notes that look out of
//! date, with Edit, Forget and Keep), the changes a dream made (with Why and Undo), and the fold of what it saw once.
use super::dreaming_panel::{session_link, small, small_button};
use super::memory_notes::pressable;
use crate::State;
use crate::fleet::settings::dreaming::{Change, Dream, Kind, list_label, session_label};
use crate::icons::{CHEVRON_DOWN, CHEVRON_RIGHT};
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, mono, sans};
use crate::ui::rem;
use crate::ui::text_menu::text_input;
use rune::{Cursor, El, col, icon, row, text};

/// `text-[11px] font-semibold uppercase tracking-wide text-muted`, the line over a card.
fn head(t: &Theme, kind: Kind, list: &str, ink: rune::Color, tail: &str) -> El {
    let font = sans(11.0, 600.0, 1.45).tracking(0.025 * 11.0);
    let mut line = row().w_full().center_y().gap(7.0).child(text(kind.label().to_uppercase()).font(font).color(ink));
    let rest = format!("{list}{tail}");
    if !rest.is_empty() {
        line = line.child(text(format!("· {}", rest.to_uppercase())).font(font).color(t.muted));
    }
    line
}

fn sm(words: &str, ink: rune::Color) -> El {
    text(words.to_string()).font(sans(rem(0.875), 400.0, 1.43)).color(ink).wrap().w_full().mt(3.5)
}

fn xs(t: &Theme, words: &str) -> El {
    small(t, words).wrap().w_full().mt(3.5)
}

/// `pre`: a quote, monospace on the page's own colour.
fn quote(t: &Theme, words: &str, gap: f32) -> El {
    text(words.to_string())
        .font(mono(11.0, 400.0, 1.5))
        .color(t.text)
        .wrap()
        .w_full()
        .mt(gap)
        .px(7.0)
        .py(7.0)
        .radius(RADIUS_BTN)
        .fill(t.main_bg)
}

/// A note that was replaced: struck through and dimmed.
fn struck(t: &Theme, words: &str) -> El {
    text(words.to_string()).font(sans(rem(0.875), 400.0, 1.43)).color(t.muted).wrap().w_full().mt(7.0).opacity(0.7).strikes([(
        0,
        words.len(),
        None,
    )])
}

fn session_links(s: &State, t: &Theme, change: &Change) -> El {
    let names = s.dreaming_harnesses();
    let mut list = col().w_full().mt(3.5).gap(1.75);
    for (at, session) in change.sessions.iter().enumerate() {
        let label = session_label(session, &names);
        list = list.child(session_link(t, format!("dream-session-{}-{at}", change.id), &label, &session.id, rem(0.75)));
    }
    list
}

// ─── Flags ──────────────────────────────────────────────────────────────────

fn flag_evidence(s: &State, t: &Theme, change: &Change) -> Option<El> {
    if !change.has_evidence() {
        return None;
    }
    let mut block = col().key(format!("dream-flag-evidence-{}", change.id)).w_full().mt(3.5);
    if let Some(e) = change.evidence.as_deref().filter(|e| !e.is_empty()) {
        block = block.child(small(t, e).wrap().w_full());
    }
    if let Some(q) = change.quote.as_deref().filter(|q| !q.is_empty()) {
        block = block.child(quote(t, q, 3.5));
    }
    Some(block.child(session_links(s, t, change)))
}

fn edit_box(s: &State, t: &Theme) -> El {
    let edit = &s.settings.memory.dreaming.edit;
    text_input(edit, sans(rem(0.875), 400.0, 1.43))
        .key("dream-flag-edit-text")
        .label("Edit the note")
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .on_edit(|s: &mut State| &mut s.settings.memory.dreaming.edit)
        .w_full()
        .h(51.0)
        .mt(3.5)
        .px(7.0)
        .py(7.0)
        .radius(RADIUS_BTN)
        .border(1.0, if edit.focused { t.accent } else { t.border })
        .border_layout()
        .fill(t.main_bg)
}

fn flag_buttons(s: &State, t: &Theme, change: &Change) -> El {
    let d = &s.settings.memory.dreaming;
    let id = change.id.clone();
    let working = d.undoing.as_deref() == Some(id.as_str());
    let no_note = change.note_id.is_none();
    let mut buttons = row().w_full().mt(7.0).justify_end().center_y().gap(7.0);
    if d.editing.as_deref() == Some(id.as_str()) {
        let (save_id, off) = (id.clone(), working || d.edit.text.trim().is_empty());
        let save = row()
            .px(7.0)
            .py(1.75)
            .radius(RADIUS_BTN)
            .fill(t.accent)
            .opacity(if off { 0.6 } else { 1.0 })
            .focus_ring(2.0, 1.0, t.accent)
            .child(text("Save").font(sans(rem(0.75), 500.0, 1.33)).color(t.primary_fg));
        buttons = buttons
            .child(pressable(save, "dream-flag-save".into(), "Save", off, move |s: &mut State| s.dreaming_save(&save_id)))
            .child(small_button(t, "dream-flag-cancel".into(), "Cancel", false, |s: &mut State| s.dreaming_cancel_edit()));
    } else {
        let (edit_id, forget_id, keep_id) = (id.clone(), id.clone(), id.clone());
        buttons = buttons
            .child(small_button(t, "dream-flag-edit".into(), "Edit", working || no_note, move |s: &mut State| s.dreaming_edit(&edit_id)))
            .child(small_button(t, "dream-flag-forget".into(), "Forget", working || no_note, move |s: &mut State| {
                s.dreaming_forget(&forget_id)
            }))
            .child(small_button(t, "dream-flag-keep".into(), "Keep", working, move |s: &mut State| s.dreaming_keep(&keep_id)));
    }
    buttons
}

fn flag_card(s: &State, t: &Theme, change: &Change) -> El {
    let d = &s.settings.memory.dreaming;
    let editing = d.editing.as_deref() == Some(change.id.as_str());
    let mut card = col()
        .key(format!("dream-flag-{}", change.id))
        .w_full()
        .px(10.5)
        .py(10.5)
        .radius(RADIUS_CARD)
        .border(1.0, t.idle.alpha(0.4))
        .border_layout()
        .fill(t.card_bg)
        .child(head(t, change.kind, change.list.map_or("", |l| list_label(Some(l))), t.idle_text, " · your note"));
    card = card.child(if editing { edit_box(s, t) } else { sm(change.text.as_deref().unwrap_or(""), t.text) });
    card = card.child(xs(t, &change.why));
    card.children(flag_evidence(s, t, change)).child(flag_buttons(s, t, change))
}

/// The flags nobody has dealt with.
pub fn flags(s: &State, t: &Theme, dream: &Dream) -> Option<El> {
    let open = dream.open_flags();
    if open.is_empty() {
        return None;
    }
    let mut list = col().key("dream-flags").w_full().mt(10.5).gap(7.0);
    for change in open {
        list = list.child(flag_card(s, t, change));
    }
    Some(list)
}

// ─── Changes ────────────────────────────────────────────────────────────────

fn why_button(t: &Theme, change: &Change, open: bool) -> El {
    let id = change.id.clone();
    let e = row()
        .gap(3.5)
        .center_y()
        .focus_ring(2.0, 1.0, t.accent)
        .child(icon(if open { &CHEVRON_DOWN } else { &CHEVRON_RIGHT }, 12.0).color(t.muted))
        .child(text("Why").font(sans(rem(0.75), 400.0, 1.33)).color(t.muted));
    pressable(e, format!("dream-why-{}", change.id), "Why", false, move |s: &mut State| s.dreaming_toggle_why(&id)).cursor(Cursor::Pointer)
}

fn evidence_block(s: &State, t: &Theme, change: &Change) -> El {
    let mut block =
        col().key(format!("dream-evidence-{}", change.id)).w_full().mt(7.0).pt(7.0).border_sides([1.0, 0.0, 0.0, 0.0], t.border);
    if let Some(e) = change.evidence.as_deref().filter(|e| !e.is_empty()) {
        block = block.child(small(t, e).wrap().w_full());
    }
    if !change.sessions.is_empty() {
        let n = change.sessions.len();
        block = block.child(small(t, &format!("{n} of {} sessions", change.of_sessions)).wrap().w_full());
    }
    block = block.child(session_links(s, t, change));
    match change.quote.as_deref().filter(|q| !q.is_empty()) {
        Some(q) => block.child(quote(t, q, 7.0)),
        None => block,
    }
}

fn change_card(s: &State, t: &Theme, change: &Change) -> El {
    let d = &s.settings.memory.dreaming;
    let left = change.kind == Kind::Left;
    let id = change.id.clone();
    let mut card = col()
        .key(format!("dream-change-{id}"))
        .w_full()
        .px(10.5)
        .py(10.5)
        .radius(RADIUS_CARD)
        .border(1.0, if left { t.idle.alpha(0.4) } else { t.border })
        .border_layout()
        .fill(t.card_bg)
        .child(head(t, change.kind, change.list.map_or("", |l| list_label(Some(l))), if left { t.idle_text } else { t.accent_text }, ""));
    if left {
        if let Some(words) = change.text.as_deref().filter(|w| !w.is_empty()) {
            card = card.child(sm(words, t.text));
        }
        return card.child(xs(t, change.reason.as_deref().unwrap_or(&change.why)));
    }
    card = card.child(xs(t, &change.why));
    let replaced = |at: usize| change.replaced.get(at).map(|r| r.text.as_str());
    match change.kind {
        Kind::Rewritten => {
            if let Some(old) = replaced(0) {
                card = card.child(struck(t, old)).child(small(t, "now reads").font(sans(11.0, 400.0, 1.4)).mt(3.5));
            }
        }
        Kind::Merged => {
            for old in &change.replaced {
                card = card.child(struck(t, &old.text));
            }
            card = card.child(small(t, "kept").font(sans(11.0, 400.0, 1.4)).mt(3.5));
        }
        Kind::Retired => {
            if let Some(old) = replaced(0) {
                card = card.child(struck(t, old));
            }
        }
        _ => {}
    }
    if let Some(words) = change.text.as_deref().filter(|w| !w.is_empty() && change.kind != Kind::Retired) {
        card = card.child(sm(words, t.text));
    }
    if let (Kind::Confirmed, Some(how)) = (change.kind, change.how.as_deref().filter(|h| !h.is_empty())) {
        card = card.child(
            text(format!("Tested · {how}"))
                .key(format!("dream-how-{id}"))
                .font(sans(rem(0.75), 400.0, 1.33))
                .color(t.running)
                .wrap()
                .w_full()
                .mt(3.5),
        );
    }
    let open = d.open.contains(&id);
    let working = d.undoing.as_deref() == Some(id.as_str());
    let off = change.undone || working;
    let undo_id = id.clone();
    let undo = small_button(t, format!("dream-undo-{id}"), if change.undone { "Undone" } else { "Undo" }, off, move |s: &mut State| {
        s.dreaming_undo(&undo_id)
    });
    let mut buttons = row().w_full().mt(7.0).justify_between().center_y().gap(7.0);
    buttons = buttons.child(if change.has_evidence() { why_button(t, change, open) } else { row() }).child(undo);
    card = card.child(buttons);
    if open && change.has_evidence() {
        card = card.child(evidence_block(s, t, change));
    }
    card
}

/// What the dream did and what it left as it was, in that order.
pub fn changes(s: &State, t: &Theme, dream: &Dream) -> Option<El> {
    let g = dream.groups();
    let shown: Vec<&Change> = g.applied.iter().chain(&g.left).copied().collect();
    let mut list = col().key("dream-changes").w_full().mt(10.5).gap(7.0);
    for change in shown {
        list = list.child(change_card(s, t, change));
    }
    Some(list)
}

/// "Seen in one session only, not changed (2)", a fold.
pub fn skipped(s: &State, t: &Theme, dream: &Dream) -> Option<El> {
    let g = dream.groups();
    if g.skipped.is_empty() {
        return None;
    }
    let open = s.settings.memory.dreaming.show_skipped;
    let toggle = row()
        .gap(3.5)
        .center_y()
        .focus_ring(2.0, 1.0, t.accent)
        .child(icon(if open { &CHEVRON_DOWN } else { &CHEVRON_RIGHT }, 12.0).color(t.muted))
        .child(small(t, &format!("Seen in one session only, not changed ({})", g.skipped.len())));
    let mut block = col().key("dream-fold").w_full().mt(10.5).child(
        pressable(toggle, "dream-skipped".into(), "Seen in one session only", false, |s: &mut State| s.dreaming_toggle_skipped())
            .cursor(Cursor::Pointer),
    );
    if open {
        let mut lines = col().key("dream-fold-list").w_full().mt(7.0).gap(3.5);
        for change in &g.skipped {
            lines = lines.child(small(t, change.text.as_deref().unwrap_or(&change.why)).wrap().w_full());
        }
        block = block.child(lines);
    }
    Some(block)
}
