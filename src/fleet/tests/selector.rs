//! The selector dropdown driven with no window (docs/parity/05-composer.md, piece E): keys and clicks by element key,
//! state read back. Ports the behaviours of `SelectorDropdown.test.ts`, `SelectorDropdownPlacement.test.ts` and
//! `picker-screen-reader.test.ts`; the rules they rest on are tested in `composer/tests_selector.rs`.
use super::reconnect::app;
use crate::State;
use crate::composer::groups::{ModelPicker, PREFERENCE_KEY, model_picker_key};
use crate::mock::Status;
use crate::ui::Open;
use rune::{App, Env, Key, KeyEvent, Mods, Runtime};

const FRAME: f64 = 1.0 / 60.0;

/// Runs the menu's entrance (120 ms of fade and scale) to its end, so rows are where they will stay.
fn settle(rt: &mut Runtime<State>) {
    for _ in 0..30 {
        rt.frame(FRAME);
    }
}

fn click(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element keyed {key:?}"));
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(rt);
}

fn hover(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element keyed {key:?}"));
    rt.pointer_move(x, y);
    settle(rt);
}

fn press(rt: &mut Runtime<State>, key: Key) {
    rt.key(KeyEvent { key, mods: Mods::default() });
    rt.frame(FRAME);
    rt.frame(FRAME);
}

fn type_text(rt: &mut Runtime<State>, text: &str) {
    press(rt, Key::Text(text.into()));
}

fn titles(rt: &Runtime<State>, name: &str) -> Vec<String> {
    rt.state.selector_shown(name).iter().filter_map(|s| s.title.clone()).collect()
}

fn rect(rt: &Runtime<State>, key: &str) -> [f32; 4] {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).unwrap_or_else(|| panic!("no element keyed {key:?}")).rect
}

fn star(rt: &mut Runtime<State>, model: &str) {
    let picker = ModelPicker { favourites: vec![model_picker_key("opencode", "anthropic", model)], ..Default::default() };
    rt.update(move |s, _| {
        s.pickers.prefs.set(PREFERENCE_KEY, &picker.to_json());
    });
    rt.frame(FRAME);
}

fn row_count(rt: &Runtime<State>, name: &str) -> usize {
    rt.state.selector_shown(name).iter().map(|s| s.rows.len()).sum()
}

fn model_key(rt: &Runtime<State>, id: &str) -> String {
    rt.state.lists().models.iter().find(|m| m.id == id).unwrap().selection_key.clone()
}

// selector dropdown: "shows the starred model under Favourites, then All models"
#[test]
fn shows_the_starred_model_under_favourites_then_all_models() {
    let mut rt = app();
    star(&mut rt, "opus-5-5");
    click(&mut rt, "model");
    assert_eq!(titles(&rt, "model"), ["Favourites", "All models"]);
    // Default, then the favourite, then Default's list again: the favourite is in both runs and filled in both.
    let shown = rt.state.selector_shown("model");
    let starred = shown.iter().flat_map(|s| &s.rows).filter(|r| r.item.starred == Some(true)).count();
    assert_eq!(starred, 2);
}

// selector dropdown: "flattens while typing" (and Enter takes the first match, not Default)
#[test]
fn flattens_while_typing_and_enter_takes_the_first_match() {
    let mut rt = app();
    star(&mut rt, "opus-5-5");
    click(&mut rt, "model");
    type_text(&mut rt, "pea");
    assert!(titles(&rt, "model").is_empty(), "no headings while a filter is typed");
    let ids: Vec<String> = rt.state.selector_shown("model")[0].rows.iter().map(|r| r.item.id.clone()).collect();
    assert_eq!(ids.len(), 2, "Default stays, and the one match");
    press(&mut rt, Key::Enter);
    assert_eq!(rt.state.choice().model, ids[1]);
    assert_eq!(rt.state.ui.open, None);
}

