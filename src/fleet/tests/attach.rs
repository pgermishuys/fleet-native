//! Image attachments (docs/parity/05-composer.md, piece N): the picker, a paste and a drop reach the draft through a thread,
//! the chips draw and remove, and a prompt carries its pictures and gives them back when it is refused. Ports "Composer
//! images" and the refused-restore case of `Composer.test.ts`, with `use-composer-actions.refused.test.ts`.
use super::queue::{Live, click, ctrl, mock_app, press, typed};
use crate::State;
use crate::composer::attach::QUEUE_REFUSAL;
use crate::mock::Status;
use crate::mock::images::PAGE;
use rune::{ClipboardContent, ClipboardImage, Key, Mods, Runtime};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const FRAME: f64 = 1.0 / 60.0;

/// A file of `bytes` in a folder of its own under the temp folder.
fn scratch(name: &str, bytes: &[u8]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("fleet-native-attach-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, bytes).unwrap();
    path
}

fn forget(path: &Path) {
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

fn images_on(rt: &mut Runtime<State>) {
    rt.update(|s, _| {
        for h in &mut s.pickers.harnesses {
            h.supports_image_attachments = true;
        }
    });
    rt.frame(FRAME);
}

/// Frames until `until` holds (the reads are on threads).
fn settle(rt: &mut Runtime<State>, until: impl Fn(&State) -> bool, what: &str) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !until(&rt.state) {
        assert!(Instant::now() < deadline, "{what}");
        rt.frame(FRAME);
        std::thread::sleep(Duration::from_millis(5));
    }
    rt.frame(FRAME);
    rt.frame(FRAME);
}

fn attach(rt: &mut Runtime<State>, path: &Path) {
    let p = path.to_path_buf();
    rt.update(move |s, tx| s.add_files(&[p], tx));
}

fn held(rt: &Runtime<State>) -> Vec<String> {
    rt.state.held_images().iter().map(|i| i.filename.clone()).collect()
}

/// A mock app with a harness that takes images and one picture held.
fn with_picture() -> (Runtime<State>, PathBuf) {
    let mut rt = mock_app(false, "opencode", false, false);
    images_on(&mut rt);
    let path = scratch("login-error.png", PAGE);
    attach(&mut rt, &path);
    settle(
        &mut rt,
        |s| s.held_images().first().is_some_and(|i| s.images.ready(&i.preview_key()).is_some()),
        "the picture to be held and decoded",
    );
    (rt, path)
}

// Composer.test.ts "Composer images": "Attach image is offered everywhere" (the picker is on the toolbar, and opens)
#[test]
fn attach_is_offered_everywhere_and_opens_the_picker() {
    let mut rt = mock_app(false, "opencode", false, false);
    let path = scratch("picked.png", PAGE);
    click(&mut rt, "attach");
    assert!(rt.file_dialog().is_some(), "the paperclip opens the system's file picker");
    images_on(&mut rt);
    rt.resolve_file_dialog(Some(path.clone())).unwrap();
    settle(&mut rt, |s| s.held_images().len() == 1, "the picked picture");
    assert_eq!(held(&rt), ["picked.png"]);
    forget(&path);
}

#[test]
fn a_picked_picture_is_a_chip_with_its_thumbnail_and_the_send_button_wakes() {
    let (rt, path) = with_picture();
    let id = rt.state.held_images()[0].id.clone();
    assert!(rt.find("attachment-strip").is_some());
    assert!(rt.find(&format!("attachment-chip-{id}")).is_some());
    assert!(rt.state.images.ready(&rt.state.held_images()[0].preview_key()).is_some(), "the thumbnail decoded");
    // The thumbnail is 80 px; the chip adds its padding and border.
    let thumb = rt.presented(&format!("attachment-thumb-{id}")).unwrap().0;
    assert_eq!((thumb[2].round(), thumb[3].round()), (80.0, 80.0));
    let chip = rt.presented(&format!("attachment-chip-{id}")).unwrap().0;
    assert_eq!(chip[2].round(), 94.0, "80 + 2 × 6 + 2 × 1");
    // A picture alone is something to send.
    assert!(rt.presented("prompt-send-button").is_some());
    assert_eq!(rt.state.draft.text, "");
    forget(&path);
}

#[test]
fn the_remove_button_takes_the_picture_off_and_lets_its_thumbnail_go() {
    let (mut rt, path) = with_picture();
    let (id, key) = (rt.state.held_images()[0].id.clone(), rt.state.held_images()[0].preview_key());
    click(&mut rt, &format!("attachment-remove-{id}"));
    assert!(rt.state.held_images().is_empty());
    assert!(rt.find("attachment-strip").is_none(), "no strip without an image or a file");
    assert!(rt.state.images.slot(&key).is_none(), "the preview is released");
    forget(&path);
}

#[test]
fn the_thumbnail_opens_the_lightbox_and_escape_hands_focus_back() {
    let (mut rt, path) = with_picture();
    let id = rt.state.held_images()[0].id.clone();
    click(&mut rt, &format!("attachment-thumb-{id}"));
    rt.frame(FRAME);
    assert!(rt.find("lightbox").is_some(), "the lightbox opens over the window");
    press(&mut rt, Key::Escape, Mods::default());
    assert!(rt.find("lightbox").is_none());
    forget(&path);
}

// use-image-attachments.test.ts: "holds an image as base64 with its name" (through the thread and the view)
#[test]
fn a_picture_is_held_under_the_name_of_its_file_with_the_type_its_bytes_say() {
    let mut rt = mock_app(false, "opencode", false, false);
    images_on(&mut rt);
    // A PNG called .jpg is a PNG.
    let path = scratch("shot.jpg", PAGE);
    attach(&mut rt, &path);
    settle(&mut rt, |s| s.held_images().len() == 1, "the picture");
    let image = &rt.state.held_images()[0];
    assert_eq!((image.mime.as_str(), image.filename.as_str()), ("image/png", "shot.jpg"));
    assert!(image.data.starts_with("iVBORw0KGgo"));
    forget(&path);
}

#[test]
fn a_file_that_is_no_picture_is_refused_by_its_bytes_and_the_error_clears_after_five_seconds() {
    let mut rt = mock_app(false, "opencode", false, false);
    images_on(&mut rt);
    let path = scratch("logo.png", b"<svg/>");
    attach(&mut rt, &path);
    settle(&mut rt, |s| s.attach_error().is_some(), "the refusal");
    assert!(held(&rt).is_empty());
    assert_eq!(rt.state.attach_error(), Some("logo.png isn't a PNG, JPEG, GIF or WebP picture."));
    assert!(rt.find("send-prompt-error").is_some(), "it shows in the error box");
    for _ in 0..(5.5 / FRAME) as usize {
        rt.frame(FRAME);
    }
    assert_eq!(rt.state.attach_error(), None, "forgotten after five seconds");
    assert!(rt.find("send-prompt-error").is_none());
    forget(&path);
}

// use-image-attachments.test.ts: "refuses an image the harness can't take"
#[test]
fn a_harness_that_cannot_take_images_refuses_without_reading() {
    let mut rt = mock_app(false, "opencode", false, false);
    let path = scratch("shot.png", PAGE);
    attach(&mut rt, &path);
    assert_eq!(rt.state.attach_error(), Some("This session's harness can't take images."));
    assert!(!rt.state.attach.has_reads(), "no thread was started");
    forget(&path);
}

// use-image-attachments.test.ts: "refuses other types and big images"
#[test]
fn other_types_and_big_pictures_are_refused() {
    let mut rt = mock_app(false, "opencode", false, false);
    images_on(&mut rt);
    // A file that is no picture is held as a file now (piece O); only a picture is checked as one.
    let big = scratch("big.png", &vec![0u8; 5 * 1024 * 1024 + 1]);
    attach(&mut rt, &big);
    settle(&mut rt, |s| s.attach_error().is_some_and(|e| e.contains("exceeds")), "the size refusal");
    assert_eq!(rt.state.attach_error(), Some("Image exceeds the 5 MB limit (5.0 MB)"));
    assert!(held(&rt).is_empty());
    forget(&big);
}

#[test]
fn eleven_pictures_at_once_hold_ten_and_say_so() {
    let mut rt = mock_app(false, "opencode", false, false);
    images_on(&mut rt);
    let paths: Vec<PathBuf> = (0..11).map(|i| scratch(&format!("p{i}.png"), PAGE)).collect();
    let all = paths.clone();
    rt.update(move |s, tx| s.add_files(&all, tx));
    assert_eq!(rt.state.attach_error(), Some("10 files at most per prompt."), "the eleventh, counting reads on their way");
    settle(&mut rt, |s| s.held_images().len() == 10, "ten pictures");
    // The good adds clear the message.
    assert_eq!(rt.state.attach_error(), None);
    for p in &paths {
        forget(p);
    }
}

// Composer.test.ts "Composer images": "pastes a spreadsheet's text rather than its picture"
#[test]
fn pastes_a_spreadsheets_text_rather_than_its_picture() {
    let png = ClipboardImage::from_png(PAGE.to_vec()).expect("a PNG");
    // Where the harness can't take images and the clipboard has text beside the picture, the field gets the text.
    let mut rt = mock_app(false, "opencode", false, false);
    click(&mut rt, "composer");
    rt.paste_content(ClipboardContent::new("Q3, 4, 5").with_image(png.clone()));
    rt.frame(FRAME);
    assert_eq!(rt.state.draft.text, "Q3, 4, 5");
    assert!(held(&rt).is_empty() && rt.state.attach_error().is_none());
    // Where it can, the picture is taken and the text is not pasted.
    images_on(&mut rt);
    rt.paste_content(ClipboardContent::new("Q3, 4, 5").with_image(png));
    settle(&mut rt, |s| s.held_images().len() == 1, "the pasted picture");
    assert_eq!(held(&rt), ["image.png"]);
    assert_eq!(rt.state.draft.text, "Q3, 4, 5", "the draft keeps only what was typed before");
}

#[test]
fn a_screenshot_with_no_text_is_refused_with_the_reason_where_the_harness_cannot_take_it() {
    let png = ClipboardImage::from_png(PAGE.to_vec()).expect("a PNG");
    let mut rt = mock_app(false, "opencode", false, false);
    click(&mut rt, "composer");
    rt.paste_content(ClipboardContent::new("").with_image(png));
    rt.frame(FRAME);
    assert_eq!(rt.state.attach_error(), Some("This session's harness can't take images."));
}

#[test]
fn a_file_copied_in_a_file_manager_is_taken_by_a_paste() {
    let mut rt = mock_app(false, "opencode", false, false);
    images_on(&mut rt);
    click(&mut rt, "composer");
    let path = scratch("copied.png", PAGE);
    rt.paste_content(ClipboardContent::new("copied.png").with_files([path.clone()]));
    settle(&mut rt, |s| s.held_images().len() == 1, "the copied file");
    assert_eq!(held(&rt), ["copied.png"]);
    forget(&path);
}

// row 1.4: dragging a file over the composer; a drop attaches
#[test]
fn a_file_dropped_on_the_composer_is_attached_and_the_frame_lights_while_it_is_over() {
    let mut rt = mock_app(false, "opencode", false, false);
    images_on(&mut rt);
    let path = scratch("dropped.png", PAGE);
    rt.drag_files(vec![path.clone()], rt.find("composer-frame"));
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(rt.state.dragging, "the frame shows it will take the file");
    let at = rt.find("composer-frame");
    rt.drag_files(vec![], None);
    rt.drop_files(vec![path.clone()], at);
    settle(&mut rt, |s| s.held_images().len() == 1, "the dropped picture");
    assert!(!rt.state.dragging);
    assert_eq!(held(&rt), ["dropped.png"]);
    forget(&path);
}

// ─── sending ────────────────────────────────────────────────────────────────

fn live_idle(refuse: Option<(u16, serde_json::Value)>) -> Live {
    let mut live = Live::start(json!([]), refuse, "opencode", false);
    live.rt.update(|s, _| {
        s.pickers.harnesses[0].supports_image_attachments = true;
        s.projects[0].sessions[0].status = Status::Idle;
    });
    live.rt.frame(FRAME);
    live
}

fn prompts(live: &Live) -> Vec<serde_json::Value> {
    live.calls().into_iter().filter(|(m, p, _)| m == "POST" && p.ends_with("/prompt")).map(|(_, _, b)| b).collect()
}

// the request body: `attachments: [{mime, filename, data}]`
#[test]
fn a_prompt_carries_its_pictures_as_base64_and_they_leave_the_draft() {
    let mut live = live_idle(None);
    let path = scratch("login-error.png", PAGE);
    attach(&mut live.rt, &path);
    live.wait(&|l| l.rt.state.held_images().len() == 1, "the picture");
    typed(&mut live.rt, "what is wrong here?");
    press(&mut live.rt, Key::Enter, Mods::default());
    live.wait(&|l| !prompts(l).is_empty(), "the prompt");
    let body = &prompts(&live)[0];
    assert_eq!(body["text"], "what is wrong here?");
    let sent = &body["attachments"];
    assert_eq!(sent.as_array().unwrap().len(), 1);
    assert_eq!((sent[0]["mime"].as_str(), sent[0]["filename"].as_str()), (Some("image/png"), Some("login-error.png")));
    let data = sent[0]["data"].as_str().unwrap();
    assert!(data.starts_with("iVBORw0KGgo") && !data.starts_with("data:"), "base64 with no prefix");
    assert!(live.rt.state.held_images().is_empty(), "the draft let go of them");
    forget(&path);
}

#[test]
fn a_picture_alone_is_a_prompt() {
    let mut live = live_idle(None);
    let path = scratch("only.png", PAGE);
    attach(&mut live.rt, &path);
    live.wait(&|l| l.rt.state.held_images().len() == 1, "the picture");
    click(&mut live.rt, "prompt-send-button");
    live.wait(&|l| !prompts(l).is_empty(), "the prompt");
    let body = &prompts(&live)[0];
    assert_eq!(body["text"], "");
    assert_eq!(body["attachments"][0]["filename"], "only.png");
    forget(&path);
}

// use-composer-actions.refused.test.ts: "puts the text back and tells the composer, so the photos it cleared come back too"
// Composer.test.ts: "a refused message restores images"
#[test]
fn a_refused_prompt_brings_its_pictures_back_beside_anything_added_since() {
    let mut live = live_idle(Some((500, json!({"error": "The model is down."}))));
    let first = scratch("first.png", PAGE);
    attach(&mut live.rt, &first);
    live.wait(&|l| l.rt.state.held_images().len() == 1, "the picture");
    typed(&mut live.rt, "look");
    press(&mut live.rt, Key::Enter, Mods::default());
    // Something added while the request is on its way stays.
    let second = scratch("second.png", PAGE);
    attach(&mut live.rt, &second);
    live.wait(&|l| l.rt.state.composer_error.is_some(), "the refusal");
    live.wait(&|l| l.rt.state.held_images().len() == 2, "the pictures to come back");
    let names: Vec<String> = live.rt.state.held_images().iter().map(|i| i.filename.clone()).collect();
    assert!(names.contains(&"first.png".to_string()) && names.contains(&"second.png".to_string()), "{names:?}");
    assert_eq!(live.rt.state.draft.text, "look", "the text comes back too");
    // Their thumbnails are made again from the text the prompt carried.
    live.wait(
        &|l| l.rt.state.held_images().iter().all(|i| l.rt.state.images.ready(&i.preview_key()).is_some()),
        "the thumbnails to come back",
    );
    forget(&first);
    forget(&second);
}

// the queue refusal text
#[test]
fn a_prompt_that_would_wait_in_the_queue_with_a_picture_pending_keeps_both_and_says_why() {
    let mut live = Live::start(json!([]), None, "opencode", false);
    live.rt.update(|s, _| s.pickers.harnesses[0].supports_image_attachments = true);
    live.working();
    let path = scratch("later.png", PAGE);
    attach(&mut live.rt, &path);
    live.wait(&|l| l.rt.state.held_images().len() == 1, "the picture");
    live.working();
    typed(&mut live.rt, "and this");
    press(&mut live.rt, Key::Enter, Mods::default());
    assert_eq!(live.rt.state.composer_error.as_deref(), Some(QUEUE_REFUSAL));
    assert_eq!(
        live.rt.state.composer_error.as_deref(),
        Some("Images can't wait in the queue yet. Send now with Ctrl+Enter, or send once the turn ends.")
    );
    assert_eq!(live.rt.state.draft.text, "and this");
    assert_eq!(live.rt.state.held_images().len(), 1);
    assert!(live.calls().is_empty(), "nothing was sent");
    // Ctrl+Enter steers where the harness can; the picture goes with it.
    press(&mut live.rt, Key::Enter, ctrl());
    live.wait(&|l| !prompts(l).is_empty(), "the steered prompt");
    assert_eq!(prompts(&live)[0]["delivery"], "steer");
    assert_eq!(prompts(&live)[0]["attachments"][0]["filename"], "later.png");
    forget(&path);
}

#[test]
fn pictures_belong_to_their_session() {
    let (mut rt, path) = with_picture();
    click(&mut rt, "session-0-1");
    assert!(rt.state.held_images().is_empty(), "another session has no pictures");
    click(&mut rt, "session-0-2");
    assert_eq!(rt.state.held_images().len(), 1, "they wait where they were attached");
    forget(&path);
}
