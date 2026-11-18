//! The review in the Changes tab (`ChangesReview.vue`, rows 5.1 to 5.9 and 6.1 to 6.6 of docs/parity/12-changes-and-review.md): its
//! header (title, scope, totals, the layout choice, Find, Wrap, Collapse all) and a card per changed file with its sticky
//! header and its body's states. A file's hunks, folds and lines are `view::review_lines` (piece F; G draws Split, H only what is
//! near, I Revert).
use crate::State;
use crate::conv::diff::{DiffLine, parse_diff_lines};
use crate::conv::review::{ReviewItem, build_review};
use crate::fleet::diff_contents::Entry;
use crate::fleet::diffs::{FileDiff, FileStatus, diff_base_label};
use crate::fleet::review::{Layout, dir_of, name_of};
use crate::icons::{CHEVRON_DOWN, CHEVRONS_DOWN_UP, CHEVRONS_UP_DOWN, COPY, EXTERNAL_LINK, FILE_TEXT, SEARCH, TEXT_WRAP, UNDO_2};
use crate::theme::{Theme, mono, sans};
use rune::{Cursor, El, Icon, ScrollSpec, Tx, col, el, icon, row, text};
use std::hash::{Hash, Hasher};

use super::review_lines::LINE_H;
use crate::fleet::review::MAX_DRAWN_LINES;

pub fn review(s: &State, t: &Theme) -> El {
    let files = s.review_files();
    let mut body = col().key("review-body").w_full().gap(10.0).px(12.0).pt(12.0).pb(16.0);
    for file in &files {
        body = body.child(section(s, t, file));
    }
    col()
        .key("changes-review")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .child(header(s, t, &files))
        .child(body.scroll(ScrollSpec { stick_bottom: false }).key("review-scroll").w_full().grow(1.0).min_h(0.0))
}

// ── The header ──────────────────────────────────────────────────────────────────────────────────────────────────────

fn totals(t: &Theme, key: &str, additions: u64, deletions: u64) -> El {
    // `.review__totals`: mono 12, tabular, gap 6; the minus is U+2212.
    let font = mono(12.0, 500.0, 1.4).features("'tnum' 1");
    row()
        .key(key.to_string())
        .gap(6.0)
        .label(format!("+{additions}\u{2212}{deletions}"))
        .child(text(format!("+{additions}")).font(font).color(t.diff_add_text()))
        .child(text(format!("\u{2212}{deletions}")).font(font).color(t.diff_del_text()))
}

/// `.review__icon`: 28 square, `--muted`, a wash on hover, `--accent-dim` while pressed.
fn icon_button(t: &Theme, key: &str, tip: &str, glyph: &'static Icon, pressed: bool) -> El {
    let (ink, fill) = if pressed { (t.accent, t.accent_dim) } else { (t.muted, rune::Color::TRANSPARENT) };
    let mut b = el()
        .key(key.to_string())
        .role("button")
        .label(tip.to_string())
        .help(tip.to_string())
        .size(28.0, 28.0)
        .center()
        .radius(7.0)
        .fill(fill)
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 0.0, t.accent)
        .child(if pressed { icon(glyph, 15.0).color(ink) } else { icon(glyph, 15.0).color(ink).group_color(t.text) });
    if pressed {
        b = b.checked(true);
    } else {
        b = b.hover(t.wash(7.0)).hover_group();
    }
    b
}

/// `.review__seg`: a 2px-padded group on a 6% wash; the pressed choice is the panel's colour with a 1px ring.
fn segment(t: &Theme, key: &str, group: &str, small: bool, choices: &[(&str, &str, bool)], pick: impl Fn(&str) -> Pick + 'static) -> El {
    let (px, py, size) = if small { (8.0, 1.0, 12.0) } else { (10.0, 3.0, 12.5) };
    let mut seg = row().key(key.to_string()).role("group").label(group.to_string()).gap(2.0).px(2.0).py(2.0).radius(8.0).fill(t.wash(6.0));
    for (id, words, on) in choices {
        let mut b = row()
            .key(format!("{key}-{id}"))
            .role("button")
            .label((*words).to_string())
            .checked(*on)
            .px(px)
            .py(py)
            .radius(6.0)
            .cursor(Cursor::Pointer)
            .focusable(true)
            .focus_ring(2.0, 0.0, t.accent)
            .child(text((*words).to_string()).font(sans(size, 500.0, 1.4)).color(if *on { t.text } else { t.muted }));
        if *on {
            b = b.fill(t.panel_bg).border(1.0, t.border);
        }
        let pick = pick(id);
        seg = seg.child(b.on_press(move |s: &mut State| pick(s)));
    }
    seg
}

type Pick = Box<dyn Fn(&mut State)>;

