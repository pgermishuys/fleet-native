//! Ports `components/session/FindInFilesResults.vue`, and the hint and the toggles of `FileBrowserPanel.vue`'s contents half:
//! what the Files tab shows under the search box in "File contents" mode. A failure, "Searching…", or the summary and the matching
//! lines grouped by file, with the notes that follow. The search itself is `fleet::find_text`.
use super::files_tab::spinner;
use crate::State;
use crate::conv::fold::format_duration_tenths;
use crate::fleet::canvases::{file_icon, file_name};
use crate::fleet::find_text::{Engine, FailureKind, FindText, MATCHES_PER_FILE, Stopped, TextFile, TextSearchResult, segments_of};
use crate::theme::{RADIUS_CARD, Theme, mono, sans};
use crate::ui::empty_state;
use rune::{Cursor, El, Span, col, el, icon, row, text};

/// What the list says when the folder isn't a git repository.
const WALK: &str = "This folder isn't a git repository, so Fleet searched every file except build and dependency folders.";

pub fn hit_key(path: &str, line: u32) -> String {
    format!("find-hit-{path}:{line}")
}

fn folder_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

/// `.find-results__note`: a card of 12.5 px words, its first line bold; `edge` colours the border and the bold line.
fn note(t: &Theme, key: &str, edge: rune::Color, bold: rune::Color, lead: &str, rest: Option<(&str, bool)>) -> El {
    let font = |w| sans(12.5, w, 1.45);
    let mut c = col()
        .key(key.to_string())
        .w_full()
        .gap(2.0)
        .mx(2.0)
        .my(8.0)
        .px(12.0)
        .py(9.0)
        .radius(RADIUS_CARD)
        .fill(t.card_bg)
        .border(1.0, edge)
        .items_stretch()
        .child(text(lead.to_string()).font(font(600.0)).color(bold).wrap());
    if let Some((rest, in_mono)) = rest {
        let style = if in_mono { mono(11.5, 400.0, 1.45) } else { font(400.0) };
        c = c.child(text(rest.to_string()).font(style).color(t.muted).wrap());
    }
    c
}

fn warn(t: &Theme, key: &str, lead: &str, rest: &str) -> El {
    note(t, key, t.idle.alpha(0.45), t.idle_text, lead, Some((rest, false)))
}

fn summary_words(found: &TextSearchResult, took_ms: f64) -> String {
    if found.files.is_empty() {
        return String::new();
    }
    let files = format!("{} file{}", found.files.len(), if found.files.len() == 1 { "" } else { "s" });
    let head = if found.stopped == Some(Stopped::Limit) {
        format!("First {} matches, in {files}", found.matches)
    } else {
        format!("{} match{} in {files}", found.matches, if found.matches == 1 { "" } else { "es" })
    };
    let tail = if found.stopped == Some(Stopped::Deadline) {
        "stopped after 10 s".to_string()
    } else {
        format_duration_tenths(Some(took_ms as i64))
    };
    format!("{head} \u{b7} {tail}{}", if found.engine == Engine::Walk { " \u{b7} not a git repository" } else { "" })
}

/// One matching line: its number, and the line cut into plain and marked pieces.
fn hit(s: &State, t: &Theme, file: &TextFile, m: &crate::fleet::find_text::TextMatch) -> El {
    let active = s.find_text.active.as_ref().is_some_and(|a| a.path == file.path && a.line == m.line);
    let (path, line) = (file.path.clone(), m.line);
    let mut words = String::new();
    let mut spans = Vec::new();
    for part in segments_of(&m.text, &m.ranges) {
        if part.marked {
            spans.push(Span::new(words.len(), words.len() + part.text.len()).color(t.text).bg(t.idle.alpha(0.3), 0.0));
        }
        words.push_str(&part.text);
    }
    let font = mono(12.0, 400.0, 1.55);
    let mut r = row()
        .key(hit_key(&file.path, m.line))
        .role("button")
        .label(format!("{}:{}", file.path, m.line))
        .w_full()
        .min_w(0.0)
        .gap(8.0)
        .pl(24.0)
        .pr(6.0)
        .py(2.0)
        .radius(5.0)
        .cursor(Cursor::Pointer)
        .hover(t.wash(7.0))
        .focusable(true)
        .focus_ring(2.0, -2.0, t.accent)
        .child(text(m.line.to_string()).font(font).color(t.muted).min_w(3.2 * 0.6 * 12.0).align(rune::Align::End))
        .child(text(words).font(font).color(t.text.alpha(0.82)).spans(spans).truncate().min_w(0.0).grow(1.0));
    if active {
        r = r.fill(t.wash(7.0)).border(1.0, t.accent.alpha(0.5));
    }
    r.on_press_tx(move |s: &mut State, tx| {
        s.find_text.active = Some(crate::fleet::find_text::VisibleHit { path: path.clone(), line });
        s.open_hit(&path, line, tx)
    })
}

