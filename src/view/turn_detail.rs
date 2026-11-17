//! The open turn's detail in the Turns tab (`TurnsCanvas.vue`, `.turn__detail`): the meta line, the files it wrote with
//! their diffs, "Also ran", sub-agents, and the jump to the chat.
use crate::State;
use crate::composer::choice::ModelOption;
use crate::fleet::turns::{SessionTurn, TurnFile, command_outcome, meta_items, nothing_written};
use crate::fleet::turns_tab::TurnsTab;
use crate::theme::{RADIUS_BTN, Theme, mono, sans};
use rune::{Cursor, El, Span, col, el, rich, row, text};

/// Section label: mono 10, tracked 0.1em, uppercase.
fn label(t: &Theme, words: &str) -> El {
    text(words.to_uppercase()).font(mono(10.0, 400.0, 1.5).tracking(1.0)).color(t.muted).mb(4.0)
}

/// Text with its bold part (`<b>`: 500, `--text` at 84 %).
fn with_bold(t: &Theme, lead: String, bold: String, font: rune::TextStyle, ink: rune::Color) -> El {
    let at = lead.len();
    let all = format!("{lead}{bold}");
    let end = all.len();
    rich(all, vec![Span::new(at, end).weight(500.0).color(t.text.alpha(0.84))]).font(font).color(ink)
}

pub fn detail(s: &State, t: &Theme, turn: &SessionTurn, models: &[ModelOption]) -> El {
    let tab = &s.turns_tab;
    let left = if tab.narrow() { 16.0 } else { 41.0 };
    let mut d = col().key(format!("turn-detail-{}", turn.id)).w_full().gap(12.0).pl(left).pr(10.0).pt(2.0).pb(12.0).items_stretch();
    let mut meta = row().key(format!("turn-meta-{}", turn.id)).w_full().flex_wrap().gaps(4.0, 18.0);
    for (name, value) in meta_items(turn, models) {
        meta = meta.child(with_bold(t, format!("{name} "), value, sans(12.0, 400.0, 1.5).features("'tnum' 1"), t.muted));
    }
    d = d.child(meta);

    if turn.files.is_empty() {
        d = d.child(text(nothing_written(turn)).key(format!("turn-note-{}", turn.id)).font(sans(12.5, 400.0, 1.5)).color(t.muted).wrap());
    } else {
        let mut files = col().key(format!("turn-files-{}", turn.id)).w_full().items_stretch().child(label(t, "Wrote this turn"));
        for (i, file) in turn.files.iter().enumerate() {
            files = files.child(file_row(t, tab, turn, i, file));
            if tab.open_file.as_deref() == Some(&TurnsTab::file_key(turn, &file.path)) {
                files = files.child(file_diff(s, t, turn, i, file));
            }
        }
        d = d.child(files);
    }

    if turn.tool_count > 0 {
        let tools = first_words(turn);
        let edits = (turn.edit_count > 0).then(|| format!("{} {}", turn.edit_count, if turn.edit_count == 1 { "edit" } else { "edits" }));
        let lead = if edits.is_some() { format!("{tools} \u{b7} ") } else { tools };
        let first = row().key(format!("turn-chip-tools-{}", turn.id)).child(chip_body(t, lead, edits));
        let mut chips = row().w_full().flex_wrap().gap(6.0).child(chip(t, first));
        for (i, command) in turn.commands.iter().enumerate() {
            let outcome = command_outcome(command);
            let mut inner = row()
                .key(format!("turn-cmd-{}-{i}", turn.id))
                .gap(4.0)
                .center_y()
                .min_w(0.0)
                .child(text(command.label.clone()).font(sans(11.5, 400.0, 1.5)).color(t.muted).truncate().shrink().min_w(0.0));
            if let Some(word) = outcome {
                inner = inner.child(text("\u{b7}").font(sans(11.5, 400.0, 1.5)).color(t.muted)).child(bold(t, word.to_string()));
            }
            chips = chips.child(chip(t, inner).help(command.label.clone()));
        }
        d = d.child(col().w_full().items_stretch().child(label(t, "Also ran")).child(chips));
    }

    if !turn.delegations.is_empty() {
        let mut chips = row().w_full().flex_wrap().gap(6.0);
        for (i, sub) in turn.delegations.iter().enumerate() {
            let inner = row()
                .key(format!("turn-sub-{}-{i}", turn.id))
                .gap(4.0)
                .center_y()
                .min_w(0.0)
                .child(text(sub.title.clone()).font(sans(11.5, 400.0, 1.5)).color(t.muted).truncate().shrink().min_w(0.0))
                .child(text("\u{b7}").font(sans(11.5, 400.0, 1.5)).color(t.muted))
                .child(bold(t, sub.status.wire().to_string()));
            chips = chips.child(chip(t, inner).help(sub.title.clone()));
        }
        d = d.child(col().w_full().items_stretch().child(label(t, "Subagents")).child(chips));
    }

    let id = turn.id.clone();
    let jump = row()
        .key(format!("turn-jump-{}", turn.id))
        .role("button")
        .label("Show this turn in the chat")
        .px(10.0)
        .py(4.0)
        .radius(8.0)
        .border(1.0, t.border)
        .fill(t.card_bg)
        .cursor(Cursor::Pointer)
        .hover(t.wash(6.0))
        .hover_group()
        .focusable(true)
        .focus_ring(2.0, 2.0, t.accent)
        .on_press_tx(move |s: &mut State, tx: &mut rune::Tx| s.show_turn_in_chat(&id, tx))
        .child(text("Show this turn in the chat \u{2191}").font(sans(12.0, 400.0, 1.5)).color(t.text.alpha(0.82)).group_color(t.text));
    d.child(row().w_full().child(jump))
}

