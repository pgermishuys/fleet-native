//! `stores/__tests__/session-selection.test.ts`, under its names.
use crate::fleet::selection::Selection;

fn sorted(s: &Selection) -> Vec<String> {
    let mut v = s.ids().to_vec();
    v.sort();
    v
}

fn order(ids: &[&str]) -> Vec<String> {
    ids.iter().map(|s| s.to_string()).collect()
}

#[test]
fn toggles_single_rows() {
    let mut s = Selection::default();
    s.toggle("a", None);
    s.toggle("b", None);
    s.toggle("a", None);
    assert_eq!(s.ids(), ["b"]);
    assert!(s.is_selecting());
}

#[test]
fn takes_a_shift_range_from_the_last_picked_row_in_list_order() {
    let mut s = Selection::default();
    s.set_visible_order(order(&["a", "b", "c", "d", "e"]), None);
    s.toggle("b", None);
    s.extend_to("d", None, None);
    assert_eq!(sorted(&s), ["b", "c", "d"]);
}

#[test]
fn starts_a_shift_range_from_the_open_session_when_nothing_is_picked_yet() {
    let mut s = Selection::default();
    s.set_visible_order(order(&["a", "b", "c", "d"]), None);
    s.extend_to("c", Some("a"), None);
    assert_eq!(sorted(&s), ["a", "b", "c"]);
}

#[test]
fn drops_rows_that_leave_the_list() {
    let mut s = Selection::default();
    s.set_visible_order(order(&["a", "b", "c"]), None);
    s.toggle("a", None);
    s.toggle("c", None);
    assert!(s.set_visible_order(order(&["b", "c"]), None));
    assert_eq!(s.ids(), ["c"]);
}

#[test]
fn clears() {
    let mut s = Selection::default();
    s.toggle("a", None);
    s.clear();
    assert_eq!(s.count(), 0);
    assert!(!s.is_selecting());
}

#[test]
fn a_range_up_the_list_and_an_unknown_end_fall_back_to_a_toggle() {
    let mut s = Selection::default();
    s.set_visible_order(order(&["a", "b", "c", "d"]), None);
    s.toggle("d", None);
    s.extend_to("b", None, None);
    assert_eq!(sorted(&s), ["b", "c", "d"]);
    s.extend_to("zzz", None, None);
    assert!(s.is_selected("zzz"));
}

#[test]
fn a_row_on_another_machine_starts_again_from_it() {
    let mut s = Selection::default();
    s.toggle("a", None);
    s.toggle("m1", Some("hangar"));
    assert_eq!(s.ids(), ["m1"]);
    assert_eq!(s.machine(), Some("hangar"));
    assert!(s.is_selecting_on(Some("hangar")));
    assert!(!s.is_selecting_on(None));
    // A range on another machine than the picks starts at the clicked row.
    s.extend_to("a", None, None);
    assert_eq!(s.ids(), ["a"]);
    assert_eq!(s.machine(), None);
}
