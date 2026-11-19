//! What a file tab shows besides code (component 11, piece I): an image over its checkerboard, rendered Markdown, and rendered HTML
//! (`FileCanvas.vue`'s `.file-canvas__image` and `.file-canvas__rendered`; rows 7.1 to 7.6 of docs/parity/11-files.md).
use super::prose;
use crate::fleet::session_image::Shown;
use crate::theme::{Theme, sans};
use crate::{State, Tx};
use rune::{El, ScrollSpec, Shape, SurfaceFit, col, el, text};

/// The picture is kept this far from the tab's edges (`padding: 24px`).
const PAD: f32 = 24.0;
/// The checkerboard's tile; the conic gradient fills two of its four quadrants, so the squares are half of it.
const TILE: f32 = 16.0;

/// `.file-canvas__note`: 13 px muted, padding `14px 18px`, said as a status.
pub fn note(t: &Theme, key: &str, words: String) -> El {
    col().role("status").w_full().px(18.0).py(14.0).child(text(words).key(key).font(sans(13.0, 400.0, 1.4)).color(t.muted))
}

/// The squares of `repeating-conic-gradient(--border 0 25%, transparent 0 50%) 0 0 / 16px 16px` over `w` by `h`: the top right and
/// the bottom left quarter of each tile.
fn checkerboard(t: &Theme, w: f32, h: f32) -> Shape {
    let half = TILE / 2.0;
    let mut shape = Shape::new().view_box(w, h);
    let mut y = 0.0;
    while y < h {
        let mut x = 0.0;
        while x < w {
            for (dx, dy) in [(half, 0.0), (0.0, half)] {
                let (x0, y0) = (x + dx, y + dy);
                if x0 < w && y0 < h {
                    let (x1, y1) = ((x0 + half).min(w), (y0 + half).min(h));
                    shape = shape.fill([[x0, y0], [x1, y0], [x1, y1], [x0, y1]], t.border);
                }
            }
            x += TILE;
        }
        y += TILE;
    }
    shape
}

/// Scaled down to fit `room`, never up (`max-width: 100%; max-height: 100%; object-fit: contain`).
pub fn fit(natural: [f32; 2], room: [f32; 2]) -> [f32; 2] {
    let k = (room[0] / natural[0]).min(room[1] / natural[1]).min(1.0);
    if k.is_finite() && k > 0.0 { [(natural[0] * k).round().max(1.0), (natural[1] * k).round().max(1.0)] } else { natural }
}

/// The image of the open file: its note while it opens or can't be shown, or the picture centred in the tab.
pub fn image_body(s: &State, t: &Theme, machine: &str, session: &str, path: &str) -> El {
    match s.session_images.shown(machine, session, path) {
        Shown::Opening => note(t, "file-opening", format!("Opening {path}\u{2026}")),
        Shown::Note(why) => note(t, "file-unavailable", why.to_string()),
        Shown::Picture { image, size, .. } => {
            // Until the tab has been measured, the panel's own width stands for it.
            let [aw, ah] = s.session_images.area;
            let room = if aw > 0.0 && ah > 0.0 { [aw - 2.0 * PAD, ah - 2.0 * PAD] } else { [s.right_shown_w() - 2.0 * PAD, 400.0] };
            let [w, h] = fit(size, [room[0].max(1.0), room[1].max(1.0)]);
            let picture = el().key("file-image-picture").w(w).h(h).outline(1.0, 0.0, t.border).shape(checkerboard(t, w, h)).child(
                el().w(w).h(h).label(path.rsplit('/').next().unwrap_or(path).to_string()).pixels_fit(image.pixels(), SurfaceFit::Fit),
            );
            col()
                .key("file-image")
                .w_full()
                .grow(1.0)
                .min_h(0.0)
                .center()
                .padding(PAD)
                .on_layout(|s: &mut State, [_, _, w, h]| {
                    if s.session_images.area != [w, h] && w > 0.0 {
                        s.session_images.area = [w, h];
                    }
                })
                .child(picture)
        }
    }
}

/// `.file-canvas__rendered`: the text through the Markdown renderer, padding `16px 22px 28px`, scrolling. It shares the Document
/// tab's style (`prose`).
pub fn rendered_markdown(t: &Theme, path: &str, content: &str) -> El {
    // Selecting text asks the agent about it (`FileCanvas.vue` `annotatable`, row 7.5); the popover opens where the drag lets go.
    let owner = path.to_string();
    let text = col()
        .w_full()
        .selection_group(true)
        .on_selection_change(move |s: &mut State, change, tx| s.read_rendered_selection(&owner, change.selection(), change.dragging(), tx))
        .child(prose::prose(&prose::style(t), "file-markdown", content, false));
    col()
        .w_full()
        .pt(16.0)
        .pr(22.0)
        .pb(28.0)
        .pl(22.0)
        .on_pointer_move_local(|s: &mut State, p: rune::PointerEvent, tx: &mut Tx| {
            s.annotation.pointer = (p.window_x, p.window_y);
            tx.live_only();
        })
        .child(text)
        .scroll(ScrollSpec { stick_bottom: false })
        .key("file-rendered")
        .grow(1.0)
        .min_h(0.0)
}

/// Rendered HTML: no padding, in the frame no script runs in (`html_body`).
pub fn rendered_html(s: &State, t: &Theme, content: &str) -> El {
    col().key("file-rendered").w_full().grow(1.0).min_h(0.0).child(crate::web::html_body(s, t, "file-html", content))
}