fn header(s: &State, t: &Theme, files: &[FileDiff]) -> El {
    let additions: u64 = files.iter().map(|f| f.additions).sum();
    let deletions: u64 = files.iter().map(|f| f.deletions).sum();
    let base = diff_base_label(s.diffs.base.as_ref());
    let tip = base.map_or_else(
        || "Which changes to show".to_string(),
        |l| format!("{}{}", l.text, l.commit.as_deref().map_or(String::new(), |c| format!(" {c}"))),
    );
    let layout = s.review_layout();
    let all_shut = s.review.all_collapsed(files);
    let (shut_tip, shut_icon) = if all_shut { ("Expand all files", &CHEVRONS_UP_DOWN) } else { ("Collapse all files", &CHEVRONS_DOWN_UP) };
    let choices = [("unified", "Unified", layout == Layout::Unified), ("split", "Split", layout == Layout::Split)];
    row()
        .key("review-head")
        .w_full()
        .min_h(46.0)
        .items_center()
        .flex_wrap()
        .gap(10.0)
        .pl(14.0)
        .pr(10.0)
        .py(6.0)
        .border_sides([0.0, 0.0, 1.0, 0.0], t.border)
        .child(text("Changes").font(sans(14.0, 600.0, 1.4)).color(t.text))
        .child(
            row()
                .key("review-scope")
                .h(28.0)
                .items_center()
                .pl(10.0)
                .pr(8.0)
                .help(tip)
                .child(text("Whole session").font(sans(12.5, 400.0, 1.4)).color(t.muted)),
        )
        .child(totals(t, "review-totals", additions, deletions))
        .child(el().grow(1.0))
        .child(segment(t, "review-layout", "Layout", false, &choices, |id| {
            let to = if id == "split" { Layout::Split } else { Layout::Unified };
            Box::new(move |s: &mut State| s.set_review_layout(to))
        }))
        // Find is drawn now and wired by piece K.
        .child(icon_button(t, "review-find", "Find in changes (Ctrl+F)", &SEARCH, false))
        .child(
            icon_button(t, "review-wrap", "Wrap long lines", &TEXT_WRAP, s.sidebar.changes_wrap())
                .on_press(|s: &mut State| s.toggle_review_wrap()),
        )
        .child(icon_button(t, "review-collapse", shut_tip, shut_icon, false).on_press(|s: &mut State| s.toggle_review_all()))
}

// ── A file's section ────────────────────────────────────────────────────────────────────────────────────────────────

/// Why a file can't be reverted as a whole (`revertFileBlocker`).
fn revert_blocker(file: &FileDiff) -> Option<&'static str> {
    if file.binary {
        Some("A binary file can't be reverted from here")
    } else if file.truncated {
        Some("A file this large can't be reverted from here")
    } else {
        None
    }
}

/// `.rfile__act`: 24 high, 12 px, `--muted`; a wash on hover.
fn act(t: &Theme, key: String, tip: &str, glyph: &'static Icon, words: Option<&str>, off: bool) -> El {
    let mut b = row()
        .key(key)
        .role("button")
        .label(words.unwrap_or(tip).to_string())
        .help(tip.to_string())
        .h(24.0)
        .px(6.0)
        .gap(5.0)
        .center_y()
        .radius(6.0)
        .cursor(if off { Cursor::Default } else { Cursor::Pointer })
        .focusable(!off)
        .focus_ring(2.0, 0.0, t.accent)
        .child(icon(glyph, 14.0).color(t.muted).group_color(t.text));
    if let Some(w) = words {
        b = b.child(text(w.to_string()).font(sans(12.0, 400.0, 1.4)).color(t.muted).group_color(t.text));
    }
    if off { b.disabled(true).opacity(0.45) } else { b.hover(t.wash(9.0)).hover_group() }
}

