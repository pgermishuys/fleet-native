//! A file's tab (`components/canvas/FileCanvas.vue`, rows 4.1 to 4.4 and 4.6 to 4.8 of docs/parity/11-files.md): the bar with the
//! file's path as crumbs and the view switch (Rendered, Edit or Source, Diff), and under it the note while the file opens or can't be
//! shown, or its code, with Wrap (4.5); at its foot the lines bar while lines are picked (`LinesBar.vue`, 8.5) and the tab's toast
//! (4.9). The image and the Rendered page and everything of editing are other pieces'.
use super::code_view::code_view;
use super::file_diff::{deleted_view, diff_view};
use super::file_media::{image_body, note, rendered_html, rendered_markdown};
use crate::State;
use crate::fleet::canvases::{FileView, has_rendered_view, is_image_path};
use crate::fleet::diffs::FileStatus;
use crate::fleet::file_buffers::Status;
use crate::fleet::session_image::Shown;
use crate::icons::{CHEVRON_RIGHT, TEXT_WRAP};
use crate::theme::{Theme, sans};
use rune::{Cursor, El, col, el, icon, row, text};

/// `@container (max-width: 430px)`: a panel this narrow shows the file name and not the folders.
pub const NARROW: f32 = 430.0;

/// Which view shows (rows 4.7, `view`): Diff asked for with nothing changed shows Edit.
pub fn shown_view(asked: FileView, has_changes: bool) -> FileView {
    if asked == FileView::Diff && !has_changes { FileView::Edit } else { asked }
}

/// The folders of a path and its file name.
pub fn crumbs(path: &str) -> (Vec<&str>, &str) {
    let mut parts: Vec<&str> = path.split('/').collect();
    let name = parts.pop().unwrap_or("");
    (parts, name)
}

fn crumb_row(s: &State, t: &Theme, path: &str) -> El {
    let (folders, name) = crumbs(path);
    let narrow = s.right_shown_w() < NARROW;
    let mut r = row().key("file-crumbs").help(path).gap(3.0).items_center().shrink().min_w(0.0);
    if !narrow {
        for (i, folder) in folders.iter().enumerate() {
            r = r
                .child(text(*folder).key(format!("file-crumb-{i}")).font(sans(12.5, 400.0, 1.4)).color(t.muted).single_line())
                .child(icon(&CHEVRON_RIGHT, 11.0).color(t.muted));
        }
    }
    r.child(text(name).key("file-crumb-name").font(sans(12.5, 500.0, 1.4)).color(t.text).truncate().shrink().min_w(0.0))
}

fn switch_button(t: &Theme, key: &str, words: &str, on: bool, off: Option<&str>, tip: &str, press: impl Fn(&mut State) + 'static) -> El {
    let disabled = off.is_some();
    let mut b = row()
        .key(key)
        .role("button")
        .label(words)
        .help(tip)
        .h(22.0)
        .px(8.0)
        .items_center()
        .radius(6.0)
        .cursor(if disabled { Cursor::Default } else { Cursor::Pointer })
        .focusable(!disabled)
        .focus_ring(2.0, 1.0, t.accent)
        .child(text(words).font(sans(12.0, 400.0, 1.4)).color(if on { t.text } else { t.muted }).group_color(t.text).single_line());
    if on {
        b = b.fill(t.wash(10.0));
    }
    if disabled {
        return b.disabled(true).opacity(0.45);
    }
    b.hover_group().on_press(press)
}

/// `.file-canvas__wrap`: 26 high, `wrap-text` and the word "Wrap" (the word goes in a narrow panel), the file's choice pressed.
fn wrap_button(s: &State, t: &Theme, path: &str, on: bool) -> El {
    let narrow = s.right_shown_w() < NARROW;
    let (ink, fill) = if on { (t.accent_text, t.accent_dim) } else { (t.muted, rune::Color::TRANSPARENT) };
    let tip = if cfg!(target_os = "macos") { "Wrap long lines (\u{2325}Z)" } else { "Wrap long lines (Alt Z)" };
    let mut b = row()
        .key("file-wrap")
        .role("button")
        .label("Wrap")
        .help(tip)
        .h(26.0)
        .px(if narrow { 6.0 } else { 8.0 })
        .gap(5.0)
        .items_center()
        .radius(7.0)
        .fill(fill)
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 1.0, t.accent)
        .child(if on { icon(&TEXT_WRAP, 14.0).color(ink) } else { icon(&TEXT_WRAP, 14.0).color(ink).group_color(t.text) });
    if !narrow {
        b = b.child(text("Wrap").font(sans(12.0, 400.0, 1.4)).color(ink).group_color(if on { ink } else { t.text }).single_line());
    }
    if on {
        b = b.checked(true);
    } else {
        b = b.hover_group();
    }
    let path = path.to_string();
    b.on_press(move |s: &mut State| s.set_file_wrap(&path, !on))
}

