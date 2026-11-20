//! A file's Diff (`FileCanvas.vue` with `@codemirror/merge`'s `unifiedMergeView` as `merge.ts` sets it up, rows 6.3 to 6.5 and 6.9 of
//! docs/parity/11-files.md): the whole file, the base's removed lines above the lines that replaced them, a tint and a 3 px change
//! bar on what changed, the changed words darker, and runs of unchanged lines folded to a bar that opens on a click. Also a deleted
//! file's last version, every line of it removed. A file's own Diff is editable (piece R, rows 13.8 and 6.6): the lines of the text take
//! the caret, the selection and the keyboard as Edit's do, and each change has a Revert button that puts the base's lines back in
//! the buffer.
use super::code_view::{Decor, DiffEdit, LINE_H, LineKeys, code_line, coloured, draw_line, gutter_width, line_highlights, shown_range};
use crate::State;
use crate::conv::changed_lines::base_lines;
use crate::conv::file_diff::{Chunk, Item, Kind, MARGIN, MIN_FOLD, Row, chunks, deleted_rows, fold_words, items, reverted, rows};
use crate::fleet::file_buffers::Buffer;
use crate::theme::{Theme, mono};
use rune::{Color, Cursor, El, GrowthSpec, ScrollSpec, Shadow, col, el, row, text};
use std::cell::RefCell;
use std::rc::Rc;

type Key = (String, String, String, u64, u64);

thread_local! {
    /// The rows of the last few diffs: worked out once per revision of the text and of its base.
    static ROWS: RefCell<Vec<(Key, Rc<Vec<Row>>)>> = const { RefCell::new(Vec::new()) };
}

fn hash_of(text: &str) -> u64 {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let mut h = DefaultHasher::new();
    text.hash(&mut h);
    h.finish()
}

fn cached(key: Key, make: impl FnOnce() -> Vec<Row>) -> Rc<Vec<Row>> {
    if let Some(hit) = ROWS.with(|c| c.borrow().iter().find(|(k, _)| *k == key).map(|(_, v)| v.clone())) {
        return hit;
    }
    let made = Rc::new(make());
    ROWS.with(|c| {
        let mut c = c.borrow_mut();
        c.retain(|(k, _)| !(k.0 == key.0 && k.1 == key.1 && k.2 == key.2));
        c.push((key, made.clone()));
        if c.len() > 4 {
            c.remove(0);
        }
    });
    made
}

/// What a row of a diff is tinted with, its words, and its change bar (`.cm-changedLine`, `.cm-deletedChunk`, `.cm-changedText`,
/// `.cm-changedLineGutter`, `.cm-deletedLineGutter`).
fn decor(t: &Theme, r: &Row, line: &str) -> Decor {
    let (fill, word, bar) = match r.kind {
        Kind::Same => (None, Color::TRANSPARENT, None),
        Kind::Added => (Some(t.running.alpha(0.10)), t.running.alpha(0.25), Some(t.running)),
        Kind::Removed => (Some(t.error.alpha(0.10)), t.error.alpha(0.28), Some(t.error)),
    };
    Decor {
        active: false,
        number: r.no,
        fill,
        words: r.marks.iter().map(|&(a, b)| (shown_range(line, a..b), word)).collect(),
        // The agent's stripe gutter is still there in Diff, with no stripe in it (`markStripe(…, hidden)`).
        stripe: Some(false),
        change: Some(bar),
        edit: None,
    }
}

/// `.cm-collapsedLines`: a bar between the gutters' ground and the right edge, "¦ N unchanged lines ⦚", pressed to open the run.
fn fold_bar(t: &Theme, key: String, hidden: usize, gutter: f32, press: impl Fn(&mut State) + 'static) -> El {
    let (band, ink) = t.merge_fold();
    let words = fold_words(hidden);
    let font = mono(12.5, 400.0, 1.6);
    let label = row()
        .key(key)
        .role("button")
        .label(words.clone())
        .grow(1.0)
        .h(LINE_H + 4.0)
        .pl(6.0)
        .gap(0.0)
        .items_center()
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, -2.0, t.accent)
        .linear_gradient(std::f32::consts::PI, [(0.0, Color::TRANSPARENT), (0.3, band), (0.7, band), (1.0, Color::TRANSPARENT)])
        .child(text(format!("\u{a6} {words} \u{a6}")).font(font).color(ink).single_line())
        .on_press(press);
    // The number gutter and the two bars' room hold the same ground as every row's.
    row().w_full().h(LINE_H + 4.0).child(el().w(gutter + 5.0 + 3.0).h(LINE_H + 4.0).fill(t.panel_bg)).child(label)
}

