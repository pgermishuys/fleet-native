//! Ports the clone view of `NSD/FolderPicker.vue` ("Clone a repository", rows B2.160-B2.170): the repository box, the suggestions
//! the registered sources offer, the Location, the preview of where the clone lands and what git says while it runs, the
//! failure, and Cancel and Clone and use. What the fields mean is `fleet::new_session::clone`; what the keys do is
//! `folder_clone`.
use crate::fleet::new_session::folder_clone::{CLONE_KEY, PARENT_KEY};
use crate::fleet::new_session::folder_menu::Make;
use crate::fleet::new_session::plan::tilde_path;
use crate::icons::*;
use crate::theme::{Theme, mono, sans};
use crate::ui::button::{Size, Variant, button};
use crate::ui::chip::{Option_, option, pop_separator};
use crate::view::folder_menu::make_error;
use crate::{State, Tx};
use rune::{Cursor, El, Key, KeyEvent, TextEdit, col, el, icon, row, text, text_input};

fn back(t: &Theme) -> El {
    option(t, Option_ { icon: Some(&ARROW_LEFT), ..Option_::new("new-session-folder-back", "All folders") }, None, |s, tx| {
        s.show_folder_list(tx)
    })
}

/// `.ns-field__input`: padding `6px 8px`, a border that turns accent in focus, 3% wash.
fn frame(t: &Theme, inner: El) -> El {
    row()
        .w_full()
        .px(8.0)
        .py(6.0)
        .items_center()
        .radius(6.0)
        .border(1.0, t.border)
        .fill(t.wash(3.0))
        .focus_within_ring(1.0, -1.0, t.accent)
        .child(inner)
}

fn box_input(
    s: &State,
    t: &Theme,
    edit: &TextEdit,
    key: &'static str,
    label: &str,
    placeholder: &str,
    pick: fn(&mut State) -> &mut TextEdit,
) -> El {
    // Fleet makes the repository box read-only while cloning; Rune has no read-only input, so it becomes plain text.
    if s.folder_menu.make != Make::Idle && key == CLONE_KEY {
        return frame(t, text(edit.text.clone()).font(mono(12.0, 400.0, 1.3)).color(t.text).truncate().w_full()).key(key);
    }
    let input = text_input(edit, mono(12.0, 400.0, 1.3))
        .key(key)
        .label(label.to_string())
        .placeholder(placeholder.to_string(), t.muted)
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .single_line()
        .w_full()
        .on_edit(pick)
        .on_input_change(|s: &mut State, _: &mut Tx| s.clone_fields_changed())
        .on_input_key(|s: &mut State, ev: &KeyEvent, _: &mut Tx| {
            let enter = ev.key == Key::Enter;
            if enter {
                s.submit_clone();
            }
            enter
        });
    frame(t, input)
}

fn label(t: &Theme, words: &str) -> El {
    text(words.to_string()).font(sans(11.5, 400.0, 1.3)).color(t.muted)
}

fn repository_field(s: &State, t: &Theme) -> El {
    let input = box_input(
        s,
        t,
        &s.folder_menu.clone_text,
        CLONE_KEY,
        "Clone a repository",
        "owner/repo, or an https or ssh address",
        |s: &mut State| &mut s.folder_menu.clone_text,
    );
    col().key("new-session-clone").w_full().gap(4.0).px(8.0).pt(6.0).pb(4.0).child(label(t, "Clone a repository")).child(input)
}

/// `.ns-folder-suggestions`: up to five repositories from the sources, one click puts the name in the box.
fn suggestions(s: &State, t: &Theme) -> Option<El> {
    let shown = s.clone_suggestions();
    if shown.is_empty() {
        return None;
    }
    let mut c = col().key("new-session-clone-suggestions").w_full().gap(1.0).px(8.0).pb(6.0);
    for (n, sg) in shown.into_iter().enumerate() {
        let glyph = if sg.icon == "github" { &GITHUB } else { &FOLDER_GIT_2 };
        let name = sg.full_name.clone();
        c = c.child(
            row()
                .key(format!("new-session-clone-suggestion-{n}"))
                .role("button")
                .w_full()
                .px(6.0)
                .py(4.0)
                .gap(6.0)
                .items_center()
                .radius(4.0)
                .cursor(Cursor::Pointer)
                .hover(t.wash(6.0))
                .focusable(true)
                .focus_ring(2.0, 0.0, t.accent)
                .on_press_tx(move |s: &mut State, tx: &mut Tx| s.pick_clone_suggestion(name.clone(), tx))
                .child(icon(glyph, 12.0).color(t.muted))
                .child(text(sg.full_name).font(mono(11.5, 400.0, 1.3)).color(t.text).truncate().min_w(0.0)),
        );
    }
    Some(c)
}

