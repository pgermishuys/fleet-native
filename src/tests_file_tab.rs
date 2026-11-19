//! The file tab drawn (component 11, piece G): `FileCanvas.test.ts` "opens the file in the editor, clean", "says why a file too large
//! to edit can't be opened" and "says why a binary file can't be opened", on the invented `src/paginate.py` the scenes draw (no
//! server), and the rows the viewer builds for a long file. The rules under it are in `src/fleet/tests/file_buffers.rs`.
use crate::State;
use crate::fleet::file_buffers::{Kind, Request};
use crate::mock::file_tab::{Case, PATH};
use crate::tests::{FRAME, app, click};
use rune::Runtime;

fn open(case: Case) -> Runtime<State> {
    let mut rt = app();
    rt.update(move |s, _| {
        s.sheet_w = 1400.0;
        s.show_file(case, true)
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

fn has(rt: &Runtime<State>, key: &str) -> bool {
    rt.find(key).is_some()
}

fn el(rt: &Runtime<State>, key: &str) -> rune::ElementInfo {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).unwrap_or_else(|| panic!("no element {key}"))
}

fn said(rt: &Runtime<State>, key: &str) -> String {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).and_then(|e| e.text).unwrap_or_default()
}

fn lines_built(rt: &Runtime<State>) -> usize {
    rt.elements().iter().filter(|e| e.key.as_deref().is_some_and(|k| k.starts_with("code-line-"))).count()
}

// FileCanvas.test.ts
#[test]
fn opens_the_file_in_the_viewer_clean() {
    let rt = open(Case::Code);
    assert_eq!(said(&rt, "code-text-1"), "\"\"\"Splits a list of rows into pages.\"\"\"");
    assert_eq!(said(&rt, "code-text-3"), "DEFAULT_SIZE = 20");
    assert_eq!(said(&rt, "file-crumb-name"), "paginate.py");
    assert_eq!(said(&rt, "file-crumb-0"), "src", "the folders come before the name");
    assert!(!has(&rt, "file-opening") && !has(&rt, "file-unavailable"), "no note over a file that is there");
    // The viewer has no state to show: nothing is unsaved, nothing changed on disk (those are slice 2's).
    assert!(!rt.elements().iter().any(|e| e.text.as_deref().is_some_and(|t| t == "Unsaved" || t == "Changed on disk")));
}

#[test]
fn says_the_file_is_opening_while_it_is_read() {
    let rt = open(Case::Opening);
    assert_eq!(said(&rt, "file-opening"), "Opening src/paginate.py\u{2026}");
    assert!(!has(&rt, "file-view-switch"), "the view switch comes once the file is ready");
    assert!(!has(&rt, "file-code"));
}

// FileCanvas.test.ts
#[test]
fn says_why_a_file_too_large_to_edit_can_t_be_opened() {
    let rt = open(Case::TooLarge);
    assert_eq!(said(&rt, "file-unavailable"), "Too large to edit here (over 512 KB).");
    assert!(!has(&rt, "file-view-edit"));
    assert!(!has(&rt, "file-code"));
}

// FileCanvas.test.ts
#[test]
fn says_why_a_binary_file_can_t_be_opened() {
    let rt = open(Case::Binary);
    assert!(said(&rt, "file-unavailable").contains("binary or not UTF-8"));
    assert!(!has(&rt, "file-view-switch"));
}

#[test]
fn says_when_the_file_is_not_in_the_folder_any_more() {
    let rt = open(Case::Missing);
    assert_eq!(said(&rt, "file-unavailable"), "This file isn't in the session's folder any more.");
}

#[test]
fn the_view_switch_offers_edit_and_a_diff_only_a_changed_file_has() {
    let mut rt = open(Case::Code);
    assert!(has(&rt, "file-view-edit") && has(&rt, "file-view-diff"));
    assert!(!has(&rt, "file-view-rendered"), "a Python file has no Rendered view");
    assert_eq!(el(&rt, "file-view-diff").help.as_deref(), Some("Diff against the last commit"));
    click(&mut rt, "file-view-diff");
    rt.frame(FRAME);
    assert_eq!(rt.state.open_canvases().active().unwrap().file.as_ref().unwrap().view, crate::fleet::canvases::FileView::Diff);
    assert!(has(&rt, "file-code"), "the code shows for Diff too");
}

#[test]
fn a_file_with_no_changes_has_the_diff_button_off_and_shows_edit_when_asked_for_diff() {
    let mut rt = open(Case::Code);
    rt.update(|s, _| {
        let id = s.canvas_session();
        s.diffs.reset(&id);
        s.set_active_file_view(PATH, crate::fleet::canvases::FileView::Diff);
    });
    rt.frame(FRAME);
    let diff = el(&rt, "file-view-diff");
    assert_eq!(diff.help.as_deref(), Some("No changes in this file"));
    assert!(!diff.focusable, "off: Tab skips it");
    assert_eq!(crate::view::file_tab::shown_view(crate::fleet::canvases::FileView::Diff, false), crate::fleet::canvases::FileView::Edit);
}

#[test]
fn markdown_has_rendered_and_names_edit_source() {
    let rt = open(Case::Markdown);
    assert!(has(&rt, "file-view-rendered"));
    assert!(has(&rt, "file-view-edit"));
    assert_eq!(el(&rt, "file-view-edit").label.as_deref(), Some("Source"));
}

#[test]
fn a_narrow_panel_shows_the_file_name_and_not_the_folders() {
    let mut rt = open(Case::Code);
    assert!(has(&rt, "file-crumb-0"));
    rt.update(|s, _| {
        s.right.widened = false;
        s.right.width = 360.0;
    });
    rt.frame(FRAME);
    assert!(!has(&rt, "file-crumb-0"), "under 430 px the folders go");
    assert!(has(&rt, "file-crumb-name"));
    assert_eq!(el(&rt, "file-crumbs").help.as_deref(), Some(PATH), "the tooltip is the whole path");
}

// A 20,000-line file builds only the rows near the window.
#[test]
fn a_file_of_twenty_thousand_lines_builds_only_the_rows_on_screen() {
    let mut rt = open(Case::Code);
    let text: String = (1..=20_000).map(|n| format!("value_{n} = {n}\n")).collect();
    rt.update(move |s, _| {
        let id = s.canvas_session();
        let machine = s.machine_key_of(&id).to_string();
        let request = Request { machine, session: id, path: PATH.to_string(), kind: Kind::Refresh { unavailable: false } };
        let body = serde_json::json!({"content": text, "hash": "h2", "isBinary": false, "isTruncated": false}).to_string();
        s.file_buffers.finish(&request, Ok((200, body)));
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    let built = lines_built(&rt);
    assert!(built > 10, "the window has lines: {built}");
    assert!(built < 300, "and not twenty thousand of them: {built}");
}

/// Pressed: the file's choice is on, and the button shows it (the fill fades, so it is read after the frames settle).
fn pressed(rt: &mut Runtime<State>) -> bool {
    for _ in 0..30 {
        rt.frame(FRAME);
    }
    let on = rt.state.sidebar.file_wrap(crate::mock::file_tab::WRAP_PATH);
    assert_eq!(el(rt, "file-wrap").fill.a > 0.0, on, "the button shows the choice");
    on
}

fn alt_z() -> rune::KeyEvent {
    rune::KeyEvent { key: rune::Key::Text("z".into()), mods: rune::Mods { alt: true, ..rune::Mods::default() } }
}

// FileCanvas.test.ts: "wraps long lines by default; the Wrap button turns it off for that file, remembered"
#[test]
fn wraps_long_lines_by_default_and_the_wrap_button_turns_it_off_for_that_file_remembered() {
    let mut rt = open(Case::Wrap);
    assert!(pressed(&mut rt), "on by default");
    let wrapped = |rt: &Runtime<State>| el(rt, "code-text-17").rect[3];
    assert!(wrapped(&rt) >= 40.0, "line 17 takes several rows: {}", wrapped(&rt));
    assert!(!has(&rt, "code-line-18"), "the wrapped rows are keyed apart from the unwrapped ones");
    click(&mut rt, "file-wrap");
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(!pressed(&mut rt));
    assert_eq!(wrapped(&rt), 20.0, "one row again, scrolled sideways");
    // The file opened again, in a new buffer, starts the way it was left: the choice is in the settings file.
    let json = rt.state.sidebar.to_json();
    assert!(json.contains("fileWrapOff") && json.contains(crate::mock::file_tab::WRAP_PATH), "{json}");
    assert!(!crate::fleet::settings::Sidebar::parse(&json).file_wrap(crate::mock::file_tab::WRAP_PATH));
    assert!(crate::fleet::settings::Sidebar::parse(&json).file_wrap("src/other.py"), "only that file");
}

// FileCanvas.test.ts: "Alt Z toggles wrap in the editor"
#[test]
fn alt_z_toggles_wrap_in_the_code_view() {
    let mut rt = open(Case::Wrap);
    assert!(pressed(&mut rt));
    rt.key(alt_z());
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(!pressed(&mut rt));
    rt.key(alt_z());
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(pressed(&mut rt));
}

#[test]
fn the_button_has_its_tooltip_and_loses_its_word_in_a_narrow_panel() {
    let mut rt = open(Case::Wrap);
    let tip = el(&rt, "file-wrap").help.unwrap_or_default();
    assert!(tip.starts_with("Wrap long lines (") && tip.ends_with("Z)"), "{tip}");
    assert!(rt.elements().iter().any(|e| e.text.as_deref() == Some("Wrap")), "the word");
    rt.update(|s, _| {
        s.right.widened = false;
        s.right.width = 360.0;
    });
    rt.frame(FRAME);
    assert!(!rt.elements().iter().any(|e| e.text.as_deref() == Some("Wrap")), "under 430 px only the icon");
}

#[test]
fn a_wrapped_line_hangs_under_its_indent() {
    let rt = open(Case::Wrap);
    // Line 22 is indented by eight columns: its text starts after a column of that width and its rows start there too.
    let text = el(&rt, "code-text-22");
    let hang = el(&rt, "code-hang-22");
    assert_eq!(hang.rect[2], 8.0 * 7.5);
    assert!((text.rect[0] - (hang.rect[0] + hang.rect[2])).abs() < 0.5, "text {} hang {}", text.rect[0], hang.rect[0]);
    assert!(text.rect[3] > 20.0);
    assert_eq!(crate::view::code_view::hang_of(&" ".repeat(40)), 22, "at most 22 columns");
}
