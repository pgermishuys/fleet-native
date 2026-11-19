//! File links in messages (component 11, piece M): the 4 cases of `MessageBubble.file-links.test.ts`, "opens the file, keeping the
//! tab on Ctrl-click" of `MessageBubble.attached-files.test.ts`, the open-at-a-line cases of `FileViewer.test.ts`, and the
//! resolve request. The scene's reply names `src/paginate.py:24`, `src/ledger/page_size.py`, `src/ledger/legacy.py` (not a file),
//! `paginate()` (not a path) and links to `docs/pagination.md#L3`.
use crate::fleet::file_links::{self, FileLinks};
use crate::tests::{FRAME, app, click};
use crate::view::conversation::Message;
use rune::{Key, KeyEvent, Mods, PhysicalInput, Runtime};

fn scene() -> Runtime<crate::State> {
    let mut rt = app();
    rt.update(|s, _| s.show_message_file_links(true, false));
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

fn link(rt: &Runtime<crate::State>, destination: &str) -> bool {
    rt.elements().iter().any(|e| e.key.as_deref() == Some(destination) && e.role == "link")
}

fn active_file(rt: &mut Runtime<crate::State>) -> (String, bool) {
    let tabs = rt.state.open_canvases();
    let file = tabs.active().and_then(|c| c.file.clone()).expect("a file tab is active");
    (file.path, file.preview)
}

fn click_with(rt: &mut Runtime<crate::State>, key: &str, mods: Mods) {
    rt.physical_input(PhysicalInput::Modifiers(mods));
    click(rt, key);
    rt.physical_input(PhysicalInput::Modifiers(Mods::default()));
    rt.frame(FRAME);
}

#[test]
fn links_the_paths_that_are_files_in_the_sessions_folder_and_leaves_the_rest_as_code() {
    let rt = scene();
    assert!(link(&rt, "fleet-file:src/paginate.py#L24"));
    assert!(link(&rt, "fleet-file:src/ledger/page_size.py"));
    assert!(!rt.elements().iter().any(|e| e.key.as_deref().is_some_and(|k| k.contains("legacy"))), "a path that isn't a file stays code");
    assert!(!rt.elements().iter().any(|e| e.key.as_deref().is_some_and(|k| k.contains("paginate()"))));
    let found = rt.state.file_links.found(&rt.state.canvas_session());
    assert_eq!(found.get("src/paginate.py").map(String::as_str), Some("src/paginate.py"));
    assert!(!found.contains_key("src/ledger/legacy.py"));
    assert_eq!(
        file_links::help("fleet-file:src/paginate.py#L24").as_deref(),
        Some(if cfg!(target_os = "macos") {
            "Open src/paginate.py at line 24 · ⌘-click keeps the tab"
        } else {
            "Open src/paginate.py at line 24 · Ctrl-click keeps the tab"
        })
    );
    assert_eq!(file_links::help("https://example.com").as_deref(), None);
}

#[test]
fn opens_a_preview_tab_at_the_line_on_a_click_and_keeps_the_tab_on_a_ctrl_click() {
    let mut rt = scene();
    click(&mut rt, "fleet-file:src/paginate.py#L24");
    assert_eq!(active_file(&mut rt), ("src/paginate.py".to_string(), true), "a click is a preview");
    let pending = rt.state.go_to_file.focus_on_open.clone().expect("the line is asked for");
    assert_eq!((pending.path.as_str(), pending.line), ("src/paginate.py", Some(24)));
    assert!(rt.state.content_open, "the right panel shows");

    click(&mut rt, "fleet-file:src/ledger/page_size.py");
    assert_eq!(active_file(&mut rt), ("src/ledger/page_size.py".to_string(), true));
    assert!(rt.state.go_to_file.focus_on_open.as_ref().is_some_and(|p| p.line == Some(24)), "no line, no new request");

    let ctrl = Mods { ctrl: true, cmd: cfg!(target_os = "macos"), ..Mods::default() };
    rt.update(|s, _| s.content_open = true);
    rt.frame(FRAME);
    click_with(&mut rt, "fleet-file:src/ledger/page_size.py", ctrl);
    assert_eq!(active_file(&mut rt), ("src/ledger/page_size.py".to_string(), false));
}

#[test]
fn shift_alt_or_a_web_address_open_no_file() {
    let mut rt = scene();
    let before = rt.state.open_canvases().canvases.len();
    click_with(&mut rt, "fleet-file:src/ledger/page_size.py", Mods { shift: true, ..Mods::default() });
    click_with(&mut rt, "fleet-file:src/ledger/page_size.py", Mods { alt: true, ..Mods::default() });
    assert_eq!(rt.state.open_canvases().canvases.len(), before);
}

#[test]
fn opens_from_the_keyboard_with_enter() {
    let mut rt = scene();
    // Links are Tab stops in reading order, the conversation before the composer: two steps back from it is the second one.
    rt.update(|_, tx| tx.focus("composer", true));
    rt.frame(FRAME);
    let mut focused = Vec::new();
    for _ in 0..2 {
        rt.key(KeyEvent { key: Key::Tab, mods: Mods { shift: true, ..Mods::default() } });
        rt.frame(FRAME);
        let mut now = None;
        rt.update(|_, tx| now = tx.focused().map(str::to_string));
        focused.push(now);
    }
    assert_eq!(focused[1].as_deref(), Some("fleet-file:src/ledger/page_size.py"), "{focused:?}");
    rt.key(KeyEvent { key: Key::Enter, mods: Mods::default() });
    rt.frame(FRAME);
    assert_eq!(active_file(&mut rt), ("src/ledger/page_size.py".to_string(), true));
}

#[test]
fn doesnt_open_when_the_click_ended_a_drag_that_selected_text() {
    let mut rt = scene();
    // The press began in the words before the link and ended on it (a drag that began on the link itself is Rune's click: docs/RUNE.md).
    let (x, y) = rt.find("fleet-file:src/ledger/page_size.py").unwrap();
    let before = rt.state.open_canvases().active_id;
    rt.pointer_move(430.0, y);
    rt.pointer_down(430.0, y);
    rt.pointer_move(x, y);
    rt.pointer_up(x, y);
    rt.frame(FRAME);
    assert_eq!(rt.state.open_canvases().active_id, before);
}

#[test]
fn a_markdown_link_to_a_file_opens_it_on_its_source_at_the_line() {
    let mut rt = scene();
    click(&mut rt, "docs/pagination.md#L3");
    let tabs = rt.state.open_canvases();
    let file = tabs.active().and_then(|c| c.file.clone()).unwrap();
    assert_eq!(file.path, "docs/pagination.md");
    assert_eq!(file.view, crate::fleet::canvases::FileView::Edit, "at a line Markdown opens on its source");
}

#[test]
fn opens_at_a_line_and_moves_an_open_file_to_another() {
    let mut rt = scene();
    rt.update(|s, tx| {
        let id = s.canvas_session();
        s.open_file_at(&id, "src/ledger/page_size.py", Some(9), false, tx);
        s.open_file_at(&id, "src/ledger/page_size.py", Some(3), false, tx);
    });
    let pending = rt.state.go_to_file.focus_on_open.clone().unwrap();
    assert_eq!((pending.path.as_str(), pending.line), ("src/ledger/page_size.py", Some(3)));
}

#[test]
fn an_attached_file_chip_opens_its_file_keeping_the_tab_on_a_ctrl_click() {
    let mut rt = scene();
    let note = "Look at this\n\nAttached: src/ledger/totals.py (1.2 KB)\nRead it as needed.";
    rt.update(move |s, _| {
        let (p, i) = s.open_at();
        s.projects[p].sessions[i].messages.push(Message::user("u2", note));
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    click(&mut rt, "attached-src/ledger/totals.py");
    assert_eq!(active_file(&mut rt), ("src/ledger/totals.py".to_string(), true));
    rt.update(|s, _| s.content_open = true);
    rt.frame(FRAME);
    click_with(&mut rt, "attached-src/ledger/totals.py", Mods { ctrl: true, cmd: cfg!(target_os = "macos"), ..Mods::default() });
    assert_eq!(active_file(&mut rt), ("src/ledger/totals.py".to_string(), false));
}

#[test]
fn asks_each_path_once_in_batches_and_again_after_a_turn_only_for_the_missing() {
    let mut links = FileLinks::default();
    let texts = [("a", "`src/a.ts` and `src/b.ts`"), ("b", "`src/a.ts` `lineTotal`")];
    let asked = links.wanted("s1", texts.iter().copied(), 0.0);
    assert_eq!(asked, ["src/a.ts", "src/b.ts"]);
    assert!(links.wanted("s1", texts.iter().copied(), 0.1).is_empty(), "being asked");
    links.answered("s1", &asked, &[("src/a.ts".to_string(), "src/a.ts".to_string())].into_iter().collect());
    links.turn("s1", true);
    links.turn("s1", false);
    let again = links.wanted("s1", texts.iter().copied(), 1.0);
    assert_eq!(again, ["src/b.ts"], "a file stays a link, what wasn't one is asked again");
}

#[test]
fn a_failed_request_asks_again_after_a_pause() {
    let mut links = FileLinks::default();
    let texts = [("a", "`src/a.ts`")];
    let asked = links.wanted("s1", texts.iter().copied(), 0.0);
    links.failed("s1", &asked, 0.0);
    assert!(links.wanted("s1", texts.iter().copied(), 1.0).is_empty());
    assert_eq!(links.wanted("s1", texts.iter().copied(), 6.0), ["src/a.ts"]);
}

#[test]
fn reads_the_resolve_answer() {
    let body = r#"{"files":[{"path":"/w/src/a.ts","relativePath":"src/a.ts"},{"path":"b.md","relativePath":"b.md"}]}"#;
    let files = file_links::parse_resolved(body);
    assert_eq!(files.get("/w/src/a.ts").map(String::as_str), Some("src/a.ts"));
    assert_eq!(files.len(), 2);
    assert!(file_links::parse_resolved("not json").is_empty());
}