// selector dropdown: "picks the active row with the arrows and Enter"
#[test]
fn picks_the_active_row_with_the_arrows_and_enter() {
    let mut rt = app();
    click(&mut rt, "model");
    assert_eq!(rt.state.ui.sel.active, 0, "the highlight opens on the selected row, Default");
    press(&mut rt, Key::Down);
    press(&mut rt, Key::Down);
    press(&mut rt, Key::Enter);
    assert_eq!(rt.state.choice().model, rt.state.lists().models[1].selection_key);
    assert_eq!(rt.state.ui.open, None);
}

// row 3.12: the highlight stops at the ends, and opens on the selected row
#[test]
fn the_highlight_opens_on_the_selected_row_and_stops_at_the_ends() {
    let mut rt = app();
    let key = model_key(&rt, "opus-5-5");
    rt.update(move |s, tx| s.pick_model(&key, tx));
    click(&mut rt, "model");
    let n = row_count(&rt, "model");
    assert_eq!(rt.state.ui.sel.active, 2, "Default, Sonnet, then Opus");
    for _ in 0..n + 3 {
        press(&mut rt, Key::Down);
    }
    assert_eq!(rt.state.ui.sel.active, n - 1);
    for _ in 0..n + 3 {
        press(&mut rt, Key::Up);
    }
    assert_eq!(rt.state.ui.sel.active, 0);
}

// selector dropdown: "stars a model without picking it or closing"
#[test]
fn stars_a_model_without_picking_it_or_closing() {
    let mut rt = app();
    click(&mut rt, "model");
    hover(&mut rt, "model-pop-item1");
    assert_eq!(rt.state.ui.sel.active, 1, "the pointer moves the highlight");
    click(&mut rt, "model-star1");
    let picker = ModelPicker::parse(rt.state.pickers.prefs.get(PREFERENCE_KEY).unwrap());
    assert_eq!(picker.favourites, [model_picker_key("opencode", "anthropic", "sonnet-5-5")]);
    assert_eq!(rt.state.choice().model, "", "nothing was picked");
    assert_eq!(rt.state.ui.open, Some(Open::Menu("model")), "the menu stays open");
    // A second click takes the star off again.
    click(&mut rt, "model-star1");
    assert!(ModelPicker::parse(rt.state.pickers.prefs.get(PREFERENCE_KEY).unwrap()).favourites.is_empty());
}

// row 3.9: a star that isn't starred is shown only on the highlighted row
#[test]
fn a_star_shows_only_on_the_highlighted_row_or_when_starred() {
    let mut rt = app();
    star(&mut rt, "haiku-5-5");
    click(&mut rt, "model");
    // Default, Favourites (Haiku), All models (Sonnet is the third row).
    // The menu has settled; the opacity is the star's own, to a thousandth.
    let shown = |rt: &Runtime<State>, key: &str| {
        rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).map(|e| (e.opacity * 100.0).round() / 100.0)
    };
    assert_eq!(shown(&rt, "model-star2"), Some(0.0), "Sonnet is not starred and not highlighted");
    assert_eq!(shown(&rt, "model-star1"), Some(1.0), "Haiku is starred, so its star always shows");
    hover(&mut rt, "model-pop-item2");
    assert_eq!(shown(&rt, "model-star2"), Some(1.0));
}

// the agent list has no stars, headings or notes
#[test]
fn the_agent_list_has_no_stars_or_headings() {
    let mut rt = app();
    click(&mut rt, "agent");
    assert!(titles(&rt, "agent").is_empty());
    assert!(rt.find("agent-star1").is_none());
    assert_eq!(row_count(&rt, "agent"), 4, "Default and the three agents");
}

// row 3.12: closing clears the filter; picking closes
#[test]
fn closing_clears_the_filter_and_the_next_opening_starts_empty() {
    let mut rt = app();
    click(&mut rt, "agent");
    type_text(&mut rt, "pla");
    assert_eq!(rt.state.ui.sel.filter.text, "pla");
    press(&mut rt, Key::Escape);
    assert_eq!(rt.state.ui.open, None);
    click(&mut rt, "agent");
    assert_eq!(rt.state.ui.sel.filter.text, "");
    assert_eq!(row_count(&rt, "agent"), 4);
}

