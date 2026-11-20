//! Component 24, piece I: the right-click menu on text (`desktop/src/menu.ts` `showContextMenu`, rows 24.110-24.113) and the link
//! rule (`desktop/test/desktop.test.ts` "links", row 24.122) against what the app hands `open_url`. The menu is drawn by the
//! app only where Rune has none (Linux); these tests turn that on so they run the same on every system. Nothing opens a browser:
//! a headless run keeps the requests.
use crate::State;
use crate::tests::{FRAME, app, click, press_key, typed};
use crate::ui::text_menu::{Params, Row, openable, own_menus, rows, set_own_menus};
use rune::{Key, Mods, Runtime};

fn menu_runtime() -> Runtime<State> {
    set_own_menus(true);
    assert!(own_menus());
    app()
}

fn open(rt: &mut Runtime<State>, params: Params, field: Option<&str>) {
    let field = field.map(str::to_owned);
    rt.update(move |s, _| s.open_text_menu(params, field, (400.0, 300.0)));
    rt.frame(FRAME);
    rt.frame(FRAME);
}

fn has(rt: &Runtime<State>, key: &str) -> bool {
    rt.find(key).is_some()
}

fn link(url: &str) -> Params {
    Params { link: Some(url.into()), ..Params::default() }
}

fn field(selected: &str) -> Params {
    let on = !selected.is_empty();
    Params { editable: true, can_cut: on, can_copy: on, can_paste: true, selection: selected.into(), link: None }
}

fn right_click(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element keyed {key:?}"));
    rt.pointer_move(x, y);
    rt.secondary_down(x, y);
    rt.secondary_up(x, y);
    rt.frame(FRAME);
    rt.frame(FRAME);
}

// ---- the rows, one case at a time ----------------------------------------------------------------------------------------------

#[test]
fn a_link_gets_open_link_in_browser_and_copy_link() {
    assert_eq!(rows(&link("https://example.test/a")), [Row::OpenLink, Row::CopyLink]);
    assert_eq!(rows(&link("mailto:ada@example.test")), [Row::OpenLink, Row::CopyLink]);
    assert_eq!(rows(&link("HTTP://EXAMPLE.TEST")), [Row::OpenLink, Row::CopyLink], "the scheme is matched without case");
}

#[test]
fn a_link_that_is_not_http_https_or_mailto_gets_no_link_rows() {
    for url in ["javascript:alert(1)", "file:///work/notes.txt", "vscode://open", "not a url", ""] {
        assert!(rows(&link(url)).is_empty(), "{url:?}");
    }
}

#[test]
fn a_field_gets_cut_copy_paste_each_enabled_by_what_it_can_do_then_select_all() {
    assert_eq!(rows(&field("")), [Row::Cut(false), Row::Copy(false), Row::Paste(true), Row::Separator, Row::SelectAll]);
    assert_eq!(rows(&field("some words")), [Row::Cut(true), Row::Copy(true), Row::Paste(true), Row::Separator, Row::SelectAll]);
}

#[test]
fn selected_text_in_a_page_gets_copy_alone() {
    let p = Params { selection: "two words".into(), ..Params::default() };
    assert_eq!(rows(&p), [Row::Copy(true)]);
}

#[test]
fn a_link_over_a_selection_gets_the_link_rows_a_separator_and_copy() {
    let p = Params { selection: "two words".into(), ..link("https://example.test/") };
    assert_eq!(rows(&p), [Row::OpenLink, Row::CopyLink, Row::Separator, Row::Copy(true)]);
}

#[test]
fn nothing_to_show_is_no_menu_and_no_trailing_separator() {
    assert!(rows(&Params::default()).is_empty());
    assert!(rows(&Params { selection: "  \n ".into(), ..Params::default() }).is_empty(), "blanks are not a selection");
    assert_eq!(rows(&link("https://example.test/")).last(), Some(&Row::CopyLink), "a link alone ends on Copy Link");
    for p in [field(""), field("x"), link("mailto:a@example.test")] {
        assert_ne!(rows(&p).last(), Some(&Row::Separator));
    }
    let mut rt = menu_runtime();
    open(&mut rt, Params::default(), None);
    assert!(rt.state.ui.open.is_none(), "no menu opens");
    assert!(!has(&rt, "text-menu"));
}

// ---- drawn and used --------------------------------------------------------------------------------------------------------------

#[test]
fn the_menu_over_a_link_has_its_two_rows_and_copy_link_copies_it() {
    let mut rt = menu_runtime();
    let clipboard = rune::clipboard::memory();
    rt.use_clipboard(&clipboard);
    open(&mut rt, link("https://example.test/docs"), None);
    assert!(has(&rt, "text-menu"));
    assert!(has(&rt, "text-menu-open-link") && has(&rt, "text-menu-copy-link"));
    assert!(!has(&rt, "text-menu-cut"), "a link is not a field");
    click(&mut rt, "text-menu-copy-link");
    rt.frame(FRAME);
    assert_eq!(clipboard.text().as_deref().map(String::as_str), Some("https://example.test/docs"));
    assert!(rt.state.ui.open.is_none(), "a pick closes the menu");
    assert!(rt.take_url_requests().is_empty(), "copying opens nothing");
}