fn section(s: &State, t: &Theme, file: &FileDiff) -> El {
    let open = s.review.is_open(&file.file);
    let path = file.file.clone();
    let toggle = {
        let path = path.clone();
        move |s: &mut State| s.toggle_review_file(&path)
    };
    let blocker = revert_blocker(file);
    let revert_tip = blocker.unwrap_or(if file.status == FileStatus::Added { "Revert file: delete this new file" } else { "Revert file" });
    let copy_path = path.clone();
    // `.rfile__acts`: shown while the header has the pointer or focus, at the right.
    let acts = row()
        .gap(2.0)
        .center_y()
        .h(24.0)
        // Fleet takes the actions out of the line until the header is hovered, so the path keeps the whole width. Here they lie over
        // its right end on the hover colour, and take no room while hidden.
        .pin(None, Some(5.0), Some(10.0), None)
        .fill(t.mix(t.text, t.panel_bg, 0.07))
        .group_opacity(0.0, 1.0)
        // Revert is drawn disabled until piece I; Open file until component 11's file tab (13.3).
        .child(act(t, format!("review-revert-file-{path}"), revert_tip, &UNDO_2, Some("Revert file"), true))
        .child(act(t, format!("review-open-file-{path}"), "Opening files comes with the file viewer", &EXTERNAL_LINK, None, true))
        .child(
            act(t, format!("review-copy-path-{path}"), "Copy path", &COPY, None, false)
                .on_press_tx(move |_: &mut State, tx: &mut Tx| tx.copy(copy_path.clone())),
        );
    let name = row()
        .key(format!("review-path-{path}"))
        .shrink()
        .help(path.clone())
        .child(text(dir_of(&path).to_string()).font(mono(12.5, 400.0, 1.4)).color(t.muted).truncate().shrink())
        .child(text(name_of(&path).to_string()).font(mono(12.5, 500.0, 1.4)).color(t.text).single_line());
    let mut head = row()
        .key(format!("review-file-head-{path}"))
        .role("button")
        .label(path.clone())
        .expanded(open)
        .w_full()
        .h(36.0)
        .items_center()
        .gap(8.0)
        .pl(8.0)
        .pr(10.0)
        .fill(t.mix(t.text, t.panel_bg, 0.04))
        .hover(t.mix(t.text, t.panel_bg, 0.07))
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, -2.0, t.accent)
        .hover_group()
        // Held to the top of the scrolling body while its file scrolls under it (`top: -12px` inside the body's padding).
        .sticky_top(0.0)
        .child(icon(&CHEVRON_DOWN, 14.0).color(t.muted).rotate(if open { 0.0 } else { -std::f32::consts::FRAC_PI_2 }))
        .child(icon(&FILE_TEXT, 15.0).color(t.muted))
        .child(name)
        .child(totals(t, &format!("review-counts-{path}"), file.additions, file.deletions))
        .child(acts)
        .on_press(toggle);
    if open {
        head = head.border_sides([0.0, 0.0, 1.0, 0.0], t.border);
    }
    let mut card = col()
        .key(format!("review-file-{path}"))
        .w_full()
        .fill(t.panel_bg)
        .border(1.0, t.border)
        .border_layout()
        .radius(8.0)
        .clip_rounded(8.0)
        .child(head);
    if open {
        card = card.child(body(s, t, file));
    }
    card
}

/// `.rfile__note`: padding `12px 14px`, 12.5 px `--muted`.
fn note(t: &Theme, key: String, words: &str) -> El {
    row()
        .key(key)
        .w_full()
        .items_center()
        .gap(8.0)
        .px(14.0)
        .py(12.0)
        .child(text(words.to_string()).font(sans(12.5, 400.0, 1.4)).color(t.muted).wrap())
}

/// The file's body: loading, the failure with Try again, a placeholder, no changes, or its lines.
fn body(s: &State, t: &Theme, file: &FileDiff) -> El {
    let key = |what: &str| format!("review-{what}-{}", file.file);
    match s.diff_contents.entry(&file.file) {
        None | Some(Entry::Loading) => {
            // Holds about the height its lines will take, so the files below don't all come into view at once (8.3).
            let held = ((file.additions + file.deletions + 8).min(MAX_DRAWN_LINES as u64) as f32 * LINE_H).max(0.0);
            col().key(key("body")).w_full().min_h(held).child(note(t, key("loading"), "Loading…"))
        }
        Some(Entry::Error(_)) => {
            let path = file.file.clone();
            let retry = row()
                .key(key("retry"))
                .role("button")
                .label("Try again")
                .px(10.0)
                .py(2.0)
                .border(1.0, t.border)
                .cursor(Cursor::Pointer)
                .focusable(true)
                .focus_ring(2.0, 0.0, t.accent)
                .child(text("Try again").font(sans(12.5, 400.0, 1.4)).color(t.text))
                .on_press(move |s: &mut State| s.retry_review_file(&path));
            row()
                .key(key("failed"))
                .w_full()
                .items_center()
                .gap(8.0)
                .px(14.0)
                .py(12.0)
                .child(text("Couldn't load this diff.").font(sans(12.5, 400.0, 1.4)).color(t.muted))
                .child(retry)
        }
        Some(Entry::Ready(item)) if item.binary => note(t, key("binary"), "Binary file \u{2014} no diff available"),
        Some(Entry::Ready(item)) if item.truncated => note(t, key("too-large"), "File too large to diff"),
        Some(Entry::Ready(item)) => {
            let items = review_items(item);
            if items.is_empty() { note(t, key("empty"), "No line changes.") } else { super::review_lines::lines(s, t, file, &items) }
        }
    }
}

/// A file's hunks, worked out once per answer: its lines come from an edit-distance walk that costs far more than drawing them.
fn review_items(item: &FileDiff) -> std::rc::Rc<Vec<ReviewItem>> {
    type Cached = std::collections::HashMap<String, (u64, std::rc::Rc<Vec<ReviewItem>>)>;
    thread_local! {
        static CACHE: std::cell::RefCell<Cached> = Default::default();
    }
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (&item.before, &item.after).hash(&mut h);
    let stamp = h.finish();
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if let Some((s, items)) = c.get(&item.file)
            && *s == stamp
        {
            return items.clone();
        }
        let lines: Vec<DiffLine> = parse_diff_lines(&item.before, &item.after);
        let items = std::rc::Rc::new(build_review(&lines));
        c.insert(item.file.clone(), (stamp, items.clone()));
        items
    })
}