/// What the keyboard works on in Diff: the lines it edits, and the chunks Revert is over.
struct Editing {
    machine: String,
    session: String,
    path: String,
    /// The editor has the keyboard: its caret, selection and brackets are drawn.
    focused: bool,
    pair: Option<(crate::fleet::code_edit::Pos, crate::fleet::code_edit::Pos)>,
    blink_off: bool,
    text_w: f32,
    chunks: Rc<Vec<Chunk>>,
    /// Compare (a conflict): each change has the two buttons "Take the agent's" and "Keep mine" where Diff has Revert.
    compare: bool,
}

/// `.cm-chunkButtons button`: 11 px, 1 px `--border`, radius 5, `--card-bg`, padding `1px 8px`, hover `--text` 8 %.
fn chunk_button(t: &Theme, key: String, words: &str, press: impl Fn(&mut State) + 'static) -> El {
    row()
        .key(key)
        .role("button")
        .label(words)
        .px(8.0)
        .py(1.0)
        .radius(5.0)
        .fill(t.card_bg)
        .border(1.0, t.border)
        .border_layout()
        .cursor(Cursor::Pointer)
        .hover(t.wash(8.0))
        .on_press(press)
        .child(text(words).font(crate::theme::sans(11.0, 400.0, 1.4)).color(t.text).single_line())
}

/// `.cm-chunkButtons`: Revert (Diff) or Keep mine and Take the agent's (Compare), floating over the end of the change's last line on a
/// backing of the panel's colour.
fn chunk_buttons(t: &Theme, k: usize, path: &str, compare: bool) -> El {
    let (take, keep) = (path.to_string(), path.to_string());
    let pocket = row().pin(None, Some(1.0), Some(6.0), None).padding(2.0).gap(4.0).radius(7.0).fill(t.panel_bg).box_shadow(Shadow {
        x: 0.0,
        y: 1.0,
        blur: 6.0,
        spread: 0.0,
        color: Color::BLACK.alpha(0.25),
        inset: false,
    });
    if compare {
        return pocket
            .child(chunk_button(t, format!("diff-keep-{k}"), "Keep mine", move |s: &mut State| s.keep_mine_chunk(&keep, k)))
            .child(chunk_button(t, format!("diff-take-{k}"), "Take the agent's", move |s: &mut State| s.take_agents_chunk(&take, k)));
    }
    pocket.child(chunk_button(t, format!("diff-revert-{k}"), "Revert", move |s: &mut State| s.revert_diff_chunk(&take, k)))
}

/// One row of the diff, as the code view draws a line.
fn diff_row(
    t: &Theme,
    r: &Row,
    i: usize,
    gutter: f32,
    shown: (String, Vec<rune::Span>),
    wrap: bool,
    editing: Option<(&Editing, Option<&Buffer>)>,
) -> El {
    let keys = LineKeys { number: format!("diff-no-{i}"), text: format!("diff-text-{i}"), hang: format!("diff-hang-{i}") };
    let mut d = decor(t, r, &r.text);
    let mut revert = None;
    if let Some((e, buffer)) = editing {
        if let Some(line) = r.no.map(|n| n - 1).filter(|_| r.kind != Kind::Removed) {
            let marks = buffer.filter(|_| e.focused).map(|b| super::code_edit::line_edit(&b.editor, b.lines(), line, e.pair, e.blink_off));
            d.edit = Some(DiffEdit { path: e.path.clone(), line, row: i, raw: r.text.clone(), marks, text_w: e.text_w });
        }
        revert = e.chunks.iter().position(|c| c.last_row == i).map(|k| chunk_buttons(t, k, &e.path, e.compare));
    }
    let line = code_line(t, keys, gutter, shown.0, shown.1, wrap, &d);
    match revert {
        Some(b) => line.child(b),
        None => line,
    }
}

struct Drawn {
    editing: Option<Editing>,
    rows: Rc<Vec<Row>>,
    items: Vec<Item>,
    doc: Option<Rc<super::code_view::Coloured>>,
    base: Rc<super::code_view::Coloured>,
    gutter: f32,
}

