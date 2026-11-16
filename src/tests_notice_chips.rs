//! The notice chips, the settle and the lift (component 9, piece M): `NoticeCard.test.ts` 11 and 12 under their names,
//! and the chips of `NoticeChips.vue`.
use crate::fleet::notice_place::Place;
use crate::fleet::notices::{HOLD_SECS, Link, Notice, STARTUP_QUIET_SECS};
use crate::tests::{FRAME, app, click};
use rune::Runtime;

const ID: &str = "update:app:0.37.0";

fn ready() -> Notice {
    Notice { body: Some("Installs when you restart.".into()), ..Notice::new(ID, "Fleet 0.37.0 is ready").chip("Fleet 0.37.0 ready") }
}

fn run(rt: &mut Runtime<crate::State>, seconds: f64) {
    for _ in 0..(seconds * 20.0).ceil() as usize {
        rt.frame(1.0 / 20.0);
    }
}

fn open_card(rt: &mut Runtime<crate::State>, notice: Notice) {
    rt.update(|s, _| s.notices.post(notice, 0.0));
    run(rt, STARTUP_QUIET_SECS + 0.5);
    assert!(rt.find("notice-card").is_some(), "the card opened");
}

fn rect(rt: &Runtime<crate::State>, key: &str) -> [f32; 4] {
    rt.presented(key).unwrap_or_else(|| panic!("no element keyed {key:?}")).0
}

// NoticeCard.test.ts: "sits in the bottom-right corner, lifted above a composer it would cover" (the arithmetic, with the
// test's own numbers: a 1280 by 800 window, a 264 wide card 100 tall, a composer 640 to 760 high)
#[test]
fn sits_in_the_bottom_right_corner_lifted_above_a_composer_it_would_cover() {
    let place =
        |composer: [f32; 4]| Place { window: [1280.0, 800.0], card: [1004.0, 660.0, 264.0, 100.0], composer: Some(composer), chip: None };
    // The right panel is open: the composer ends well left of the card.
    assert_eq!(place([400.0, 640.0, 500.0, 120.0]).bottom(), 40.0);
    // The right panel closes: the composer reaches under the card, so the card lifts clear of its top.
    assert_eq!(place([400.0, 640.0, 860.0, 120.0]).bottom(), 800.0 - 640.0 + 8.0);
    // No composer, or one that is not drawn.
    assert_eq!(Place { composer: None, ..place([0.0; 4]) }.bottom(), 40.0);
}

#[test]
fn the_open_card_rises_clear_of_the_composer_in_the_running_app() {
    let mut rt = app();
    open_card(&mut rt, ready());
    let (card, composer) = (rect(&rt, "notice-card"), rect(&rt, "composer-frame"));
    let window = rt.state.place.window;
    assert!(window[1] > 0.0, "the card's frame reported the window");
    let overlaps = composer[0] < card[0] + card[2] && composer[0] + composer[2] > card[0];
    if overlaps {
        assert!(card[1] + card[3] <= composer[1] - 7.5, "lifted above the composer: card {card:?}, composer {composer:?}");
    } else {
        assert!((card[1] + card[3] - (window[1] - 40.0)).abs() < 1.0, "in the corner: card {card:?}");
    }
}

// NoticeCard.test.ts: "links to the release notes"
#[test]
fn links_to_the_release_notes() {
    let mut rt = app();
    let link = Link { label: "What's new".into(), href: "https://example.test/notes".into(), run: None };
    open_card(&mut rt, Notice { link: Some(link), ..ready() });
    let n = rt.state.notices.open().unwrap();
    assert_eq!(n.link.as_ref().map(|l| (l.label.as_str(), l.href.as_str())), Some(("What's new", "https://example.test/notes")));
    let body = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("notice-body")).expect("the body is drawn");
    assert!(
        body.label.as_deref().unwrap_or_default().contains("What's new") || body.text.as_deref().unwrap_or_default().contains("What's new")
    );
}

#[test]
fn a_settled_card_leaves_its_chip_in_the_status_bar_and_a_press_opens_it_again() {
    let mut rt = app();
    open_card(&mut rt, ready());
    assert!(rt.find("notice-chip-ghost").is_some(), "the chip's place is laid out while the card is open");
    assert!(rt.find(&format!("notice-chip-{ID}")).is_none());
    run(&mut rt, HOLD_SECS + 1.0);
    assert!(rt.find("notice-card").is_none());
    assert!(rt.find(&format!("notice-chip-{ID}")).is_some());
    run(&mut rt, 1.0);
    click(&mut rt, &format!("notice-chip-{ID}"));
    rt.frame(FRAME);
    assert_eq!(rt.state.notices.open().map(|n| n.id.as_str()), Some(ID));
    assert!(rt.state.notices.pinned());
}

