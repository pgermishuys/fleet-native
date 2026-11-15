//! Image attachments (docs/parity/05-composer.md, piece N): `use-image-attachments.test.ts` and the rules of `addFiles` and
//! `handlePaste`, under Fleet's names. The reading and encoding on a thread are tested in `fleet::tests::attach`.
use super::attach::*;
use std::sync::Arc;

const PNG_HEAD: [u8; 8] = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];

fn held(id: &str, name: &str) -> HeldImage {
    HeldImage { id: id.into(), mime: "image/png".into(), filename: name.into(), data: Arc::from("iVBORw0K") }
}

/// What `addImage` does with a file, in one step: the checks, then the bytes.
fn add_image(a: &mut Attached, owner: &str, supports: bool, name: &str, bytes: &[u8], now: f64) {
    let held_now = a.of(owner).len();
    let checked = check_image(supports, mime_from_name(name), bytes.len(), held_now);
    let prepared = checked.and_then(|_| prepare(name, bytes));
    match prepared {
        Ok((mime, data)) => {
            let id = a.next_id();
            a.add(owner, HeldImage { id, mime: mime.into(), filename: name.into(), data: Arc::from(data) });
            a.clear_error();
        }
        Err(message) => a.set_error(message, now),
    }
}

fn png(size: usize) -> Vec<u8> {
    let mut bytes = vec![0u8; size.max(PNG_HEAD.len())];
    bytes[..8].copy_from_slice(&PNG_HEAD);
    bytes
}

// use-image-attachments.test.ts: "holds an image as base64 with its name"
#[test]
fn holds_an_image_as_base64_with_its_name() {
    let mut a = Attached::default();
    add_image(&mut a, "s", true, "login-error.png", &png(8), 0.0);
    assert_eq!(a.of("s").len(), 1);
    assert_eq!((a.of("s")[0].mime.as_str(), a.of("s")[0].filename.as_str()), ("image/png", "login-error.png"));
    assert_eq!(&*a.of("s")[0].data, "iVBORw0KGgo=", "the eight signature bytes, in base64 without a prefix");
    assert_eq!(a.error(), None);
}

// use-image-attachments.test.ts: "sends a PNG named .jpg as a PNG, and refuses bytes that aren't a picture"
#[test]
fn sends_a_png_named_jpg_as_a_png_and_refuses_bytes_that_arent_a_picture() {
    let mut a = Attached::default();
    let mut mislabelled = PNG_HEAD.to_vec();
    mislabelled.extend([0, 0, 0, 0]);
    // The name says JPEG; the check passes on the name and the bytes decide.
    add_image(&mut a, "s", true, "shot.jpg", &mislabelled, 0.0);
    assert_eq!((a.of("s")[0].mime.as_str(), a.of("s")[0].filename.as_str()), ("image/png", "shot.jpg"));

    add_image(&mut a, "s", true, "logo.png", b"<svg/>", 0.0);
    assert_eq!(a.of("s").len(), 1);
    assert_eq!(a.error(), Some("logo.png isn't a PNG, JPEG, GIF or WebP picture."));
    assert_eq!(prepare("", b"<svg/>").unwrap_err(), "That file isn't a PNG, JPEG, GIF or WebP picture.");
}

// use-image-attachments.test.ts: "refuses an image the harness can't take"
#[test]
fn refuses_an_image_the_harness_cant_take() {
    let mut a = Attached::default();
    add_image(&mut a, "s", false, "shot.png", &png(8), 0.0);
    assert!(a.of("s").is_empty());
    assert_eq!(a.error(), Some("This session's harness can't take images."));
}

// use-image-attachments.test.ts: "refuses other types and big images"
#[test]
fn refuses_other_types_and_big_images() {
    let mut a = Attached::default();
    add_image(&mut a, "s", true, "notes.txt", b"x", 0.0);
    assert_eq!(a.error(), Some("Unsupported image type: text/plain"));
    add_image(&mut a, "s", true, "mystery", b"x", 0.0);
    assert_eq!(a.error(), Some("Unsupported image type: unknown"));
    add_image(&mut a, "s", true, "big.png", &png(5 * 1024 * 1024 + 1), 0.0);
    assert_eq!(a.error(), Some("Image exceeds the 5 MB limit (5.0 MB)"));
    assert!(a.of("s").is_empty());
    assert!(check_image(true, "image/png", 5 * 1024 * 1024, 0).is_ok(), "exactly 5 MB is allowed");
}

// use-image-attachments.test.ts: "forgets the error after a few seconds"
#[test]
fn forgets_the_error_after_a_few_seconds() {
    let mut a = Attached::default();
    add_image(&mut a, "s", false, "shot.png", &png(8), 10.0);
    assert!(a.error().is_some());
    assert!(!a.expire(14.9));
    assert!(a.error().is_some());
    assert!(a.expire(15.0));
    assert_eq!(a.error(), None);
    // The next good add clears it too.
    a.set_error("old", 20.0);
    add_image(&mut a, "s", true, "ok.png", &png(8), 21.0);
    assert_eq!(a.error(), None);
}

