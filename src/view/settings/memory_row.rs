//! One note in a Settings → Memory list (`MemorySection.vue`'s `li`): its words or the edit box, the line under them
//! (where it came from, how it is known, how long it has left) and the buttons on the right.
use super::memory_notes::{area, icon_button, link, solid};
use crate::State;
use crate::conv::time::local_offset_secs;
use crate::fleet::settings::memory::{self, Note};
use crate::fleet::settings::memory_notes::origin;
use crate::icons::{HOURGLASS, PENCIL, PIN, X};
use crate::theme::{Theme, sans};
use crate::ui::rem;
use crate::view::message::now_ms;
use rune::{El, col, icon, row, text};

/// `.memory-kind`: a rounded pill, the colours by who wrote the note.
fn kind_pill(t: &Theme, kind: &str) -> El {
    let (fill, ink) = match kind {
        "from-you" => (t.queued.alpha(0.15), t.queued_text),
        "added" => (t.running.alpha(0.15), t.running_text),
        _ => (t.accent_dim, t.accent_text),
    };
    row().px(7.0).radius(999.0).fill(fill).child(text(memory::kind_label(kind)).font(sans(11.0, 600.0, 1.4)).color(ink))
}

fn small(t: &Theme, words: &str) -> El {
    text(words.to_string()).font(sans(11.0, 400.0, 1.4)).color(t.muted)
}

/// `.memory-life`: a small hourglass (or pin) and its words.
pub fn with_icon(t: &Theme, key: Option<String>, glyph: &'static rune::Icon, words: &str) -> El {
    let e = row().center_y().gap(3.5).child(icon(glyph, 11.0).color(t.muted)).child(small(t, words));
    match key {
        Some(key) => e.key(key),
        None => e,
    }
}

/// The line under a note's words (`memory-kind`, the origin, Tested or Untested, Pinned, found, the life label).
fn meta(s: &State, t: &Theme, note: &Note) -> El {
    let (now, offset) = (now_ms(s), local_offset_secs());
    let id = &note.id;
    let search_on = s.settings.memory.search_on();
    let mut line = row()
        .w_full()
        .mt(3.5)
        .flex_wrap()
        .center_y()
        .gaps(3.5, 7.0)
        .child(kind_pill(t, &note.kind))
        .child(small(t, &origin(note, now, offset)));
    if note.kind == "learned" {
        line = line.child(match note.how.as_deref() {
            Some(how) => text(format!("Tested · {how}"))
                .key(format!("memory-how-{id}"))
                .font(sans(11.0, 400.0, 1.4))
                .color(t.running_text)
                .truncate()
                .max_w(392.0)
                .min_w(0.0)
                .shrink()
                .help(how.to_string()),
            None => text("Untested").key(format!("memory-how-{id}")).font(sans(11.0, 400.0, 1.4)).color(t.idle_text),
        });
    }
    if search_on && note.pinned {
        line = line.child(with_icon(t, Some(format!("memory-pinned-{id}")), &PIN, "Pinned"));
    }
    if let Some(found) = search_on.then(|| memory::found_label(note, now, offset)).flatten() {
        line = line.child(small(t, &found).key(format!("memory-found-{id}")));
    }
    if let Some(life) = memory::life_label(note) {
        line = line.child(with_icon(t, Some(format!("memory-life-{id}")), &HOURGLASS, &life));
    }
    line
}

/// The buttons: what moves a note between the two groups, keep, edit, forget.
fn buttons(s: &State, t: &Theme, note: &Note) -> El {
    let busy = s.settings.memory.saving;
    let search_on = s.settings.memory.search_on();
    let id = &note.id;
    let mut e = row().center_y().gap(3.5);
    let to_search = |label: &str| {
        let (key, who) = (format!("memory-to-search-{id}"), id.clone());
        link(t, key, None, label, &format!("Move to search: {}", note.text), busy, move |s: &mut State| s.memory_move(&who, "search"))
    };
    if search_on && !note.in_search && !note.pinned {
        e = e.child(to_search("Move to search"));
    } else if search_on && note.pinned {
        e = e.child(to_search("Unpin and move to search"));
    }
    if search_on && note.in_search {
        let who = id.clone();
        e = e.child(link(
            t,
            format!("memory-to-always-{id}"),
            None,
            "Always read",
            &format!("Always read: {}", note.text),
            busy,
            move |s: &mut State| s.memory_move(&who, "always"),
        ));
    }
    if search_on && !note.in_search && !note.pinned {
        let who = id.clone();
        let pin =
            link(t, format!("memory-pin-{id}"), None, "Pin", &format!("Pin to always read: {}", note.text), busy, move |s: &mut State| {
                s.memory_move(&who, "always")
            });
        e = e.child(pin.help("Pin: Fleet never moves it to search by itself"));
    }
    if note.lifetime.is_some() {
        let who = id.clone();
        e = e.child(icon_button(
            t,
            format!("memory-keep-{id}"),
            &PIN,
            &format!("Keep note for good: {}", note.text),
            Some("Keep for good: it won't expire"),
            busy,
            move |s: &mut State| s.memory_keep(&who),
        ));
    }
    let who = id.clone();
    e = e.child(icon_button(
        t,
        format!("memory-edit-{id}"),
        &PENCIL,
        &format!("Edit note: {}", note.text),
        None,
        false,
        move |s: &mut State| s.memory_start_editing(&who),
    ));
    let who = id.clone();
    e.child(icon_button(t, format!("memory-forget-{id}"), &X, &format!("Forget note: {}", note.text), None, busy, move |s: &mut State| {
        s.memory_forget(&who)
    }))
}

/// The note's words, or while it is being edited the box with Save and Cancel.
fn words(s: &State, t: &Theme, note: &Note) -> El {
    let ui = &s.settings.memory_ui;
    if ui.editing.as_deref() != Some(note.id.as_str()) {
        return text(note.text.clone()).font(sans(rem(0.875), 400.0, 1.43)).color(t.text).wrap_anywhere().w_full();
    }
    let id = note.id.clone();
    let submit = id.clone();
    let field = area(t, &ui.edit, ui.edit.focused, "", "Edit note")
        .key("memory-edit-field".to_string())
        .on_edit(|s: &mut State| &mut s.settings.memory_ui.edit)
        .on_submit(move |s: &mut State, _| s.memory_save_edit(&submit))
        .on_input_key(|s: &mut State, ev: &rune::KeyEvent, _: &mut crate::Tx| {
            let esc = ev.key == rune::Key::Escape;
            if esc {
                s.memory_cancel_editing();
            }
            esc
        });
    let off = s.settings.memory.saving || ui.edit.text.trim().is_empty();
    col().w_full().child(field).child(
        row()
            .mt(3.5)
            .gap(7.0)
            .center_y()
            .child(solid(t, "memory-edit-save".to_string(), "Save", off, move |s: &mut State| s.memory_save_edit(&id)))
            .child(link(t, "memory-edit-cancel".to_string(), None, "Cancel", "Cancel", false, |s: &mut State| s.memory_cancel_editing())),
    )
}

/// One row of a list; a rule above all but the first.
pub fn row_for(s: &State, t: &Theme, note: &Note, first: bool) -> El {
    let left = col().w(0.0).grow(1.0).min_w(0.0).child(words(s, t, note)).child(meta(s, t, note));
    let mut e = row().key(format!("memory-note-{}", note.id)).w_full().items_start().justify_between().gap(10.5).px(10.5).py(10.5);
    if !first {
        e = e.border_sides([1.0, 0.0, 0.0, 0.0], t.border);
    }
    e.child(left).child(buttons(s, t, note))
}