#[test]
fn a_chip_with_its_own_click_tells_its_source_instead_of_opening_the_card() {
    let mut rt = app();
    let n = Notice { on_chip_click: Some("open-updates".into()), ..ready() };
    open_card(&mut rt, n);
    run(&mut rt, HOLD_SECS + 1.5);
    click(&mut rt, &format!("notice-chip-{ID}"));
    assert_eq!(rt.state.notices.take_heard(), ["open-updates"]);
    assert!(rt.state.notices.open().is_none());
}

#[test]
fn a_quiet_chip_has_no_card_and_no_ring() {
    let mut rt = app();
    rt.update(|s, _| s.notices.post(Notice::new("quiet:1", "Updated to 0.37.0").chip("Updated to 0.37.0").quiet(), 0.0));
    run(&mut rt, 0.5);
    assert!(rt.find("notice-card").is_none());
    assert!(rt.find("notice-chip-quiet:1").is_some());
    assert!(rt.find("notice-chip-ring").is_none());
}

#[test]
fn a_new_chip_pulses_a_ring_twice_and_is_then_still() {
    let mut rt = app();
    rt.update(|s, _| s.notices.post(Notice::new("n:1", "Heads up").chip("Heads up").expires(1000.0), 0.0));
    rt.update(|s, _| s.notices.reopen("n:1"));
    rt.update(|s, _| s.notices.settle());
    run(&mut rt, 0.2);
    run(&mut rt, 0.5);
    assert!(rt.find("notice-chip-ring").is_some(), "the ring is out 0.4 s after the chip arrives");
    run(&mut rt, 3.0);
    assert!(rt.find("notice-chip-ring").is_none(), "and still after its two pulses");
}

#[test]
fn the_card_knows_where_its_chip_is_so_it_can_settle_into_it() {
    let mut rt = app();
    open_card(&mut rt, ready());
    run(&mut rt, 0.5);
    let (card, chip) = (rect(&rt, "notice-card"), rect(&rt, "notice-chip-ghost"));
    let ([dx, dy], scale) = rt.state.place.settle().expect("both laid out");
    assert_eq!([dx, dy], [(chip[0] + chip[2]) - (card[0] + card[2]), (chip[1] + chip[3]) - (card[1] + card[3])]);
    assert!((0.2..1.0).contains(&scale), "shrinks to the chip's width: {scale}");
    assert!(dy > 0.0, "the chip sits lower than the card");
}

#[test]
fn the_card_folds_into_its_chip_over_a_third_of_a_second_not_at_once() {
    let mut rt = app();
    open_card(&mut rt, ready());
    run(&mut rt, 0.5);
    // Through a real press: a change made by `Runtime::update` skips the exit (docs/RUNE.md).
    click(&mut rt, "notice-dismiss");
    let at = |rt: &Runtime<crate::State>| rt.presented("notice-card").map(|r| r.0);
    let start = at(&rt).expect("still drawn the frame it settles");
    for _ in 0..8 {
        rt.frame(FRAME);
    }
    let (mid, faded) = rt.presented("notice-card").expect("still on its way 0.13 s in");
    assert!(mid[1] > start[1] + 1.0 && mid[0] != start[0], "it moved toward the chip, down and right: {start:?} then {mid:?}");
    assert!(faded < 1.0, "and is fading: {faded}");
    run(&mut rt, 0.6);
    assert!(rt.find("notice-card").is_none(), "gone once it has folded in");
}

#[test]
fn the_card_folds_into_its_chip_when_its_hold_ends_too() {
    let mut rt = app();
    open_card(&mut rt, ready());
    let mut seen_leaving = false;
    for _ in 0..((HOLD_SECS + 1.0) * 60.0) as usize {
        rt.frame(FRAME);
        if rt.state.notices.open().is_none() && rt.presented("notice-card").is_some() {
            seen_leaving = true;
            break;
        }
    }
    assert!(seen_leaving, "the card was still drawn, fading, after the hold ended");
}
