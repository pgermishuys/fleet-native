//! A page shown in the conversation, as a card and full size (component 14, piece F): `ConversationPage.test.ts`'s cases
//! (the frame that grows to the page's height is not ported: decision 2) and what the card needs that Fleet's frame doesn't.
use crate::tests::{FRAME, app, click};
use crate::web::{Wanted, wanted};
use rune::{ElementInfo, Key, KeyEvent, Mods, Runtime};

fn cards() -> Runtime<crate::State> {
    let mut rt = app();
    rt.update(|s, _| s.show_page_cards(true));
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

fn info(rt: &Runtime<crate::State>, key: &str) -> Option<ElementInfo> {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting)
}

fn text_of(rt: &Runtime<crate::State>, key: &str) -> String {
    info(rt, key).and_then(|e| e.text).unwrap_or_else(|| panic!("no text keyed {key:?}"))
}

fn press(rt: &mut Runtime<crate::State>, key: Key) {
    rt.key(KeyEvent { key, mods: Mods::default() });
    rt.frame(FRAME);
    rt.frame(FRAME);
}

#[test]
fn shows_the_page_under_the_calls_outside_their_box_named_with_its_title_and_file() {
    let rt = cards();
    let call = info(&rt, "tool-p3").or_else(|| info(&rt, "tool-row-p3")).map(|e| e.rect[1]);
    let card = info(&rt, "page-card-p3").expect("the page's card").rect;
    if let Some(call) = call {
        assert!(card[1] > call, "the card is under its call");
    }
    assert_eq!(text_of(&rt, "page-card-p3-file"), "flaky.html");
    assert!(text_of(&rt, "page-card-p3-title").starts_with("Flaky tests, ranked"));
    assert_eq!(card[3], 72.0);
}

#[test]
fn draws_a_page_shown_again_in_the_same_turn_once_at_its_last_show() {
    let rt = cards();
    assert!(info(&rt, "page-card-p1").is_none(), "the draft is replaced by the fix");
    assert!(info(&rt, "page-card-p2").is_some());
    assert!(info(&rt, "page-card-p3").is_some(), "another turn keeps its page");
}

#[test]
fn leaves_calls_without_a_page_as_they_were() {
    let mut rt = app();
    rt.update(|s, _| {
        s.dark = Some(true);
        s.show_conversation(crate::mock::visuals::of_visuals());
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(rt.elements().iter().all(|e| !e.key.as_deref().is_some_and(|k| k.starts_with("page-card"))));
}

#[test]
fn opens_full_size_over_the_app_with_focus_on_close_and_escape_puts_it_away() {
    let mut rt = cards();
    rt.update(|_, tx| tx.focus("page-card-p2-expand", false));
    rt.frame(FRAME);
    click(&mut rt, "page-card-p2-expand");
    rt.frame(FRAME);
    assert!(info(&rt, "page-full").is_some(), "the dialog is up");
    assert_eq!(text_of(&rt, "page-full-title"), "Test times by day");
    assert!(info(&rt, "page-full-close").is_some_and(|e| e.focused), "focus is on Close");
    // The sheet is as big as the window allows, less the scrim's 24 px.
    let sheet = info(&rt, "page-full").unwrap().rect;
    assert!(sheet[2] <= 1200.0 && sheet[3] <= 900.0 && sheet[2] >= 1000.0);

    press(&mut rt, Key::Escape);
    assert!(info(&rt, "page-full").is_none(), "Escape puts it away");
    assert!(rt.state.web.full.is_none());
    assert!(info(&rt, "page-card-p2-expand").is_some_and(|e| e.focused), "focus is back where it was");
}

#[test]
fn close_and_the_scrim_put_it_away_too() {
    let mut rt = cards();
    click(&mut rt, "page-card-p2-expand");
    click(&mut rt, "page-full-close");
    rt.frame(FRAME);
    assert!(info(&rt, "page-full").is_none());
    click(&mut rt, "page-card-p2-expand");
    rt.pointer_move(5.0, 5.0);
    rt.pointer_down(5.0, 5.0);
    rt.pointer_up(5.0, 5.0);
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(info(&rt, "page-full").is_none(), "a press on the scrim closes it");
}

#[test]
fn headless_full_size_says_the_page_cant_be_shown_here_and_offers_the_browser() {
    let mut rt = cards();
    click(&mut rt, "page-card-p2-expand");
    rt.frame(FRAME);
    assert!(info(&rt, "page-full-fallback").is_some());
    assert!(info(&rt, "page-full-open-browser").is_some());
}

#[test]
fn full_size_is_the_one_web_view_wanted_in_the_place_conversation_and_goes_with_the_dialog() {
    let mut rt = cards();
    click(&mut rt, "page-card-p2-expand");
    // A window that can host views, asked before any frame draws (there is no window here to make one in).
    rt.state.web.available = true;
    assert_eq!(wanted(&rt.state), Wanted::Nothing, "the dialog's body has not been placed yet");
    rt.state.web.full_rect = Some([100.0, 80.0, 1000.0, 560.0]);
    let Wanted::Shown(p) = wanted(&rt.state) else { panic!("the dialog's page is shown: {:?}", wanted(&rt.state)) };
    assert_eq!(p.url, "http://127.0.0.1:6411/pages/pg_0123456789abcdef0123456789abcde0/times.html");
    assert_eq!(p.rect, [100.0, 80.0, 1000.0, 560.0]);
    assert_eq!(p.place, crate::web::page_theme::PagePlace::Conversation);
    assert!(rt.state.page_theme().frame_name(p.place).contains("\"place\":\"conversation\""));
    rt.state.web.available = false;
    press(&mut rt, Key::Escape);
    rt.state.web.available = true;
    assert_eq!(wanted(&rt.state), Wanted::Nothing);
}

// ConversationPage.test.ts "offers to open the page in a new tab, and does not when it is contained": the page's own card
// has the button; the contained half is in `tests_mods_page.rs` (a mod's page is the first caller that passes `contained`).
#[test]
fn offers_to_open_the_page_in_a_new_tab() {
    let rt = cards();
    assert!(info(&rt, "page-card-p3-open").is_some(), "the card has Open in a new tab");
    assert!(info(&rt, "page-card-p3-expand").is_some(), "and Expand");
}