fn group(s: &State, t: &Theme, file: &TextFile) -> El {
    let first = file.matches.first().map_or(1, |m| m.line);
    let open = file.path.clone();
    let heading = row()
        .key(format!("find-file-{}", file.path))
        .role("button")
        .label(file.path.clone())
        .help(file.path.clone())
        .w_full()
        .gap(6.0)
        .center_y()
        .px(6.0)
        .py(5.0)
        .radius(6.0)
        .cursor(Cursor::Pointer)
        .hover(t.wash(5.0))
        .focusable(true)
        .focus_ring(2.0, -2.0, t.accent)
        .child(icon(file_icon(&file.path), 13.0).color(t.muted))
        .child(text(file_name(&file.path).to_string()).font(sans(12.5, 600.0, 1.5)).color(t.text))
        .child(text(folder_of(&file.path).to_string()).font(sans(11.0, 400.0, 1.5)).color(t.muted).truncate().min_w(0.0).grow(1.0))
        .child(el().px(6.0).radius(99.0).fill(t.wash(8.0)).child(text(file.total.to_string()).font(sans(10.5, 400.0, 1.5)).color(t.muted)))
        .on_press_tx(move |s: &mut State, tx| s.open_hit(&open, first, tx));
    let expanded = s.find_text.expanded.contains(&file.path);
    let shown = if expanded { file.matches.len() } else { MATCHES_PER_FILE.min(file.matches.len()) };
    let mut g = col().key(format!("find-group-{}", file.path)).w_full().mb(2.0).items_stretch().child(heading);
    for m in &file.matches[..shown] {
        g = g.child(hit(s, t, file, m));
    }
    if file.matches.len() > MATCHES_PER_FILE && !expanded {
        let path = file.path.clone();
        g = g.child(
            el().key(format!("find-more-{}", file.path))
                .role("button")
                .ml(24.0)
                .mt(1.0)
                .mb(4.0)
                .px(6.0)
                .py(2.0)
                .radius(5.0)
                .cursor(Cursor::Pointer)
                .hover(t.wash(6.0))
                .hover_group()
                .focusable(true)
                .focus_ring(2.0, -2.0, t.accent)
                .child(
                    text(format!("Show {} more in this file", file.matches.len() - MATCHES_PER_FILE))
                        .font(sans(11.5, 400.0, 1.5))
                        .color(t.muted)
                        .group_color(t.text),
                )
                .on_press(move |s: &mut State| s.find_text.expand(&path)),
        );
    }
    g
}

