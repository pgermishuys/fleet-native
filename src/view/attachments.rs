//! The strip of attachments above the toolbar, inside the frame (`.attachment-strip` of `Composer.vue`) with the image chip
//! (`ImageAttachmentChips.vue`) and the file chip in it, the strip of terminal lines under it, and the bar that offers a
//! large paste as a file (`.paste-offer`).
use crate::State;
use crate::composer::attach::HeldImage;
use crate::composer::attached::{DraftFile, TerminalDraft, describe_draft_file, describe_paste, file_extension_label, terminal_line_range};
use crate::fleet::uploads::Choice;
use crate::icons::{SQUARE_TERMINAL, X};
use crate::theme::{RADIUS_BTN, Theme, mono, sans};
use rune::{Cursor, El, SurfaceFit, col, el, icon, row, text};

/// `.attachment-chip__thumb` and the name's `max-width`.
const THUMB: f32 = 80.0;

/// The strip, or nothing when the draft holds no picture and no file.
pub fn strip(s: &State, t: &Theme) -> Option<El> {
    let (images, files) = (s.held_images(), s.held_files());
    if images.is_empty() && files.is_empty() {
        return None;
    }
    // flex-wrap, gap 8, padding 8px 12px 4px.
    let strip = row().key("attachment-strip").w_full().flex_wrap().gap(8.0).pt(8.0).px(12.0).pb(4.0);
    let strip = images.iter().fold(strip, |strip, image| strip.child(chip(s, t, image)));
    Some(files.iter().fold(strip, |strip, file| strip.child(file_chip(s, t, file))))
}

/// `.attachment-chip--file`: the extension in a box, the name, what it holds, and a remove button that is always there.
fn file_chip(s: &State, t: &Theme, file: &DraftFile) -> El {
    let id = file.id.clone();
    let remove = el()
        .key(format!("file-remove-{}", file.id))
        .role("button")
        .label(format!("Remove {}", file.name))
        .help(format!("Remove {}", file.name))
        .size(14.0, 14.0)
        .center()
        .hover_group()
        .opacity(if s.uploading { 0.5 } else { 1.0 })
        .child(icon(&X, 12.0).color(t.muted).group_color(t.text));
    let remove = if s.uploading {
        remove.disabled(true)
    } else {
        remove.cursor(Cursor::Pointer).focus_ring(2.0, 1.0, t.accent).on_press(move |s: &mut State| s.remove_file(&id))
    };
    row()
        .key(format!("file-chip-{}", file.id))
        .center_y()
        .gap(8.0)
        .px(8.0)
        .py(4.0)
        .border(1.0, t.border)
        .border_layout()
        .radius(RADIUS_BTN)
        .fill(t.panel_bg)
        .child(
            row()
                .px(4.0)
                .border(1.0, t.border)
                .border_layout()
                .radius(3.0)
                .child(text(file_extension_label(&file.name)).font(mono(9.0, 400.0, 1.5)).color(t.muted)),
        )
        .child(text(file.name.clone()).font(sans(11.0, 400.0, 1.5)).color(t.text).max_w(200.0).truncate())
        .child(text(describe_draft_file(file.size, file.lines)).font(sans(10.0, 400.0, 1.5)).color(t.muted))
        .child(remove)
}

/// `.terminal-context-strip`: one chip per run of terminal lines in the message, or nothing.
pub fn terminal_strip(s: &State, t: &Theme) -> Option<El> {
    let lines = s.terminal_lines();
    if lines.is_empty() {
        return None;
    }
    let strip =
        row().key("terminal-context-strip").label("Terminal lines in this message").w_full().flex_wrap().gap(6.0).pt(8.0).px(12.0).pb(2.0);
    Some(lines.iter().fold(strip, |strip, line| strip.child(terminal_chip(t, line))))
}

/// `.terminal-context-chip`: the terminal's name and the lines taken, in the accent; the lines' text is the tooltip.
fn terminal_chip(t: &Theme, line: &TerminalDraft) -> El {
    let range = terminal_line_range(line.from, line.to);
    let id = line.id.clone();
    let remove = el()
        .key(format!("terminal-remove-{}", line.id))
        .role("button")
        .label(format!("Remove {} {range}", line.label))
        .size(18.0, 18.0)
        .center()
        .radius(4.0)
        .opacity(0.7)
        .hover(t.accent.alpha(0.18))
        .group_opacity(0.7, 1.0)
        .cursor(Cursor::Pointer)
        .focus_ring(2.0, 1.0, t.accent)
        .on_press(move |s: &mut State| s.remove_terminal_lines(&id))
        .child(icon(&X, 11.0).color(t.accent_text));
    row()
        .key(format!("terminal-chip-{}", line.id))
        .help(line.text.clone())
        .h(24.0)
        .pl(8.0)
        .pr(4.0)
        .gap(6.0)
        .center_y()
        .radius(RADIUS_BTN - 1.0)
        .fill(t.accent_dim)
        .hover_group()
        .child(icon(&SQUARE_TERMINAL, 12.0).color(t.accent_text))
        .child(text(line.label.clone()).font(sans(12.0, 500.0, 1.5)).color(t.accent_text))
        .child(text(range).font(mono(11.0, 400.0, 1.5)).color(t.accent_text).opacity(0.85))
        .child(remove)
}

/// `.paste-offer`: the pill above the frame that asks what a large paste should be. The buttons never take the keyboard: it
/// stays in the text area, where Enter, Tab and Esc reach the bar.
pub fn paste_offer(s: &State, t: &Theme) -> Option<El> {
    let offer = s.offer_view()?;
    let choice = |key: &'static str, label: &'static str, hint: &'static str, mine: Choice| {
        let on = offer.choice == mine;
        let (ink, fill) = if on { (t.primary_fg, t.accent) } else { (t.text, rune::Color::TRANSPARENT) };
        row()
            .key(key)
            .role("button")
            .label(label)
            .px(10.0)
            .py(3.0)
            .radius(999.0)
            .fill(fill)
            .gap(4.0)
            .center_y()
            .cursor(Cursor::Pointer)
            .on_press_tx(move |s: &mut State, tx| s.choose_offer(mine, tx))
            .child(text(label).font(sans(12.0, 400.0, 1.5)).color(ink))
            .child(text(hint).font(sans(10.0, 400.0, 1.5)).color(ink).opacity(0.6))
    };
    Some(
        row()
            .key("paste-offer")
            .label("Large paste")
            // Fleet's pill is a block-level flex box: it spans the composer's whole content width (24 px padding each side),
            // not the frame's 760 px column, so in a wide window it reaches 14 px past the frame on each side.
            .w_full()
            .center_y()
            .gap(4.0)
            .mb(6.0)
            .px(3.0)
            .py(3.0)
            .border(1.0, t.border)
            .border_layout()
            .radius(999.0)
            .fill(t.panel_bg)
            .child(text(describe_paste(&offer.text)).font(sans(12.0, 400.0, 1.5)).color(t.muted).px(8.0))
            .child(choice("paste-offer-attach", "Attach as file", "Enter", Choice::Attach))
            .child(choice("paste-offer-text", "Paste as text", "Esc", Choice::Text)),
    )
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
