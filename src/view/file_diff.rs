//! A file's Diff (`FileCanvas.vue` with `@codemirror/merge`'s `unifiedMergeView` as `merge.ts` sets it up, rows 6.3 to 6.5 and 6.9 of
//! docs/parity/11-files.md): the whole file, the base's removed lines above the lines that replaced them, a tint and a 3 px change
//! bar on what changed, the changed words darker, and runs of unchanged lines folded to a bar that opens on a click. Also a deleted
//! file's last version, every line of it removed. Read-only: the per-hunk Revert button is editing's (slice 2, row 6.6).
use super::code_view::{Decor, LINE_H, LineKeys, code_line, coloured, draw_line, gutter_width, line_highlights, shown_range};
use crate::State;
use crate::conv::changed_lines::base_lines;
use crate::conv::file_diff::{Item, Kind, MARGIN, MIN_FOLD, Row, deleted_rows, fold_words, items, rows};
use crate::fleet::file_buffers::Buffer;
use crate::theme::{Theme, mono};
use rune::{Color, Cursor, El, GrowthSpec, ScrollSpec, col, el, row, text};
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

/// One row of the diff, as the code view draws a line.
fn diff_row(t: &Theme, r: &Row, i: usize, gutter: f32, shown: (String, Vec<rune::Span>), wrap: bool) -> El {
    let keys = LineKeys { number: format!("diff-no-{i}"), text: format!("diff-text-{i}"), hang: format!("diff-hang-{i}") };
    let d = decor(t, r, &r.text);
    code_line(t, keys, gutter, shown.0, shown.1, wrap, &d)
}

struct Drawn {
    rows: Rc<Vec<Row>>,
    items: Vec<Item>,
    doc: Option<Rc<super::code_view::Coloured>>,
    base: Rc<super::code_view::Coloured>,
    gutter: f32,
}

fn viewport(s: &State, t: &Theme, session: &str, path: &str, drawn: Drawn, wrap: bool, version: u64) -> El {
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
            move |_: &State, i| match drawn.items[i] {
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
                    diff_row(&theme, r, n, drawn.gutter, shown, wrap)
                }
            },
        )
        .virtual_rows_version((path.to_string(), version, wrap));
    let _ = s;
    let view = col().key("file-diff").w_full().grow(1.0).shrink().min_h(0.0);
    (if wrap { view.scroll(ScrollSpec { stick_bottom: false }) } else { view.scroll_xy() }).child(rows_el)
}

/// The Diff of a file: its text against the git base, or (a file git sees as unchanged) against the text as last read (row 6.5).
pub fn diff_view(s: &State, t: &Theme, machine: &str, session: &str, path: &str, buffer: &Buffer, wrap: bool) -> El {
    let base = s.file_base(session, path).map(base_lines);
    let base_hash = s.file_base(session, path).map_or(0, hash_of);
    let reference: Vec<String> = base.unwrap_or_else(|| buffer.saved_lines().to_vec());
    let key: Key = (machine.into(), session.into(), path.into(), buffer.rev(), base_hash);
    let made = cached(key, || rows(&reference, buffer.lines()));
    let items = items(&made, MARGIN, MIN_FOLD, &|first| s.file_folds.is_open(session, path, first));
    let doc = coloured((machine.into(), session.into(), path.into(), buffer.rev()), path, buffer.lines());
    let base_key = (machine.to_string(), session.to_string(), format!("base:{path}"), base_hash ^ buffer.rev().rotate_left(32));
    let base_c = coloured(base_key, path, &reference);
    let drawn = Drawn { rows: made, items, doc: Some(doc), base: base_c, gutter: gutter_width(buffer.lines().len()) };
    let opened: u64 = drawn.items.len() as u64;
    viewport(s, t, session, path, drawn, wrap, buffer.rev() ^ base_hash ^ opened.rotate_left(48))
}

/// A file the session deleted: "This file was deleted in this session. Its last version:", then every line of it as a removed one.
pub fn deleted_view(s: &State, t: &Theme, machine: &str, session: &str, path: &str, before: &str, wrap: bool) -> El {
    let r = Rc::new(deleted_rows(before));
    let lines = base_lines(before);
    let base_c = coloured((machine.into(), session.into(), format!("deleted:{path}"), hash_of(before)), path, &lines);
    let items = (0..r.len()).map(Item::Row).collect();
    let drawn = Drawn { rows: r.clone(), items, doc: None, base: base_c, gutter: gutter_width(r.len()) };
    viewport(s, t, session, path, drawn, wrap, hash_of(before))
}
