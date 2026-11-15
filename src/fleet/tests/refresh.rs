//! When the session list is fetched again (checklist rows 7.2 and 7.3).
use crate::fleet::refresh::{DEBOUNCE, POLL, Refresh};
use std::time::{Duration, Instant};

#[test]
fn the_list_refetch_is_debounced() {
    let t0 = Instant::now();
    let mut r = Refresh::new(t0);
    assert!(!r.due(t0));
    r.event(t0);
    // More events inside the wait don't push the fetch back, and don't make a second one.
    r.event(t0 + Duration::from_millis(300));
    assert!(!r.due(t0 + Duration::from_millis(399)));
    assert!(r.due(t0 + DEBOUNCE));
    assert!(!r.due(t0 + DEBOUNCE + Duration::from_millis(1)));
}

#[test]
fn the_list_is_polled_every_15_seconds_while_the_window_is_visible() {
    let t0 = Instant::now();
    let mut r = Refresh::new(t0);
    assert!(!r.due(t0 + POLL - Duration::from_millis(1)));
    assert!(r.due(t0 + POLL));
    assert!(!r.due(t0 + POLL + Duration::from_secs(14)));
    assert!(r.due(t0 + POLL * 2));
}

#[test]
fn a_hidden_window_is_not_polled_and_fetches_when_it_comes_back() {
    let t0 = Instant::now();
    let mut r = Refresh::new(t0);
    r.set_visible(false, t0);
    assert!(!r.due(t0 + POLL * 4));
    r.set_visible(true, t0 + POLL * 4);
    assert!(r.due(t0 + POLL * 4));
    assert!(!r.due(t0 + POLL * 4 + Duration::from_secs(1)));
}

#[test]
fn the_window_regaining_focus_fetches_the_list_at_once() {
    let t0 = Instant::now();
    let mut r = Refresh::new(t0);
    r.window_back();
    assert!(r.due(t0 + Duration::from_millis(1)));
    assert!(!r.due(t0 + Duration::from_millis(2)));
}