/// `.file-canvas__toggle`: Rendered (Markdown and HTML only), Edit (Source when Rendered is there) and Diff.
fn view_switch(t: &Theme, path: &str, view: FileView, has_changes: bool) -> El {
    let renderable = has_rendered_view(path);
    let mut group = row().key("file-view-switch").role("group").label("View").gap(1.0).padding(2.0).radius(8.0).fill(t.wash(5.0));
    let set = |to: FileView| {
        let path = path.to_string();
        move |s: &mut State| s.set_active_file_view(&path, to)
    };
    if renderable {
        group =
            group.child(switch_button(t, "file-view-rendered", "Rendered", view == FileView::Rendered, None, "", set(FileView::Rendered)));
    }
    let edit = if renderable { "Source" } else { "Edit" };
    group = group.child(switch_button(t, "file-view-edit", edit, view == FileView::Edit, None, "", set(FileView::Edit)));
    let (off, tip) = if has_changes { (None, "Diff against the last commit") } else { (Some(()), "No changes in this file") };
    group.child(switch_button(t, "file-view-diff", "Diff", view == FileView::Diff, off.map(|_| tip), tip, set(FileView::Diff)))
}

/// `/\.html?$/i`.
pub fn is_html(path: &str) -> bool {
    let p = path.to_ascii_lowercase();
    p.ends_with(".html") || p.ends_with(".htm")
}

/// The note over a deleted file's last version (`FileCanvas.vue` 533-540), said as a status like every note.
pub const DELETED_NOTE: &str = "This file was deleted in this session. Its last version:";

/// A file the session deleted (`deleted`): among the changes as deleted, its read failed and its base is known.
pub fn is_deleted(s: &State, session: &str, path: &str, status: Option<Status>) -> bool {
    status == Some(Status::Error)
        && s.diffs.session() == session
        && s.diffs.by_file(path).is_some_and(|d| d.status == FileStatus::Deleted)
        && s.file_base(session, path).is_some()
}

/// The bar of a file tab: its crumbs, a spacer and what the state says (an image's size).
fn bar(s: &State, t: &Theme, path: &str) -> El {
    row()
        .key("file-bar")
        .w_full()
        .min_h(38.0)
        .pl(14.0)
        .pr(10.0)
        .gap(8.0)
        .items_center()
        .border_sides([0.0, 0.0, 1.0, 0.0], t.border)
        .child(crumb_row(s, t, path))
        .child(el().grow(1.0))
}

/// `.file-canvas__state` while the file has unsaved text or is being saved: a 7 px `--idle` dot, "Unsaved" and Save with its key (the
/// key goes in a narrow panel); "Saving\u{2026}" while the request is out. A file in conflict says "Changed on disk" with an `--error` dot (piece S).
fn save_state(s: &State, t: &Theme, session: &str, path: &str, info: &crate::fleet::file_buffers::Info) -> Option<El> {
    if info.conflict.is_some() {
        // `.file-canvas__state-dot--conflict`: the dot is `--error`.
        let words = text("Changed on disk").font(sans(12.0, 400.0, 1.4)).color(t.muted).single_line();
        return Some(row().key("file-state").gap(8.0).items_center().child(el().w(7.0).h(7.0).radius(3.5).fill(t.error)).child(words));
    }
    let words = |w: &str| text(w.to_string()).font(sans(12.0, 400.0, 1.4)).color(t.muted).single_line();
    if info.saving {
        return Some(row().key("file-state").items_center().child(words("Saving\u{2026}")));
    }
    if !info.dirty {
        return None;
    }
    let narrow = s.right_shown_w() < NARROW;
    // A border that turns to the accent on hover isn't in Rune (docs/RUNE.md): a 1 px ring round the button takes the hover instead.
    let mut inner = row()
        .h(22.0)
        .px(8.0)
        .gap(5.0)
        .items_center()
        .radius(6.0)
        .fill(t.card_bg)
        .child(text("Save").font(sans(12.0, 500.0, 1.4)).color(t.text).single_line());
    if !narrow {
        let key = if cfg!(target_os = "macos") { "\u{2318}S" } else { "Ctrl S" };
        inner = inner.child(text(key).font(sans(10.5, 500.0, 1.4)).color(t.muted).single_line());
    }
    let save = row()
        .key("file-save")
        .role("button")
        .label("Save")
        .padding(1.0)
        .radius(7.0)
        .fill(t.border)
        .hover(t.accent)
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 1.0, t.accent)
        .child(inner);
    let (session, path) = (session.to_string(), path.to_string());
    let save = save.on_press_tx(move |s: &mut State, tx| s.save_file(&session, &path, tx));
    Some(
        row()
            .key("file-state")
            .gap(8.0)
            .items_center()
            .child(el().w(7.0).h(7.0).radius(3.5).fill(t.idle))
            .child(words("Unsaved"))
            .child(save),
    )
}

