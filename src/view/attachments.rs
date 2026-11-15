//! The strip of attachments above the toolbar, inside the frame (`.attachment-strip` of `Composer.vue`) and the image chip
//! in it (`ImageAttachmentChips.vue`). File chips and terminal chips join the strip in piece O.
use crate::State;
use crate::composer::attach::HeldImage;
use crate::icons::X;
use crate::theme::{RADIUS_BTN, Theme, sans};
use rune::{Cursor, El, SurfaceFit, col, el, icon, row, text};

/// `.attachment-chip__thumb` and the name's `max-width`.
const THUMB: f32 = 80.0;

/// The strip, or nothing when the draft holds no picture.
pub fn strip(s: &State, t: &Theme) -> Option<El> {
    let images = s.held_images();
    if images.is_empty() {
        return None;
    }
    // flex-wrap, gap 8, padding 8px 12px 4px.
    Some(images.iter().fold(row().key("attachment-strip").w_full().flex_wrap().gap(8.0).pt(8.0).px(12.0).pb(4.0), |strip, image| {
        strip.child(chip(s, t, image))
    }))
}

/// `.attachment-chip`: the thumbnail over its name, in a bordered box; the remove button shows while the chip is hovered
/// or has the keyboard.
fn chip(s: &State, t: &Theme, image: &HeldImage) -> El {
    let key = image.preview_key();
    let mut face = el().size(THUMB, THUMB).radius(RADIUS_BTN - 3.0).fill(t.main_bg).group_opacity(1.0, 0.8);
    if let Some((picture, _)) = s.images.ready(&key) {
        face = face.pixels_fit(picture.pixels(), SurfaceFit::Fill);
    }
    let (open, alt, opener) = (key.clone(), image.filename.clone(), format!("attachment-thumb-{}", image.id));
    let thumb = el()
        .key(opener.clone())
        .role("button")
        .label(format!("Preview {}", image.filename))
        .help("Click to preview")
        .hover_group()
        .cursor(Cursor::Pointer)
        .radius(RADIUS_BTN - 3.0)
        .focus_ring(2.0, 2.0, t.accent)
        .on_press(move |s: &mut State| s.open_lightbox(&open, &alt, &opener))
        .child(face);
    let id = image.id.clone();
    let remove = el()
        .key(format!("attachment-remove-{}", image.id))
        .role("button")
        .label(format!("Remove {}", image.filename))
        .help(format!("Remove {}", image.filename))
        .pin(None, Some(4.0), Some(4.0), None)
        .size(18.0, 18.0)
        .center()
        .radius(9.0)
        .fill(t.chip_remove())
        .hover(t.chip_remove_hover())
        .group_opacity(0.0, 1.0)
        .cursor(Cursor::Pointer)
        .focus_ring(2.0, 1.0, t.accent)
        .on_press(move |s: &mut State| s.remove_image(&id))
        .child(icon(&X, 12.0).color(rune::Color::WHITE));
    col()
        .key(format!("attachment-chip-{}", image.id))
        .items_center()
        .gap(4.0)
        .pt(6.0)
        .px(6.0)
        .pb(6.0)
        .border(1.0, t.border)
        .border_layout()
        .radius(RADIUS_BTN)
        .fill(t.panel_bg)
        .hover_group()
        .child(thumb)
        .child(text(image.filename.clone()).font(sans(10.0, 400.0, 1.5)).color(t.muted).max_w(THUMB).truncate())
        .child(remove)
}