/// The Location: every location and "Another folder…". Fleet's is a native `<select>`; this box opens its list under itself.
fn location_field(s: &State, t: &Theme) -> El {
    let m = &s.folder_menu;
    let idle = m.make == Make::Idle;
    let shown = m.clone_location.as_deref().map_or("Another folder…".to_string(), tilde_path);
    let mut select = row()
        .key("new-session-clone-location")
        .role("combobox")
        .label("Location")
        .w_full()
        .px(8.0)
        .py(6.0)
        .gap(4.0)
        .items_center()
        .radius(6.0)
        .border(1.0, t.border)
        .fill(t.wash(3.0))
        .child(text(shown).font(sans(13.0, 400.0, 1.3)).color(t.text).truncate().grow(1.0).min_w(0.0))
        .child(icon(&CHEVRON_DOWN, 12.0).color(t.muted));
    select = if idle {
        select.cursor(Cursor::Pointer).focusable(true).focus_ring(2.0, 0.0, t.accent).on_press(|s: &mut State| {
            s.folder_menu.clone_root_list = !s.folder_menu.clone_root_list;
        })
    } else {
        select.opacity(0.5).disabled(true)
    };
    let mut c = col().w_full().gap(4.0).px(8.0).pt(6.0).pb(4.0).child(label(t, "Location")).child(select);
    if idle && m.clone_root_list {
        let item = |key: String, words: String, value: Option<String>, font: rune::TextStyle| {
            row()
                .key(key)
                .role("option")
                .w_full()
                .px(8.0)
                .py(4.0)
                .radius(5.0)
                .cursor(Cursor::Pointer)
                .hover(t.wash(6.0))
                .on_press(move |s: &mut State| s.pick_clone_location(value.clone()))
                .child(text(words).font(font).color(t.text).truncate())
        };
        for (n, root) in s.feeds.roots.iter().enumerate() {
            c = c.child(item(format!("new-session-clone-location-{n}"), tilde_path(root), Some(root.clone()), mono(11.5, 400.0, 1.3)));
        }
        c = c.child(item("new-session-clone-location-other".into(), "Another folder…".into(), None, sans(13.0, 400.0, 1.3)));
    }
    if m.clone_location.is_none() {
        c = c.child(box_input(s, t, &m.clone_parent, PARENT_KEY, "Parent folder", "/path/to/parent", |s: &mut State| {
            &mut s.folder_menu.clone_parent
        }));
    }
    c
}

/// `.ns-folder-preview`: where the clone lands, and what it is (or git's words while it runs).
fn preview(s: &State, t: &Theme) -> Option<El> {
    let plan = s.clone_plan();
    let target = plan.target.clone()?;
    let what = if s.folder_menu.make == Make::Cloning { s.clone_status_text() } else { plan.description() };
    Some(
        col()
            .key("new-session-clone-preview")
            .gap(2.0)
            .mx(8.0)
            .mb(8.0)
            .px(8.0)
            .py(7.0)
            .radius(6.0)
            .fill(t.wash(4.0))
            .child(text(tilde_path(&target)).font(mono(12.0, 400.0, 1.3)).color(t.text).wrap().w_full())
            .child(text(what).font(sans(11.5, 400.0, 1.4)).color(t.muted).wrap().w_full()),
    )
}

fn actions(s: &State, t: &Theme) -> El {
    let cloning = s.folder_menu.make == Make::Cloning;
    let busy = s.folder_menu.make != Make::Idle;
    let cancel = button(t, Variant::Ghost, Size::Sm, None, "Cancel")
        .key("new-session-clone-cancel")
        .on_press_tx(|s: &mut State, tx: &mut Tx| s.show_folder_list(tx));
    let cancel = if busy { cancel.opacity(0.5).disabled(true) } else { cancel };
    let go = button(t, Variant::Default, Size::Sm, None, if cloning { "Cloning…" } else { "Clone and use" })
        .key("new-session-clone-submit")
        .on_press(|s: &mut State| s.submit_clone());
    let go = if s.can_submit_clone() { go } else { go.opacity(0.5).disabled(true) };
    row().w_full().justify_end().gap(6.0).px(8.0).pb(6.0).child(cancel).child(go)
}

/// The whole view, one child of the popover per block.
pub fn clone_view(s: &State, t: &Theme) -> Vec<El> {
    let mut body = vec![back(t), pop_separator(t), repository_field(s, t)];
    body.extend(suggestions(s, t));
    body.push(location_field(s, t));
    body.extend(preview(s, t));
    body.extend(make_error(s, t));
    body.push(actions(s, t));
    let _ = el;
    body
}