#[test]
fn a_right_click_in_the_composer_opens_the_field_menu_and_select_all_then_copy_copies_the_text() {
    let mut rt = menu_runtime();
    let clipboard = rune::clipboard::memory();
    rt.use_clipboard(&clipboard);
    typed(&mut rt, "Ship the paging fix");
    right_click(&mut rt, "composer");
    assert!(has(&rt, "text-menu"), "the menu opened over the field");
    assert!(has(&rt, "text-menu-cut") && has(&rt, "text-menu-paste") && has(&rt, "text-menu-select-all"));
    click(&mut rt, "text-menu-select-all");
    rt.frame(FRAME);
    right_click(&mut rt, "composer");
    click(&mut rt, "text-menu-copy");
    rt.frame(FRAME);
    assert_eq!(clipboard.text().as_deref().map(String::as_str), Some("Ship the paging fix"));
    assert_eq!(rt.state.draft.text, "Ship the paging fix", "Copy leaves the text");
}

#[test]
fn cut_takes_the_selection_out_and_paste_leaves_the_field_focused() {
    let mut rt = menu_runtime();
    let clipboard = rune::clipboard::memory();
    rt.use_clipboard(&clipboard);
    typed(&mut rt, "alpha beta");
    press_key(&mut rt, Key::Command('a'), Mods::default());
    right_click(&mut rt, "composer");
    click(&mut rt, "text-menu-cut");
    rt.frame(FRAME);
    assert_eq!(rt.state.draft.text, "");
    assert_eq!(clipboard.text().as_deref().map(String::as_str), Some("alpha beta"));
    // Paste asks Rune to read the clipboard, which a headless run has no one to do: the pick closes the menu and the field still
    // has focus for the text to land in.
    right_click(&mut rt, "composer");
    click(&mut rt, "text-menu-paste");
    for _ in 0..4 {
        rt.frame(FRAME);
    }
    assert!(rt.state.ui.open.is_none());
    assert!(rt.elements().iter().any(|e| e.key.as_deref() == Some("composer") && e.focused), "the field kept focus");
}

#[test]
fn a_field_with_nothing_selected_has_cut_and_copy_off() {
    let mut rt = menu_runtime();
    typed(&mut rt, "alpha beta");
    press_key(&mut rt, Key::Right, Mods::default());
    right_click(&mut rt, "composer");
    assert!(has(&rt, "text-menu"));
    let off =
        |rt: &Runtime<State>, key: &str| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).is_some_and(|e| !e.focusable);
    assert!(off(&rt, "text-menu-cut") && off(&rt, "text-menu-copy"));
    assert!(!off(&rt, "text-menu-paste"));
}

#[test]
fn where_the_platform_draws_its_own_field_menu_the_app_draws_none() {
    set_own_menus(false);
    let mut rt = app();
    typed(&mut rt, "alpha beta");
    right_click(&mut rt, "composer");
    assert!(!has(&rt, "text-menu"));
    set_own_menus(true);
}

// ---- LNK: what the app hands open_url -----------------------------------------------------------------------------------------

// desktop.test.ts "links": "keeps Fleet's own pages in the window" is n/a: the native app draws them, there is no page to keep.

// "opens web and mail links in the browser"
#[test]
fn opens_web_and_mail_links_in_the_browser() {
    let mut rt = menu_runtime();
    let links = ["https://github.com/example/harbor/pull/1", "http://127.0.0.1:41000/", "mailto:someone@example.test"];
    for url in links {
        open(&mut rt, link(url), None);
        click(&mut rt, "text-menu-open-link");
        rt.frame(FRAME);
    }
    assert_eq!(rt.take_url_requests(), links);
}

// "blocks everything else"
#[test]
fn blocks_everything_else() {
    let mut rt = menu_runtime();
    for url in ["javascript:alert(1)", "file:///etc/passwd", "vscode://open", "not a url"] {
        assert!(!openable(url), "{url:?} is not a link the menu offers");
        // Nor can a handler get one through: the app's own `open_url` calls refuse it.
        let refused = std::cell::Cell::new(false);
        let seen = &refused;
        rt.update(move |_, tx| seen.set(tx.open_url(url).is_err()));
        rt.frame(FRAME);
        assert!(refused.get(), "{url:?} was refused");
    }
    assert!(rt.take_url_requests().is_empty(), "nothing reached the browser");
}