fn first_words(turn: &SessionTurn) -> String {
    format!("{} {}", turn.tool_count, if turn.tool_count == 1 { "tool" } else { "tools" })
}

fn bold(t: &Theme, word: String) -> El {
    text(word).font(sans(11.5, 500.0, 1.5)).color(t.text.alpha(0.84))
}

/// The text of the first chip: the plain part, then the bold one when there is one.
fn chip_body(t: &Theme, lead: String, bold_part: Option<String>) -> El {
    let font = sans(11.5, 400.0, 1.5);
    match bold_part {
        Some(b) => with_bold(t, lead, b, font, t.muted),
        None => text(lead).font(font).color(t.muted),
    }
}

/// `.turn__chip`: a pill with a border.
fn chip(t: &Theme, inner: impl Into<El>) -> El {
    row().px(9.0).py(3.0).radius(999.0).border(1.0, t.border).fill(t.card_bg).center_y().min_w(0.0).child(inner.into())
}

fn file_row(t: &Theme, tab: &TurnsTab, turn: &SessionTurn, index: usize, file: &TurnFile) -> El {
    let key = TurnsTab::file_key(turn, &file.path);
    let open = tab.open_file.as_deref() == Some(&key);
    let mut r = row()
        .key(format!("turn-file-{}-{index}", turn.id))
        .role("button")
        .label(file.path.clone())
        .w_full()
        .min_h(28.0)
        .px(8.0)
        .gap(8.0)
        .center_y()
        .radius(RADIUS_BTN)
        .fill(if open { t.wash(5.0) } else { rune::Color::TRANSPARENT })
        .hover(t.wash(5.0))
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, -2.0, t.accent)
        .help(file.path.clone())
        .on_press(move |s: &mut State| s.turns_tab.toggle_file(&key))
        .child(text(file.name.clone()).font(sans(13.0, 400.0, 1.5)).color(if open { t.text } else { t.text.alpha(0.86) }))
        .child(text(file.dir.clone()).font(sans(12.0, 400.0, 1.5)).color(t.muted.alpha(0.8)).truncate().grow(1.0).shrink().min_w(0.0));
    if file.created || file.whole_file {
        let word = if file.created { "new" } else { "changed" };
        r = r.child(row().px(5.0).radius(4.0).fill(t.wash(7.0)).child(text(word).font(sans(10.0, 400.0, 1.5)).color(t.muted)));
    }
    let mut stats = row().gap(6.0).center_y();
    if let Some(a) = additions_text_raw(file.additions) {
        stats = stats.child(text(a).font(mono(11.5, 400.0, 1.5).features("'tnum' 1")).color(t.running_text));
    }
    if let Some(d) = deletions_text_raw(file.deletions) {
        stats = stats.child(text(d).font(mono(11.5, 400.0, 1.5).features("'tnum' 1")).color(t.error_text));
    }
    r.child(stats)
}

/// The file's own counts are raw (not grouped, unlike the bar's totals).
fn additions_text_raw(n: u32) -> Option<String> {
    (n > 0).then(|| format!("+{n}"))
}

fn deletions_text_raw(n: u32) -> Option<String> {
    (n > 0).then(|| format!("\u{2212}{n}"))
}

/// An open file's diff: the lines this turn wrote, 340 px tall at most and scrolling inside, or the note.
fn file_diff(s: &State, t: &Theme, turn: &SessionTurn, index: usize, file: &TurnFile) -> El {
    let wrap = col().key(format!("turn-diff-{}-{index}", turn.id)).w_full().mt(4.0).mb(8.0).ml(8.0).items_stretch();
    if file.diff.is_empty() {
        return wrap.child(
            row()
                .border(1.0, t.border)
                .radius(8.0)
                .px(10.0)
                .py(8.0)
                .child(text("This harness didn't send the lines for this edit.").font(sans(12.0, 400.0, 1.5)).color(t.muted).wrap()),
        );
    }
    let diff = super::diff::diff_view(t, s, &format!("turn-diff-view-{}-{index}", turn.id), &file.diff);
    wrap.child(el().w_full().max_h(340.0).scroll(rune::ScrollSpec { stick_bottom: false }).child(diff))
}