fn viewport(s: &State, t: &Theme, session: &str, path: &str, drawn: Drawn, wrap: bool, version: (u64, u64)) -> El {
    let editing = drawn.editing.is_some();
    let count = drawn.items.len();
    let theme = *t;
    let (session_id, file) = (session.to_string(), path.to_string());
    let rows_el = col()
        .key("file-diff-lines")
        .w_full()
        .py(6.0)
        .selection_group(true)
        .growth(GrowthSpec::default())
        .virtual_rows(
            count,
            LINE_H,
            {
                let items = drawn.items.clone();
                move |_: &State, i| {
                    let tag = match items[i] {
                        Item::Row(n) => format!("diff-line-{n}"),
                        Item::Fold { first, .. } => format!("diff-bar-{first}"),
                    };
                    std::borrow::Cow::Owned(format!("{tag}{}", if wrap { "w" } else { "" }))
                }
            },
            move |s: &State, i| match drawn.items[i] {
                Item::Fold { first, hidden } => {
                    let (sid, f) = (session_id.clone(), file.clone());
                    fold_bar(&theme, format!("diff-fold-{first}"), hidden, drawn.gutter, move |s: &mut State| {
                        let _ = &sid;
                        s.open_diff_fold(&f, first)
                    })
                }
                Item::Row(n) => {
                    let r = &drawn.rows[n];
                    let (colours, at) = match (r.kind, r.base) {
                        (Kind::Removed, Some(b)) => (&drawn.base, b),
                        (_, _) => match (&drawn.doc, r.no) {
                            (Some(doc), Some(no)) => (doc, no - 1),
                            _ => (&drawn.base, r.base.unwrap_or(0)),
                        },
                    };
                    let shown = draw_line(&theme, &r.text, line_highlights(colours, at, r.text.len()));
                    let buffer = drawn.editing.as_ref().and_then(|e| s.file_buffers.buffer(&e.machine, &e.session, &e.path));
                    diff_row(&theme, r, n, drawn.gutter, shown, wrap, drawn.editing.as_ref().map(|e| (e, buffer)))
                }
            },
        )
        .virtual_rows_version((path.to_string(), version, wrap, s.code_edit.stamp, s.code_edit.blink_off, s.code_edit.focused));
    let _ = s;
    let view = col().key(DIFF).w_full().grow(1.0).shrink().min_h(0.0);
    let mut view = if wrap { view.scroll(ScrollSpec { stick_bottom: false }) } else { view.scroll_xy() };
    if editing {
        // As Edit's code: the view owns its keyboard and shows no ring.
        view = view
            .focusable(true)
            .focus_ring(0.0, 0.0, Color::TRANSPARENT)
            .on_key(|s: &mut State, ev: &rune::KeyEvent, tx| s.code_edit_key(ev, tx))
            .on_layout(|s: &mut State, r: [f32; 4]| s.code_edit.view_h = r[3]);
    }
    view.child(rows_el)
}

/// The scroll box of Diff, which has the keyboard while the file is edited there.
pub const DIFF: &str = "file-diff";

/// What a file's diff is drawn against: the git base, or (a file git sees as unchanged) the text as last read (row 6.5).
fn reference_of(s: &State, session: &str, path: &str, buffer: &Buffer) -> Vec<String> {
    s.file_base(session, path).map(base_lines).unwrap_or_else(|| buffer.saved_lines().to_vec())
}

/// The rows of the last draw of a file's diff.
fn drawn_rows(machine: &str, session: &str, path: &str) -> Option<Rc<Vec<Row>>> {
    ROWS.with(|c| c.borrow().iter().find(|(k, _)| k.0 == machine && k.1 == session && k.2 == path).map(|(_, v)| v.clone()))
}

/// The line of the text (0-based) under the pointer, `y` points below the pressed row `row` of the diff: rows count a line each, and a
/// row with no line of the text (a removed one) takes the nearest line above it, or below when there is none.
pub fn line_under(s: &State, path: &str, row: usize, line: usize, y: f32) -> usize {
    let session = s.canvas_session();
    let machine = s.machine_key_of(&session).to_string();
    let Some(rows) = drawn_rows(&machine, &session, path) else { return line };
    let target = (row as f32 + (y / LINE_H).floor()).clamp(0.0, rows.len().saturating_sub(1) as f32) as usize;
    let numbered = |i: usize| rows.get(i).and_then(|r| r.no);
    (0..=target).rev().find_map(numbered).or_else(|| (target..rows.len()).find_map(numbered)).map_or(line, |n| n - 1)
}

/// The row of the diff a line of the text is drawn in.
fn row_of_line(machine: &str, session: &str, path: &str, line: usize) -> Option<usize> {
    drawn_rows(machine, session, path)?.iter().position(|r| r.kind != Kind::Removed && r.no == Some(line + 1))
}

impl State {
    /// Revert on change `k` of the open file's Diff: the base's lines go back in the buffer as one step undo takes back (it is the
    /// buffer, not the disk, so the file is then unsaved).
    pub fn revert_diff_chunk(&mut self, path: &str, k: usize) {
        let session = self.canvas_session();
        let machine = self.machine_key_of(&session).to_string();
        let Some(buffer) = self.file_buffers.buffer(&machine, &session, path) else { return };
        let reference = reference_of(self, &session, path, buffer);
        let found = chunks(&rows(&reference, buffer.lines()));
        let Some(chunk) = found.get(k) else { return };
        let after = reverted(buffer.lines(), &reference, chunk);
        let now = self.clock;
        self.file_buffers.replace_as_edit(&machine, &session, path, &after, now);
        self.canvases.keep_file(&session, path);
        self.code_edit.stamp += 1;
    }

