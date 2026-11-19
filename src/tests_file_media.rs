//! The file tab's images and Rendered views (component 11, piece I): `FileCanvas.test.ts`'s three image cases and "Markdown opens
//! rendered from the buffer, with Source and Diff beside it" (the annotation waits for 12's piece N), on the scenes' invented files.
use crate::State;
use crate::mock::file_tab::{Case, IMAGE_PATH};
use crate::tests::{FRAME, app, click};
use rune::Runtime;

fn open(case: Case) -> Runtime<State> {
    let mut rt = app();
    rt.update(move |s, _| {
        s.sheet_w = 1400.0;
        s.show_file(case, true)
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

fn has(rt: &Runtime<State>, key: &str) -> bool {
    rt.find(key).is_some()
}

fn said(rt: &Runtime<State>, key: &str) -> String {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).and_then(|e| e.text).unwrap_or_default()
}

fn supply(rt: &mut Runtime<State>, path: &'static str, bytes: &'static [u8]) {
    rt.update(move |s, _| {
        let session = s.canvas_session();
        s.supply_session_image(&session, path, bytes);
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
}

// FileCanvas.test.ts "shows an image as a picture with its pixel size and file size, never in the editor, however large"
#[test]
fn shows_an_image_as_a_picture_with_its_pixel_size_and_file_size_never_in_the_editor_however_large() {
    let mut rt = open(Case::Image);
    assert!(has(&rt, "file-image") && has(&rt, "file-image-picture"));
    assert_eq!(said(&rt, "image-meta"), format!("96 \u{d7} 96 \u{b7} {} B", crate::mock::file_tab::LOGO.len()));
    for editor in ["file-code", "file-view-switch", "file-wrap"] {
        assert!(!has(&rt, editor), "{editor} has no place over a picture");
    }
    // The buffer was never made: an image is never read as text.
    let session = rt.state.canvas_session();
    let machine = rt.state.machine_key_of(&session).to_string();
    assert!(rt.state.file_buffers.info(&machine, &session, IMAGE_PATH).is_none());

    // A big picture is just as much a picture: its bytes read "1 MB"-ish and no code view comes.
    let mut big = vec![0u8; 3 * 1024 * 1024];
    big[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
    rt.update(move |s, _| {
        let session = s.canvas_session();
        s.supply_session_image(&session, IMAGE_PATH, &big);
    });
    rt.frame(FRAME);
    assert!(!has(&rt, "file-code"));
    assert_eq!(said(&rt, "file-unavailable"), "This image couldn't be shown.", "bytes that don't decode say so, still no editor");
}

#[test]
fn a_raster_picture_says_its_pixels_and_is_never_drawn_bigger_than_it_is() {
    let mut rt = open(Case::Image);
    supply(&mut rt, IMAGE_PATH, include_bytes!("../assets/mock_page.png"));
    let [w, h] = {
        let session = rt.state.canvas_session();
        let machine = rt.state.machine_key_of(&session).to_string();
        match rt.state.session_images.shown(&machine, &session, IMAGE_PATH) {
            crate::fleet::session_image::Shown::Picture { size, .. } => size,
            _ => panic!("not shown"),
        }
    };
    assert!(said(&rt, "image-meta").starts_with(&format!("{w} \u{d7} {h} \u{b7} ")), "{}", said(&rt, "image-meta"));
    let shown = rt.presented("file-image-picture").map(|p| p.0).expect("the picture");
    assert!(shown[2] <= w + 0.5 && shown[3] <= h + 0.5, "never up: {shown:?} for {w}x{h}");
    assert!((shown[2] / shown[3] - w / h).abs() < 0.02, "the shape is kept");
}

#[test]
fn a_picture_that_does_not_fit_is_scaled_down_to_fit_with_24_around_it() {
    use crate::view::file_media::fit;
    assert_eq!(fit([96.0, 96.0], [500.0, 500.0]), [96.0, 96.0], "never up");
    assert_eq!(fit([800.0, 400.0], [400.0, 300.0]), [400.0, 200.0], "wide: by width");
    assert_eq!(fit([400.0, 800.0], [400.0, 300.0]), [150.0, 300.0], "tall: by height");
}

// FileCanvas.test.ts "shows an SVG as a picture too, with its size alone when it has no pixel size"
#[test]
fn shows_an_svg_as_a_picture_too_with_its_size_alone_when_it_has_no_pixel_size() {
    use crate::fleet::session_image::svg_has_own_size;
    assert!(svg_has_own_size(crate::mock::file_tab::LOGO.as_bytes()));
    assert!(!svg_has_own_size(b"<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 10 10'><rect width='5' height='5'/></svg>"));
    assert!(!svg_has_own_size(b"<svg width=\"100%\" height=\"100%\" viewBox=\"0 0 10 10\"></svg>"));
    assert!(svg_has_own_size(b"<?xml version=\"1.0\"?>\n<svg width='20.5' height=\"30px\"></svg>"));

    let mut rt = open(Case::Image);
    let bare: &'static [u8] =
        b"<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 40 20\"><rect width=\"40\" height=\"20\" fill=\"#2f6fed\"/></svg>";
    supply(&mut rt, IMAGE_PATH, bare);
    assert!(has(&rt, "file-image-picture"));
    assert_eq!(said(&rt, "image-meta"), format!("{} B", bare.len()));
}

// FileCanvas.test.ts "says so when the image is no longer there"
#[test]
fn says_so_when_the_image_is_no_longer_there() {
    let rt = open(Case::ImageMissing);
    assert_eq!(said(&rt, "file-unavailable"), "This file isn't in the session's folder any more.");
    assert!(!has(&rt, "file-image-picture") && !has(&rt, "image-meta"));
}

#[test]
fn any_other_refusal_says_the_image_could_not_be_read_and_a_new_read_clears_it() {
    let mut rt = open(Case::ImageMissing);
    rt.update(|s, _| {
        let session = s.canvas_session();
        s.answer_session_image(&session, IMAGE_PATH, Ok((500, Vec::new())));
    });
    rt.frame(FRAME);
    assert_eq!(said(&rt, "file-unavailable"), "The image couldn't be read.");
    rt.update(|s, _| {
        let session = s.canvas_session();
        s.answer_session_image(&session, IMAGE_PATH, Err("no answer".into()));
    });
    rt.frame(FRAME);
    assert_eq!(said(&rt, "file-unavailable"), "The image couldn't be read.");
    supply(&mut rt, IMAGE_PATH, crate::mock::file_tab::LOGO.as_bytes());
    assert!(!has(&rt, "file-unavailable") && has(&rt, "file-image-picture"));
}

#[test]
fn opening_says_so_until_the_bytes_arrive_and_an_old_answer_does_not_win() {
    let mut rt = open(Case::Image);
    rt.update(|s, _| {
        let session = s.canvas_session();
        let machine = s.machine_key_of(&session).to_string();
        // A newer read is on its way; the older one's answer is dropped.
        s.read_session_image(&session, IMAGE_PATH);
        s.on_session_image(&machine, &session, IMAGE_PATH, 0, Ok((404, Vec::new())));
    });
    rt.frame(FRAME);
    assert!(!has(&rt, "file-unavailable"), "the stale 404 changed nothing");
    assert!(has(&rt, "file-image-picture"), "the old picture stays up while the file is read again");
    rt.update(|s, _| {
        let session = s.canvas_session();
        #[allow(clippy::unnecessary_to_owned)]
        s.session_images.forget(&s.machine_key_of(&session).to_string(), &session, IMAGE_PATH);
    });
    rt.frame(FRAME);
    assert_eq!(said(&rt, "file-opening"), "Opening assets/logo.svg\u{2026}");
}

// FileCanvas.test.ts "Markdown opens rendered from the buffer, annotatable, with Source and Diff beside it" (not yet annotatable: 12's piece N)
#[test]
fn markdown_opens_rendered_from_the_buffer_with_source_and_diff_beside_it() {
    let mut rt = open(Case::Markdown);
    assert!(has(&rt, "file-rendered"), "Rendered is what a Markdown file opens in");
    assert!(!has(&rt, "file-code"), "no editor under it");
    assert!(!has(&rt, "file-wrap"), "Wrap belongs to the code");
    let edit = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("file-view-edit")).and_then(|e| e.label);
    assert_eq!(edit.as_deref(), Some("Source"), "Edit is named Source beside Rendered");
    assert!(has(&rt, "file-view-rendered") && has(&rt, "file-view-diff"));
    let heading = rt.elements().into_iter().filter_map(|e| e.text).find(|t| t.contains("Ledger Lite"));
    assert!(heading.is_some(), "the text went through the renderer");
    click(&mut rt, "file-view-edit");
    rt.frame(FRAME);
    assert!(has(&rt, "file-code") && !has(&rt, "file-rendered"), "Source shows the text");
    assert!(has(&rt, "file-wrap"));
    click(&mut rt, "file-view-rendered");
    rt.frame(FRAME);
    assert!(has(&rt, "file-rendered") && !has(&rt, "file-code"));
}

#[test]
fn rendered_markdown_has_the_padding_of_fleets_rendered_pane() {
    let rt = open(Case::Markdown);
    let pane = rt.presented("file-rendered").map(|p| p.0).expect("the pane");
    let first = rt.presented("file-markdown").map(|p| p.0).expect("the text");
    assert!((first[0] - pane[0] - 22.0).abs() < 1.5, "22 on the left: {first:?} in {pane:?}");
    assert!((first[1] - pane[1] - 16.0).abs() < 1.5, "16 above: {first:?} in {pane:?}");
}

#[test]
fn rendered_html_is_a_frame_with_no_padding_and_the_file_tab_is_wanted_as_a_page() {
    use crate::web::{Wanted, wanted};
    let mut rt = open(Case::Html);
    assert!(has(&rt, "file-rendered"));
    assert!(has(&rt, "file-html-fallback"), "headless: no window, so the fallback panel");
    assert_eq!(wanted(&rt.state), Wanted::Nothing);
    rt.update(|s, _| {
        s.web.available = true;
        s.web.rect = Some([600.0, 60.0, 560.0, 500.0]);
    });
    let Wanted::Shown(p) = wanted(&rt.state) else { panic!("{:?}", wanted(&rt.state)) };
    assert_eq!(p.tab, "file:docs/report.html");
    let html = p.html.unwrap_or_default();
    assert!(html.contains("script-src 'none'") && html.contains("Pagination report"), "scripts refused, the file's own text");
    // Source shows the code and no page.
    rt.update(|s, _| s.set_active_file_view("docs/report.html", crate::fleet::canvases::FileView::Edit));
    assert_eq!(wanted(&rt.state), Wanted::Nothing);
}