// use-image-attachments.test.ts: "removes and clears images"
#[test]
fn removes_and_clears_images() {
    let mut a = Attached::default();
    add_image(&mut a, "s", true, "a.png", &png(8), 0.0);
    add_image(&mut a, "s", true, "b.png", &png(8), 0.0);
    let first = a.of("s")[0].id.clone();
    assert_eq!(a.remove("s", &first).map(|i| i.filename), Some("a.png".into()));
    assert_eq!(a.of("s").iter().map(|i| i.filename.as_str()).collect::<Vec<_>>(), ["b.png"]);
    assert_eq!(a.clear("s").len(), 1);
    assert!(a.of("s").is_empty());
}

#[test]
fn pictures_belong_to_their_draft() {
    let mut a = Attached::default();
    a.add("one", held("1", "a.png"));
    assert!(a.of("two").is_empty());
    assert_eq!(a.of("one").len(), 1);
}

// Composer.vue `addFiles`: "10 files at most per prompt." for a file; "Too many images (max 10)" for `addImage` itself
#[test]
fn ten_attachments_at_most_and_the_reason_depends_on_the_way_in() {
    assert_eq!(check_file(true, "image/png", 1, 10).unwrap_err(), "10 files at most per prompt.");
    assert_eq!(check_image(true, "image/png", 1, 10).unwrap_err(), "Too many images (max 10)");
    assert!(check_file(true, "image/png", 1, 9).is_ok());
    // A harness without images is told before the count.
    assert_eq!(check_file(false, "image/png", 1, 10).unwrap_err(), "This session's harness can't take images.");
}

#[test]
fn a_name_gives_the_type_a_browser_would() {
    assert_eq!(mime_from_name("A.PNG"), "image/png");
    assert_eq!(mime_from_name("photo.jpeg"), "image/jpeg");
    assert_eq!(mime_from_name("anim.gif"), "image/gif");
    assert_eq!(mime_from_name("pic.webp"), "image/webp");
    assert_eq!(mime_from_name("archive.zip"), "");
    assert_eq!(mime_from_name("noext"), "");
}

// Composer.test.ts "Composer images": "pastes a spreadsheet's text rather than its picture"
#[test]
fn pastes_a_spreadsheets_text_rather_than_its_picture() {
    // A picture beside text, where the harness can't take pictures: the text is what's meant.
    assert!(!paste_takes_files(false, &["image/png"], true));
    // No text beside it: the picture is taken (and refused with the reason).
    assert!(paste_takes_files(false, &["image/png"], false));
    // A harness that takes pictures takes them, text or not.
    assert!(paste_takes_files(true, &["image/png"], true));
    // Nothing to take: a plain text paste.
    assert!(!paste_takes_files(true, &[], true));
}

#[test]
fn base64_is_standard_and_padded() {
    assert_eq!(encode_base64(b""), "");
    assert_eq!(encode_base64(b"f"), "Zg==");
    assert_eq!(encode_base64(b"fo"), "Zm8=");
    assert_eq!(encode_base64(b"foo"), "Zm9v");
    let bytes: Vec<u8> = (0..=255).collect();
    assert_eq!(crate::conv::image_sniff::decode_base64(&encode_base64(&bytes)).unwrap(), bytes);
}

// Composer.vue `putBackRefused`: the pictures of a refused prompt come back beside anything added since
#[test]
fn a_refused_prompt_gives_its_pictures_back_beside_anything_added_since() {
    let mut a = Attached::default();
    a.add("s", held("1", "a.png"));
    a.add("s", held("2", "b.png"));
    assert_eq!(a.take_for_send("s", "prompt-1").len(), 2);
    assert!(a.of("s").is_empty(), "sending clears the draft's pictures");
    a.add("s", held("3", "new.png"));
    let (owner, back) = a.refused("prompt-1").unwrap();
    assert_eq!((owner.as_str(), back.len()), ("s", 2));
    // Each only once: a second refusal of the same prompt has nothing.
    assert!(a.refused("prompt-1").is_none());
    // A prompt the server took has nothing to give back.
    a.take_for_send("s", "prompt-2");
    a.delivered("prompt-2");
    assert!(a.refused("prompt-2").is_none());
}

#[test]
fn reads_in_flight_count_toward_the_limit() {
    let mut a = Attached::default();
    a.begin_read("s");
    a.begin_read("s");
    assert_eq!(a.reading("s"), 2);
    assert!(a.has_reads());
    a.end_read("s");
    a.end_read("s");
    a.end_read("s");
    assert!(!a.has_reads());
}