/// What replaces the tree once the query is long enough.
pub fn find_results(s: &State, t: &Theme) -> El {
    let f = &s.find_text;
    let wrap = || col().key("find-results").w_full().px(6.0).pb(14.0).min_w(0.0).items_stretch();
    if let Some(why) = &f.failure {
        let card = match why.kind {
            FailureKind::Pattern => {
                note(t, "find-failure", t.error.alpha(0.45), t.error, "That isn't a valid pattern.", Some((&why.message, true)))
            }
            FailureKind::Unsupported => warn(
                t,
                "find-failure",
                "This machine can't search inside files yet.",
                "The Fleet running on it is older. Update it from Settings \u{2192} Updates & about, then search again.",
            ),
            FailureKind::Failed => note(t, "find-failure", t.error.alpha(0.45), t.error, "The search failed.", Some((&why.message, false))),
        };
        return wrap().child(card);
    }
    let Some(found) = &f.result else {
        let mut status = row().key("find-searching").w_full().center().gap(8.0).px(16.0).py(24.0);
        if f.loading {
            status = status.child(spinner(t.muted, 16.0));
        }
        return wrap().child(status.child(text("Searching\u{2026}").font(sans(12.0, 400.0, 1.5)).color(t.muted)));
    };
    let busy = if f.loading { 0.6 } else { 1.0 };
    let has = !found.files.is_empty();
    let mut c = wrap();
    let summary = summary_words(found, f.took_ms);
    if !summary.is_empty() {
        c = c.child(text(summary).key("find-summary").font(sans(11.5, 400.0, 1.5)).color(t.muted).px(6.0).pt(2.0).pb(6.0).opacity(busy));
    }
    if !has && found.stopped == Some(Stopped::Deadline) {
        c = c.child(warn(
            t,
            "find-timeout",
            "The search ran out of time before it found anything.",
            "That isn't \u{201c}no matches\u{201d}. Try a longer or more exact search.",
        ));
    } else if !has {
        let hint = if f.regex { "Try turning off the regex switch." } else { "Check the spelling, or try the regex switch." };
        let mut why = if f.match_case { format!("{hint} Match case is on.") } else { hint.to_string() };
        if found.engine == Engine::Walk {
            why = format!("{why} {WALK}");
        }
        c = c.child(empty_state::compact_described(
            t,
            "find-empty",
            None,
            &format!("No matches for \u{201c}{}\u{201d}", s.file_browser.field.text.trim()),
            &why,
        ));
    } else {
        c = c.child(
            col().key("find-list").w_full().items_stretch().opacity(busy).children(found.files.iter().map(|file| group(s, t, file))),
        );
        if found.stopped == Some(Stopped::Limit) {
            c = c.child(warn(
                t,
                "find-limit",
                &format!("Showing the first {} matches.", found.matches),
                "There are more. Search for something longer, or turn on the regex switch.",
            ));
        } else if found.stopped == Some(Stopped::Deadline) {
            c = c.child(warn(t, "find-deadline", "Stopped after 10 seconds.", "These are the matches found so far."));
        }
        if found.engine == Engine::Walk {
            c = c.child(
                col()
                    .key("find-walk")
                    .w_full()
                    .mx(2.0)
                    .my(8.0)
                    .px(12.0)
                    .py(9.0)
                    .radius(RADIUS_CARD)
                    .fill(t.card_bg)
                    .border(1.0, t.border)
                    .items_stretch()
                    .child(text(WALK.to_string()).font(sans(12.5, 400.0, 1.45)).color(t.muted).wrap()),
            );
        }
    }
    c
}

/// `.file-browser-panel__find-hint`: under the tree while the query is too short to search.
pub fn hint(s: &State, t: &Theme) -> El {
    let short = !FindText::is_active(&s.file_browser.field.text);
    let mut c = col().key("find-hint").w_full().px(16.0).py(18.0).items_center();
    if short {
        c = c.child(
            text("Type to search inside every file in this folder.")
                .font(sans(12.5, 400.0, 1.5))
                .color(t.muted)
                .wrap()
                .align(rune::Align::Center),
        );
    }
    c.child(
        text("Ignored files and build folders are left out.").font(sans(11.5, 400.0, 1.5)).color(t.muted).wrap().align(rune::Align::Center),
    )
}

/// `.file-browser-panel__toggle`: Aa and .* in the field.
pub fn toggle(t: &Theme, key: &'static str, label: &'static str, glyph: &'static str, on: bool) -> El {
    let b = el()
        .key(key)
        .role("button")
        .label(label)
        .help(label)
        .h(22.0)
        .min_w(24.0)
        .px(5.0)
        .center()
        .radius(5.0)
        .cursor(Cursor::Pointer)
        .focus_ring(2.0, -2.0, t.accent);
    let b = if on { b.fill(t.accent_dim).border(1.0, t.accent.alpha(0.45)) } else { b.hover_group() };
    b.child(text(glyph).font(mono(11.0, 600.0, 1.5)).color(if on { t.text } else { t.muted }).group_color(t.text))
}