/// An image's tab: the bar with its size, then the picture. It has no buffer, no Wrap and no view switch: it is only ever shown.
fn image_tab(s: &State, t: &Theme, session: &str, path: &str) -> El {
    let machine = s.machine_key_of(session).to_string();
    let mut bar = bar(s, t, path);
    if let Shown::Picture { meta, .. } = s.session_images.shown(&machine, session, path) {
        bar = bar.child(text(meta).key("image-meta").font(sans(12.0, 400.0, 1.4)).color(t.muted).single_line());
    }
    col().key("file-canvas").w_full().grow(1.0).shrink().min_h(0.0).child(bar).child(image_body(s, t, &machine, session, path))
}

/// The body of the active file tab.
pub fn file_tab(s: &State, t: &Theme) -> El {
    let tabs = s.open_canvases();
    let Some(file) = tabs.active().and_then(|c| c.file.clone()) else { return el() };
    let (session, path) = (s.canvas_session(), file.path.clone());
    if is_image_path(&path) {
        return image_tab(s, t, &session, &path);
    }
    let machine = s.machine_key_of(&session).to_string();
    let info = s.file_buffers.info(&machine, &session, &path);
    let status = info.map(|i| i.status);
    let ready = status == Some(Status::Ready);
    let has_changes = (s.diffs.by_file(&path).is_some() && !crate::fleet::canvases::is_image_path(&path)) || info.is_some_and(|i| i.dirty);
    let view = shown_view(file.view, has_changes);
    let comparing = s.is_comparing(&session, &path);

    let mut bar = bar(s, t, &path);
    let wrap = s.sidebar.file_wrap(&path);
    if let Some(state) = info.and_then(|i| save_state(s, t, &session, &path, i)) {
        bar = bar.child(state);
    }
    if ready && (view != FileView::Rendered || comparing) {
        bar = bar.child(wrap_button(s, t, &path, wrap));
    }
    if ready {
        bar = bar.child(view_switch(t, &path, view, has_changes));
    }

    let deleted = is_deleted(s, &session, &path, status);
    let mut body = col().key("file-canvas").w_full().grow(1.0).shrink().min_h(0.0).child(bar);
    if let Some(conflict) = info.and_then(|i| i.conflict.as_ref()) {
        body = body.child(super::file_compare::notice_for(s, t, &session, &path, conflict, comparing));
    }
    body = match status {
        _ if deleted => {
            let before = s.file_base(&session, &path).unwrap_or_default();
            body.child(note(t, "file-deleted-note", DELETED_NOTE.to_string()))
                .child(deleted_view(s, t, &machine, &session, &path, before, false))
        }
        Some(Status::Loading) => body.child(note(t, "file-opening", format!("Opening {path}\u{2026}"))),
        Some(Status::Unavailable | Status::Error) => {
            let message = info.and_then(|i| i.message.clone()).unwrap_or_default();
            body.child(note(t, "file-unavailable", message))
        }
        Some(Status::Ready) if comparing => match s.file_buffers.buffer(&machine, &session, &path) {
            Some(buffer) => body.child(super::file_diff::compare_view(s, t, &machine, &session, &path, buffer, wrap)),
            None => body,
        },
        Some(Status::Ready) if view == FileView::Rendered && has_rendered_view(&path) => {
            let content = s.file_buffers.buffer(&machine, &session, &path).map(|b| b.lines().join("\n")).unwrap_or_default();
            body.child(if is_html(&path) { rendered_html(s, t, &content) } else { rendered_markdown(t, &path, &content) })
        }
        Some(Status::Ready) if view == FileView::Diff => match s.file_buffers.buffer(&machine, &session, &path) {
            Some(buffer) => body.child(diff_view(s, t, &machine, &session, &path, buffer, wrap)),
            None => body,
        },
        Some(Status::Ready) if s.code_edit.input.as_ref().is_some_and(|(f, _)| f.2 == path) => {
            body.child(super::code_input_spike::input_view(s, t, &path))
        }
        Some(Status::Ready) => body.children(super::file_find::panel(s, t)).child(code_view(s, t, &machine, &session, &path, wrap)),
        None => body,
    };
    // The lines bar, docked at the foot while lines are picked or selected and the code shows (rows 8.5 to 8.7), and the toast.
    let lines = s.file_lines();
    if let Some((path, range)) = &lines {
        let (label, count) = (range.label(), range.count());
        let link = crate::conv::line_quote::line_reference(path, range.first, range.last);
        let bar = super::lines_bar::Bar {
            file: None,
            label: &label,
            count: &count,
            link: Some(&link),
            narrow: s.right_shown_w() - 16.0 <= 404.0,
        };
        body = body.child(super::lines_bar::lines_bar(
            t,
            &bar,
            |s, tx| s.copy_file_link(tx),
            |s, tx| s.copy_file_quote(tx),
            |s, tx| s.add_file_lines_to_message(tx),
            |s, tx| s.clear_file_lines(tx),
        ));
    }
    if let Some((words, _)) = &s.file_lines.toast {
        body = body.child(super::lines_bar::toast(t, "file-toast", words, lines.is_some()));
    }
    body
}