// row 3.12: click toggles, and a click outside closes
#[test]
fn a_click_on_the_chip_or_outside_closes_the_menu() {
    let mut rt = app();
    click(&mut rt, "model");
    assert_eq!(rt.state.ui.open, Some(Open::Menu("model")));
    click(&mut rt, "model");
    assert_eq!(rt.state.ui.open, None, "the second click on the chip closes it");
    click(&mut rt, "agent");
    rt.pointer_move(5.0, 5.0);
    rt.pointer_down(5.0, 5.0);
    rt.pointer_up(5.0, 5.0);
    rt.frame(FRAME);
    assert_eq!(rt.state.ui.open, None);
}

// SelectorDropdown Escape: "leaves Escape alone while closed, so a dialog around it can take it"
#[test]
fn leaves_escape_alone_while_closed() {
    let mut rt = app();
    assert!(rt.state.session().status == Status::Working);
    press(&mut rt, Key::Escape);
    assert!(rt.state.session().status == Status::Idle, "Escape went on to interrupt");
}

// SelectorDropdown Escape: "takes Escape and closes while open"
#[test]
fn takes_escape_and_closes_while_open() {
    let mut rt = app();
    click(&mut rt, "model");
    press(&mut rt, Key::Escape);
    assert_eq!(rt.state.ui.open, None);
    assert!(rt.state.session().status == Status::Working, "the open menu took it; nothing was interrupted");
}

// picker-screen-reader: "keeps the filter out of the listbox and points it at the highlighted option"
#[test]
fn the_filter_keeps_focus_while_the_highlight_moves() {
    let mut rt = app();
    click(&mut rt, "model");
    let focused = |rt: &Runtime<State>| rt.elements().into_iter().find(|e| e.focused).and_then(|e| e.key);
    assert_eq!(focused(&rt).as_deref(), Some("model-filter"), "the filter is focused on open");
    press(&mut rt, Key::Down);
    assert_eq!(rt.state.ui.sel.active, 1);
    assert_eq!(focused(&rt).as_deref(), Some("model-filter"), "the highlight moved, the focus did not");
    type_text(&mut rt, "o");
    assert_eq!(rt.state.ui.sel.filter.text, "o", "what is typed still lands in the filter");
}

// row 3.3 and 3.4: the menu is 380 wide above its chip; in a window narrower than 396 it shrinks to the window less 16
#[test]
fn the_menu_is_380_wide_above_its_chip_and_shrinks_in_a_narrow_window() {
    let mut rt = app();
    click(&mut rt, "model");
    let [x, y, w, h] = rect(&rt, "model-pop-menu");
    let [_, cy, ..] = rect(&rt, "model");
    assert!((w - 380.0).abs() < 1.0, "width {w}");
    assert!(y + h <= cy, "above the chip");
    assert!(x >= 8.0 - 0.5, "8 px inside the window");

    let mut narrow = App::new(crate::spec()).headless_runtime((360.0, 700.0), Env::new(true, 1.0));
    narrow.frame(0.0);
    narrow.frame(FRAME);
    narrow.update(|s, _| s.open_selector("model"));
    settle(&mut narrow);
    let [x, _, w, _] = rect(&narrow, "model-pop-menu");
    assert!(w <= 360.0 - 16.0 + 0.5, "width {w}");
    assert!(x >= 7.5 && x + w <= 360.0 - 7.5, "inside the window with 8 px to spare: {x} {w}");
}

// row 3.10: "N more" counts the rows below the visible part and goes when the list is short
#[test]
fn a_long_list_says_how_many_rows_run_on_below() {
    let mut rt = app();
    rt.update(|s, _| s.pickers.add_mock_models(15));
    click(&mut rt, "model");
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(rt.find("selector-more").is_some(), "{} rows, the first screenful cut off", rt.state.ui.sel.rows.len());
    let first = rt.state.ui.sel.more_below(row_count(&rt, "model"));
    assert!(first > 5, "{first}");
    type_text(&mut rt, "model 1");
    rt.frame(FRAME);
    assert!(rt.state.ui.sel.more_below(row_count(&rt, "model")) < first);
}
