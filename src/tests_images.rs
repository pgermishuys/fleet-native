//! Pictures in the conversation (component 3, piece N): `ToolScreenshot.test.ts` ("a screenshot in the conversation") and
//! the user's image thumbnails, driven headless like the rest of `tests.rs`.
use crate::mock::images::{Case, PAGE, SHOT_PATH};
use crate::tests::{FRAME, app, click, finish, read_request};
use crate::{State, mock};
use rune::{Key, KeyEvent, Mods, Runtime};

fn shown(case: Case) -> Runtime<State> {
    let mut rt = app();
    rt.update(move |s, _| s.show_images(case));
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

fn press(rt: &mut Runtime<State>, key: Key) {
    rt.key(KeyEvent { key, mods: Mods::default() });
    rt.frame(FRAME);
    rt.frame(FRAME);
}

/// A press on the scrim, away from the picture and the close button.
fn press_backdrop(rt: &mut Runtime<State>) {
    rt.pointer_move(8.0, 400.0);
    rt.pointer_down(8.0, 400.0);
    rt.pointer_up(8.0, 400.0);
    rt.frame(FRAME);
    rt.frame(FRAME);
}

// "shows a thumbnail under the call, even with the call's row folded"
#[test]
fn shows_a_thumbnail_under_the_call_even_with_the_calls_row_folded() {
    let rt = shown(Case::Shot);
    assert!(rt.find("shot-c1").is_some(), "the thumbnail");
    assert!(rt.find("tool-c1").is_some(), "its call's row");
    assert!(rt.state.images.ready(SHOT_PATH).is_some(), "decoded");
    // The row is folded: it shows no output box, yet the picture is there.
    assert!(rt.find("tool-body-c1").is_none());
    assert!(rt.find("lightbox").is_none());
    // Under the call, not beside it.
    let ((_, row_y), (_, shot_y)) = (rt.find("tool-c1").unwrap(), rt.find("shot-c1").unwrap());
    assert!(shot_y > row_y + 20.0, "{row_y} {shot_y}");
}

#[test]
fn the_thumbnail_is_120_tall_and_as_wide_as_the_shot_is_in_shape() {
    let rt = shown(Case::Shot);
    let (x, y) = rt.find("shot-c1").unwrap();
    // 1280x800 at 120 tall is 192 wide; the button adds its 1px border all round.
    let (cx, cy) = (x + 96.0 + 1.0, y);
    assert!(rt.find("shot-c1").is_some() && cx > x && cy >= y);
    let rect = rt.presented("shot-c1").map(|p| p.0).expect("its rect");
    assert_eq!((rect[2].round(), rect[3].round()), (194.0, 122.0));
}

// "shows nothing extra for a call without one"
#[test]
fn shows_nothing_extra_for_a_call_without_one() {
    let rt = app();
    assert!(rt.find("shot-c1").is_none());
    let mut rt = shown(Case::Shot);
    rt.update(|s, _| s.show_agents(mock::agents::Case::Done));
    rt.frame(FRAME);
    assert!(rt.find("shot-c1").is_none());
}

// "opens full size on click and closes on Esc, handing focus back to the thumbnail"
#[test]
fn opens_full_size_on_click_and_closes_on_esc_handing_focus_back_to_the_thumbnail() {
    let mut rt = shown(Case::Shot);
    click(&mut rt, "shot-c1");
    rt.frame(FRAME);
    assert!(rt.find("lightbox").is_some() && rt.find("lightbox-image").is_some() && rt.find("lightbox-close").is_some());
    let open = rt.state.images.lightbox.clone().unwrap();
    assert_eq!((open.alt.as_str(), open.key.as_str()), ("Screenshot: Shop · 1280×800", SHOT_PATH));
    press(&mut rt, Key::Escape);
    assert!(rt.find("lightbox").is_none());
    // Where focus lands (the close button on open, the thumbnail on close) can't be read headless; it is set with
    // `autofocus` and `focus_return("shot-c1")`, and is looked at in a window (the pull request says how).
}

#[test]
fn esc_closes_the_picture_and_does_not_also_interrupt_the_turn() {
    let mut rt = app();
    rt.update(|s, _| s.images.lightbox = Some(crate::fleet::images::Lightbox { key: "x".into(), alt: "".into(), opener: "".into() }));
    rt.frame(FRAME);
    assert!(rt.state.session().status == crate::mock::Status::Working);
    press(&mut rt, Key::Escape);
    assert!(rt.state.images.lightbox.is_none());
    assert!(rt.state.session().status == crate::mock::Status::Working, "the turn went on");
}

// "closes from the backdrop and the close button, but not from the picture"
#[test]
fn closes_from_the_backdrop_and_the_close_button_but_not_from_the_picture() {
    let mut rt = shown(Case::Shot);
    click(&mut rt, "shot-c1");
    rt.frame(FRAME);
    click(&mut rt, "lightbox-image");
    assert!(rt.find("lightbox").is_some(), "a press on the picture keeps it open");
    click(&mut rt, "lightbox-close");
    rt.frame(FRAME);
    assert!(rt.find("lightbox").is_none());
    click(&mut rt, "shot-c1");
    rt.frame(FRAME);
    press_backdrop(&mut rt);
    assert!(rt.find("lightbox").is_none(), "a press on the scrim closes it");
}

#[test]
fn the_picture_is_shown_at_its_own_size_or_the_largest_that_fits() {
    let mut rt = shown(Case::Lightbox);
    rt.frame(FRAME);
    // 1280x800 in a 1280x760 window: 90% of the height is 684, so 1094 wide.
    let rect = rt.presented("lightbox-image").map(|p| p.0).expect("the picture");
    assert!((rect[3] - 684.0).abs() < 1.5 && (rect[2] - 1094.4).abs() < 2.0, "{rect:?}");
    // A small one stays its size (the logo is 511x234).
    rt.update(|s, _| {
        s.supply_image("logo", include_bytes!("../assets/weave_logo.png"));
        s.open_lightbox("logo", "", "");
    });
    rt.frame(FRAME);
    let rect = rt.presented("lightbox-image").map(|p| p.0).expect("the picture");
    assert_eq!((rect[2].round(), rect[3].round()), (511.0, 234.0));
}

// "disappears rather than showing a broken image when the shot is gone"
#[test]
fn disappears_rather_than_showing_a_broken_image_when_the_shot_is_gone() {
    let rt = shown(Case::Lost);
    assert!(rt.find("shot-c1").is_none());
    assert!(rt.find("tool-c1").is_some(), "the call's own row stays");
}

#[test]
fn bytes_that_are_not_an_image_or_do_not_decode_are_lost_too() {
    let mut rt = shown(Case::Lost);
    rt.update(|s, _| {
        s.on_image_bytes("a", Ok(b"<html>not found</html>".to_vec()));
        s.supply_image("b", b"\x89PNG\r\n\x1a\nnot really");
    });
    assert!(rt.state.images.is_missing("a") && rt.state.images.is_missing("b"));
}

#[test]
fn a_picture_that_decodes_after_the_view_drew_redraws_the_row() {
    let mut rt = app();
    rt.update(|s, _| {
        s.show_images(Case::Lost);
        s.images = Default::default();
    });
    rt.frame(FRAME);
    assert!(rt.find("shot-c1").is_some(), "the empty box is there while it loads");
    assert!(rt.state.images.ready(SHOT_PATH).is_none());
    rt.update(|s, _| s.supply_image(SHOT_PATH, PAGE));
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(rt.state.images.ready(SHOT_PATH).is_some());
}

#[test]
fn a_user_image_shows_in_its_bubble_and_opens_in_the_lightbox_without_a_name() {
    let mut rt = shown(Case::Attachments);
    assert!(rt.find("img-f1").is_some() && rt.find("img-f2").is_some(), "two thumbnails under the words");
    assert!(rt.find("img-f3").is_some(), "a message of images only still has its bubble");
    // At most 180x120, the shape kept: the 1280x800 page is 180 wide, 112.5 tall.
    let rect = rt.presented("img-f1").map(|p| p.0).unwrap();
    assert_eq!((rect[2].round(), rect[3].round()), (182.0, 115.0));
    click(&mut rt, "img-f1");
    rt.frame(FRAME);
    assert_eq!(rt.state.images.lightbox.as_ref().map(|l| l.alt.as_str()), Some("Image preview"));
}

#[test]
fn a_picture_is_asked_of_fleets_screenshot_path_on_a_thread() {
    use std::io::Write;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let log = seen.clone();
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let line = read_request(&mut stream).lines().next().unwrap_or("").to_string();
            let found = line.contains("shot_1");
            log.lock().unwrap().push(line);
            let (status, body): (&str, &[u8]) = if found { ("200 OK", PAGE) } else { ("404 Not Found", b"{}") };
            let _ = write!(
                stream,
                "HTTP/1.1 {status}\r\ncontent-type: image/png\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(body);
            finish(&mut stream);
        }
    });
    let bytes = crate::fleet::http::get_bytes(&base, SHOT_PATH).unwrap();
    assert_eq!(bytes, PAGE);
    assert!(crate::fleet::http::get_bytes(&base, "/api/sessions/scene/screenshots/gone").is_err(), "a 404 is a lost picture");
    let seen = seen.lock().unwrap();
    assert!(seen[0].starts_with("GET /api/sessions/scene/screenshots/shot_1 "), "{}", seen[0]);
}