    /// Brings the caret's line into view in Diff, opening the fold it is inside.
    pub fn reveal_in_diff(&mut self, tx: &mut rune::Tx) {
        let Some((machine, session, path)) = self.code_edit.file.clone() else { return };
        let line = self.current_editor().map_or(0, |e| e.caret.line);
        let Some(row) = row_of_line(&machine, &session, &path, line) else { return };
        let rows = drawn_rows(&machine, &session, &path).unwrap_or_default();
        let folds = items(&rows, MARGIN, MIN_FOLD, &|first| self.file_folds.is_open(&session, &path, first));
        let hidden = folds.iter().find_map(|i| match *i {
            Item::Fold { first, hidden } if (first..first + hidden).contains(&row) => Some(first),
            _ => None,
        });
        if let Some(first) = hidden {
            self.file_folds.open(&session, &path, first);
        }
        let wrap = self.sidebar.file_wrap(&path);
        tx.scroll_into_view_in(DIFF, format!("diff-line-{row}{}", if wrap { "w" } else { "" }));
        tx.scroll_into_view_in(DIFF, "code-caret");
    }
}

/// The Diff of a file: its text against the git base, or (a file git sees as unchanged) against the text as last read (row 6.5).
pub fn diff_view(s: &State, t: &Theme, machine: &str, session: &str, path: &str, buffer: &Buffer, wrap: bool) -> El {
    let reference = reference_of(s, session, path, buffer);
    let base_hash = s.file_base(session, path).map_or(0, hash_of);
    merge_view(s, t, (machine, session, path), buffer, wrap, &reference, base_hash, false)
}

/// Compare (rows 13.6 and 13.7): the text against the agent's version, whole, with each change's two buttons.
pub fn compare_view(s: &State, t: &Theme, machine: &str, session: &str, path: &str, buffer: &Buffer, wrap: bool) -> El {
    let reference = s.file_compare.original(session, path).cloned().unwrap_or_default();
    let hash = hash_of(&reference.join("\n")) ^ 0x636f_6d70;
    merge_view(s, t, (machine, session, path), buffer, wrap, &reference, hash, true)
}

#[allow(clippy::too_many_arguments)]
fn merge_view(
    s: &State,
    t: &Theme,
    (machine, session, path): (&str, &str, &str),
    buffer: &Buffer,
    wrap: bool,
    reference: &[String],
    base_hash: u64,
    compare: bool,
) -> El {
    let key: Key = (machine.into(), session.into(), path.into(), buffer.rev(), base_hash);
    let made = cached(key, || rows(reference, buffer.lines()));
    // Compare shows the whole file: its merge view has no `collapseUnchanged`.
    let items = if compare {
        (0..made.len()).map(Item::Row).collect()
    } else {
        items(&made, MARGIN, MIN_FOLD, &|first| s.file_folds.is_open(session, path, first))
    };
    let doc = coloured((machine.into(), session.into(), path.into(), buffer.rev()), path, buffer.lines());
    let base_key = (machine.to_string(), session.to_string(), format!("base:{path}"), base_hash ^ buffer.rev().rotate_left(32));
    let base_c = coloured(base_key, path, reference);
    let edit = s.code_edit.on && s.code_edit_target().is_some_and(|(m, se, p)| m == machine && se == session && p == path);
    let editing = edit.then(|| {
        let focused = s.code_edit.focused && s.code_edit.is_over(machine, session, path);
        let pair = (focused && !buffer.editor.has_selection())
            .then(|| crate::fleet::brackets::matching(buffer.lines(), buffer.editor.caret))
            .flatten();
        Editing {
            machine: machine.into(),
            session: session.into(),
            path: path.into(),
            focused,
            pair,
            blink_off: s.code_edit.blink_off,
            text_w: s.code_edit.text_w,
            chunks: Rc::new(chunks(&made)),
            compare,
        }
    });
    let drawn = Drawn { editing, rows: made, items, doc: Some(doc), base: base_c, gutter: gutter_width(buffer.lines().len()) };
    let opened: u64 = drawn.items.len() as u64;
    viewport(s, t, session, path, drawn, wrap, (buffer.rev() ^ base_hash, opened))
}

/// A file the session deleted: "This file was deleted in this session. Its last version:", then every line of it as a removed one.
pub fn deleted_view(s: &State, t: &Theme, machine: &str, session: &str, path: &str, before: &str, wrap: bool) -> El {
    let r = Rc::new(deleted_rows(before));
    let lines = base_lines(before);
    let base_c = coloured((machine.into(), session.into(), format!("deleted:{path}"), hash_of(before)), path, &lines);
    let items = (0..r.len()).map(Item::Row).collect();
    let drawn = Drawn { editing: None, rows: r.clone(), items, doc: None, base: base_c, gutter: gutter_width(r.len()) };
    viewport(s, t, session, path, drawn, wrap, (hash_of(before), 0))
}
