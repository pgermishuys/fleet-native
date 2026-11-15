//! Pictures in the conversation. Ports `components/session/ToolScreenshot.vue` (the thumbnail under a call),
//! `ImageLightbox.vue` (the picture over the window) and the `.msg-images` thumbnails of `MessageBubble.vue`.
//! Fetching and decoding are `fleet::images`; this file only draws what it holds.
//!
//! `object-fit: cover` with `object-position: top left` is `SurfaceFit::Fill`, which crops about the centre: the
//! two agree whenever the box has the picture's shape, which a screenshot's does (its width follows its height).
use super::conversation::Part;
use crate::State;
use crate::conv::image_sniff::Shot;
use crate::fleet::images::{Lightbox, Source};
use crate::icons::X;
use crate::theme::{RADIUS_BTN, Theme};
use rune::{Color, Cursor, El, SurfaceFit, col, el, icon, row};

/// A screenshot is this tall whatever its shape (`.tool-screenshot__img`).
const SHOT_HEIGHT: f32 = 120.0;
/// The widest a thumbnail gets: the stream's column (760) less the card's left indent and right padding.
const SHOT_MAX_WIDTH: f32 = 720.0;
/// A user's image is shown at most this big (`.msg-image-thumb__img`).
const ATTACHMENT_MAX: (f32, f32) = (180.0, 120.0);

/// What the picture is called to anyone who can't see it, and for its tooltip.
pub fn screenshot_label(title: &str) -> String {
    format!("Open screenshot: {title}")
}

/// A thumbnail button: 1px border, the button radius, the picture inside.
fn thumb(t: &Theme, key: String, label: String, fill: Option<Color>) -> El {
    let button = el()
        .key(key)
        .role("button")
        .label(label.clone())
        .help(label)
        .cursor(Cursor::ZoomIn)
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .border_layout()
        .focus_ring(2.0, 2.0, t.accent);
    match fill {
        Some(c) => button.fill(c),
        None => button,
    }
}

/// What the agent saw, under its call (`.tool-screenshot`): lined up with the call's label, 120px tall, opening full size
/// on a press. Shows even while the call's row is folded. A picture Fleet has lost draws nothing.
pub fn screenshot(t: &Theme, s: &State, id: &str, shot: &Shot, title: &str) -> El {
    screenshot_padded(t, s, id, shot, title, (2.0, 8.0, 8.0, 31.0))
}

/// The same with its own padding (top, right, bottom, left): a browser step's shot sits inside the step's grid.
pub fn screenshot_padded(t: &Theme, s: &State, id: &str, shot: &Shot, title: &str, pad: (f32, f32, f32, f32)) -> El {
    s.images.want(&shot.path, Source::Path(shot.path.clone()));
    if s.images.is_missing(&shot.path) {
        return el();
    }
    // The size it was taken at reserves the shape before the picture arrives.
    let width = (SHOT_HEIGHT * shot.width as f32 / shot.height as f32).min(SHOT_MAX_WIDTH);
    let mut face = el().w(width).h(SHOT_HEIGHT).radius(RADIUS_BTN - 1.0);
    if let Some((image, _)) = s.images.ready(&shot.path) {
        face = face.pixels_fit(image.pixels(), SurfaceFit::Fill);
    }
    let (key, path, alt) = (format!("shot-{id}"), shot.path.clone(), format!("Screenshot: {title}"));
    let opener = key.clone();
    let button = thumb(t, key, screenshot_label(title), Some(t.main_bg))
        .on_press(move |s: &mut State| s.open_lightbox(&path, &alt, &opener))
        .child(face);
    col().key(format!("shot-box-{id}")).w_full().pt(pad.0).pr(pad.1).pb(pad.2).pl(pad.3).child(row().child(button))
}

