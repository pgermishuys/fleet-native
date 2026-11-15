//! `ActivityStream.scroll.test.ts` and the scroll ones of `ActivityStream.commands.test.ts`, under their names. Fleet's
//! tests set a fake `scrollHeight`, `clientHeight` and `scrollTop` and dispatch wheel events; here the same numbers go
//! to `Pin` as `Edges`, and the wheel is `note_user_intent`. The ones that need the real box (the Jump to latest click,
//! the view staying put, showing a message) are Rune-driven tests in `src/view/scroll_tests.rs`.
use crate::conv::pin::{Edges, Pin, SHOWN_MS, Shown};

const VIEWPORT: f32 = 500.0;

/// A box of `height` points scrolled to `top`.
fn at(top: f32, height: f32) -> Edges {
    Edges { top, height, viewport: VIEWPORT }
}

/// The box as Fleet's tests open it: 2,000 tall, then scrolled to the bottom.
fn open() -> (Pin, f32) {
    let mut pin = Pin::default();
    pin.scrolled_to_bottom(at(1500.0, 2000.0));
    (pin, 2000.0)
}

#[test]
fn starts_at_the_bottom_without_the_jump_button() {
    let (pin, _) = open();
    assert!(pin.pinned());
    assert!(!pin.show_jump());
}

#[test]
fn offers_jump_to_latest_once_scrolled_up_past_the_threshold_and_scrolls_to_the_bottom_when_clicked() {
    let (mut pin, height) = open();
    // Within 80px of the bottom still counts as at the bottom.
    pin.update(at(1500.0 - 70.0, height), 0.0);
    assert!(!pin.show_jump());

    pin.update(at(1500.0 - 90.0, height), 0.0);
    assert!(pin.show_jump());

    pin.pin_to_bottom();
    pin.scrolled_to_bottom(at(1500.0, height));
    assert!(!pin.show_jump());
}

#[test]
fn leaves_the_view_where_it_is_when_scrolled_up_and_content_grows() {
    let (mut pin, _) = open();
    pin.update(at(600.0, 2000.0), 0.0);
    // The content grows 100px; the box is where it was, so it is further from the end.
    pin.update(at(600.0, 2100.0), 16.0);
    assert!(pin.show_jump());
    assert!(!pin.pinned());
}

#[test]
fn a_small_move_up_that_is_not_the_users_layout_shift_rounding_leaves_the_view_pinned() {
    let (mut pin, _) = open();
    pin.update(at(1500.0 - 40.0, 2000.0), 0.0);
    assert!(!pin.show_jump());
    // Content grows and the view is still pinned: it is to follow.
    pin.update(at(1500.0 - 40.0, 2100.0), 16.0);
    assert!(pin.pinned());
}

#[test]
fn a_small_scroll_up_by_wheel_stays_put_while_text_streams_in() {
    let (mut pin, _) = open();
    pin.note_user_intent(0.0);
    pin.update(at(1500.0 - 30.5, 2000.0), 10.0);
    assert!(pin.show_jump());

    pin.update(at(1500.0 - 30.5, 2050.0), 100.0);
    assert!(!pin.pinned(), "text arriving brought the view back");
}

#[test]
fn a_move_up_long_after_the_input_is_not_the_users() {
    let (mut pin, _) = open();
    pin.note_user_intent(0.0);
    pin.update(at(1500.0 - 30.0, 2000.0), 701.0);
    assert!(!pin.show_jump(), "an intent lasts 700 ms");
}

#[test]
fn a_wheel_up_inside_a_code_block_scrolling_on_its_own_is_not_leaving_the_bottom() {
    // In Rune a wheel over a block that scrolls on its own goes to the block: the stream's offset doesn't move, so no
    // move up reaches `Pin` and there is no intent to note. A small move up with no intent stays pinned.
    let (mut pin, _) = open();
    pin.update(at(1500.0 - 30.0, 2000.0), 0.0);
    assert!(!pin.show_jump());
}

#[test]
fn sending_a_prompt_pins_again_after_a_nudge_up_so_the_reply_is_followed() {
    let (mut pin, _) = open();
    pin.note_user_intent(0.0);
    pin.update(at(1500.0 - 30.0, 2000.0), 10.0);
    assert!(pin.show_jump());

    pin.pin_to_bottom();
    assert!(!pin.show_jump());
    pin.scrolled_to_bottom(at(1800.0, 2300.0));
    pin.update(at(1800.0, 2300.0), 50.0);
    assert!(pin.pinned());
}

#[test]
fn pins_again_for_a_session_switch() {
    let (mut pin, _) = open();
    pin.update(at(600.0, 2000.0), 0.0);
    assert!(pin.show_jump());
    pin.pin_to_bottom();
    assert!(!pin.show_jump());
}

#[test]
fn a_glide_to_the_end_passes_far_places_without_leaving_it() {
    let (mut pin, _) = open();
    pin.follow(0.0);
    pin.update(at(900.0, 2000.0), 100.0);
    assert!(pin.pinned(), "our own scroll isn't the reader leaving");
    pin.update(at(900.0, 2000.0), 1600.0);
    assert!(!pin.pinned(), "a glide that never arrived doesn't hold the pin for ever");
}

#[test]
fn asks_for_older_messages_when_scrolled_near_the_top_and_not_before() {
    let (mut pin, _) = open();
    assert!(!pin.update(at(600.0, 2000.0), 0.0).load_older);
    assert!(pin.update(at(100.0, 2000.0), 0.0).load_older);
    assert!(!pin.update(at(101.0, 2000.0), 0.0).load_older);
}

#[test]
fn stops_pinning_to_the_bottom_once_it_has_scrolled_to_a_message() {
    let (mut pin, _) = open();
    pin.show_message();
    pin.update(at(300.0, 2100.0), 0.0);
    assert!(pin.show_jump());
}

#[test]
fn a_shown_message_keeps_its_ring_for_1600_ms() {
    let shown = Shown::new("m02", 1000.0);
    assert!(shown.active(1000.0 + SHOWN_MS - 1.0));
    assert!(!shown.active(1000.0 + SHOWN_MS));
}
