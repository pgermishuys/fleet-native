//! A session tile (`SessionTile.vue`): one session of a machine, as a button. The dot says its state, the chip its machine, and
//! the now line what it is doing this second.
use crate::State;
use crate::fleet::mission::{Tile, TileState};
use crate::theme::{RADIUS_CARD, Theme, mono, sans};
use crate::ui::rem;
use rune::{Color, Cursor, El, col, el, row, text};

fn dot_ink(t: &Theme, state: TileState) -> Color {
    match state {
        TileState::NeedsYou => t.accent,
        TileState::Working => t.running,
        TileState::Idle => t.idle,
        TileState::Error => t.error,
    }
}

/// `w-full rounded-card border bg-main-bg p-3`: the border turns the accent while picked, and 50% of it under the pointer. Rune
/// has no hover border, so the border is a 1 px ring: a `border`-coloured box around the tile's own fill whose colour changes.
pub fn tile(t: &Theme, tile: &Tile, picked: bool) -> El {
    let key = &tile.key;
    let title = row().gap(7.0).center_y().w_full().child(el().size(7.0, 7.0).radius(3.5).fill(dot_ink(t, tile.state))).child(
        text(tile.title.clone())
            .key(format!("tile-title-{key}"))
            .font(sans(rem(0.875), 500.0, 1.43))
            .color(t.text)
            .truncate()
            .grow(1.0)
            .shrink()
            .min_w(0.0),
    );
    let model = tile.model.as_deref().map_or_else(String::new, |m| format!(" · {m}"));
    let chips =
        row()
            .gap(5.25)
            .mt(3.5)
            .flex_wrap()
            .center_y()
            .child(
                row().px(5.25).py(1.0).radius(4.0).border(1.0, t.border).border_layout().child(
                    text(tile.machine_name.clone()).key(format!("tile-machine-{key}")).font(mono(rem(0.75), 400.0, 1.33)).color(t.text),
                ),
            )
            .child(text(format!("{}{model}", tile.harness)).font(sans(rem(0.75), 400.0, 1.33)).color(t.muted));
    let now = row().mt(7.0).w_full().px(7.0).py(3.5).radius(4.0).fill(t.panel_bg).child(
        text(tile.now.clone())
            .key(format!("tile-now-{key}"))
            .font(mono(rem(0.75), 400.0, 1.33))
            .color(t.text)
            .truncate()
            .grow(1.0)
            .shrink()
            .min_w(0.0),
    );
    let mut body = col().w_full().padding(rem(0.75)).child(title).child(chips).child(now);
    if tile.todos.is_some() || tile.helpers > 0 {
        let mut line = row().mt(7.0).gap(10.5).center_y();
        if let Some((done, total)) = tile.todos {
            line = line
                .child(text(format!("{done}/{total}")).key(format!("tile-todos-{key}")).font(sans(rem(0.75), 400.0, 1.33)).color(t.muted));
        }
        if tile.helpers > 0 {
            let noun = if tile.helpers == 1 { "helper" } else { "helpers" };
            line = line.child(
                text(format!("{} {noun}", tile.helpers))
                    .key(format!("tile-helpers-{key}"))
                    .font(sans(rem(0.75), 400.0, 1.33))
                    .color(t.muted),
            );
        }
        body = body.child(line);
    }
    let edge = if picked { t.accent } else { t.border };
    let key = key.clone();
    // The tile's own fill sits one pixel in from the ring, so the ring is the border.
    let face = body.radius(RADIUS_CARD - 1.0).clip_rounded(RADIUS_CARD - 1.0).fill(t.main_bg);
    let mut ring =
        col().key(format!("tile-{key}")).role("button").label(tile.title.clone()).w_full().padding(1.0).radius(RADIUS_CARD).fill(edge);
    if !picked {
        ring = ring.hover(t.mix(t.accent, t.border, 0.5));
    }
    if tile.stale {
        ring = ring.opacity(0.6);
    }
    ring.cursor(Cursor::Pointer).focus_ring(2.0, 2.0, t.accent).child(face).on_press(move |s: &mut State| s.mission_pick(&key))
}