/// The images a user sent, under their words (`.msg-images`: wrapping, 6px apart, 6px below the text).
pub fn user_images(t: &Theme, s: &State, parts: &[&Part]) -> El {
    let mut wrap = row().w_full().flex_wrap().gap(6.0).mt(6.0);
    for part in parts {
        let Part::File { id, filename, url, .. } = part else { continue };
        let key = format!("img-{id}");
        if url.starts_with("data:") {
            s.images.want(&key, Source::Data(url.clone()));
        }
        // Until it has decoded its size is unknown, and so is the box: Fleet's own `<img>` has none either.
        let Some((image, [w, h])) = s.images.ready(&key) else { continue };
        let k = (ATTACHMENT_MAX.0 / w as f32).min(ATTACHMENT_MAX.1 / h as f32).min(1.0);
        let face = el().w(w as f32 * k).h(h as f32 * k).radius(RADIUS_BTN - 1.0).pixels_fit(image.pixels(), SurfaceFit::Fill);
        let name = filename.clone().unwrap_or_else(|| "Image preview".into());
        let (shown, opener) = (key.clone(), key.clone());
        wrap = wrap.child(
            thumb(t, key, name.clone(), None).on_press(move |s: &mut State| s.open_lightbox(&shown, "Image preview", &opener)).child(face),
        );
    }
    wrap
}

/// What a message's pictures are doing, for its row's memo: a picture arriving or being lost redraws it.
pub fn key(s: &State, parts: &[Part]) -> u64 {
    let mut n = 0u64;
    for part in parts {
        let path = match part {
            Part::Tool { body, .. } => body.screenshot.as_ref().map(|shot| shot.path.clone()),
            Part::File { id, mime, .. } if mime.starts_with("image/") => Some(format!("img-{id}")),
            _ => None,
        };
        n = n * 4 + path.map_or(0, |p| u64::from(s.images.state_of(&p)));
    }
    n
}

/// `ImageLightbox.vue`: the picture over the whole window on a dark scrim, at its own size or the largest that fits
/// (90% of the window, 16px in from its edge). Esc, a press on the scrim or the close button put it away; a press on the
/// picture does not. Focus starts on the close button and goes back to the thumbnail.
pub fn lightbox(s: &State, t: &Theme, open: &Lightbox) -> Vec<El> {
    let close = |s: &mut State| s.close_lightbox();
    let (win_w, win_h) = s.images.window;
    let picture = s.images.ready(&open.key).map(|(image, [w, h])| {
        let (w, h) = (w as f32, h as f32);
        let k = ((win_w - 32.0).min(win_w * 0.9) / w).min(win_h * 0.9 / h).min(1.0);
        el().key("lightbox-image")
            .role("img")
            .label(open.alt.clone())
            .w(w * k)
            .h(h * k)
            .box_shadow(t.lightbox_shadow())
            .cursor(Cursor::Default)
            .pixels_fit(image.pixels(), SurfaceFit::Fit)
            // The picture takes the press, so it doesn't reach the scrim behind it.
            .on_press(|_: &mut State| {})
    });
    let button = el()
        .key("lightbox-close")
        .role("button")
        .label("Close preview")
        .help("Close preview")
        .pin(None, Some(16.0), Some(16.0), None)
        .size(36.0, 36.0)
        .center()
        .radius(18.0)
        .fill(Color::WHITE.alpha(0.15))
        .hover(Color::WHITE.alpha(0.25))
        .cursor(Cursor::Pointer)
        .focus_ring(2.0, 2.0, Color::WHITE)
        .autofocus()
        .on_press(close)
        .child(icon(&X, 20.0).color(Color::WHITE));
    let frame = col()
        .key("lightbox")
        .role("dialog")
        .label(open.alt.clone())
        .absolute(0.0, 0.0)
        .w_full()
        .h_full()
        .center()
        .fill(t.scrim())
        .cursor(Cursor::Pointer)
        .on_layout(|s: &mut State, [_, _, w, h]| {
            if s.images.window != (w, h) && w > 0.0 {
                s.images.window = (w, h);
            }
        })
        .on_press(close)
        .modal(false, close)
        .focus_return(open.opener.clone())
        .children(picture)
        .child(button);
    vec![frame]
}
