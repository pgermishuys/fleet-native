//! A machine's view (`MissionControlScreen.vue`, the machine half): its name and the Group by switch, then its sessions as tiles in
//! groups, with the peek of the picked one beside them. While the machine hasn't answered it says it is asking; when it can't be
//! reached it says that, not that it has no sessions.
use super::{peek, tile};
use crate::State;
use crate::fleet::mission::peeking::selected;
use crate::fleet::mission::wall::Wall;
use crate::fleet::mission::{FeedStatus, Grouping, Tile, group_tiles};
use crate::icons::RADAR;
use crate::theme::{RADIUS_CARD, Theme, sans};
use crate::ui::{empty_state, error_note, rem};
use crate::view::files_tab::spinner;
use crate::view::pages::TWO_COLUMNS_FROM;
use rune::{Cursor, El, ScrollSpec, col, el, row, text};

/// `p-4 text-sm text-muted`: the address names a machine that isn't in the list.
pub fn gone(t: &Theme) -> El {
    row()
        .key("fleet-machine-gone")
        .w_full()
        .padding(rem(1.0))
        .child(text("That machine isn't in the list any more.").font(sans(rem(0.875), 400.0, 1.43)).color(t.muted))
}

/// The segmented "Group by" control: Folder and State (`GROUPINGS`).
fn group_by(s: &State, t: &Theme) -> El {
    let mut group = row()
        .key("fleet-group-by")
        .role("group")
        .label("Group by")
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .clip_rounded(RADIUS_CARD);
    for (value, name, words) in [(Grouping::Folder, "folder", "Folder"), (Grouping::State, "state", "State")] {
        let on = s.mission.grouping == value;
        let mut b = row()
            .key(format!("group-{name}"))
            .role("button")
            .label(words)
            .checked(on)
            .px(rem(0.625))
            .py(rem(0.25))
            .cursor(Cursor::Pointer)
            .focus_ring(2.0, -2.0, t.accent)
            .hover_group()
            .child(
                text(words)
                    .font(sans(rem(0.75), if on { 500.0 } else { 400.0 }, 1.33))
                    .color(if on { t.accent_text } else { t.muted })
                    .group_color(if on { t.accent_text } else { t.text }),
            );
        if on {
            b = b.fill(t.accent_dim);
        }
        group = group.child(b.on_press(move |s: &mut State| s.mission.grouping = value));
    }
    group
}

/// Tiles in a grid: one column, two from `xl`, gap 12.
fn grid(s: &State, t: &Theme, tiles: &[Tile], picked: Option<&str>) -> El {
    let columns = if s.ui.sel.window.0 >= TWO_COLUMNS_FROM { 2 } else { 1 };
    let mut grid = col().w_full().gap(10.5);
    for line in tiles.chunks(columns) {
        let mut cells = row().w_full().gap(10.5).items_start();
        for entry in line {
            cells = cells.child(col().grow(1.0).w(0.0).child(tile::tile(t, entry, picked == Some(entry.key.as_str()))));
        }
        for _ in line.len()..columns {
            cells = cells.child(el().grow(1.0).w(0.0));
        }
        grid = grid.child(cells);
    }
    grid
}

/// What the page says in place of tiles, or nothing when there are some.
fn status(t: &Theme, name: &str, asking: bool, unreachable: bool) -> El {
    if asking {
        return row()
            .key("fleet-machine-loading")
            .label(format!("Asking {name} for its sessions…"))
            .w_full()
            .justify_center()
            .padding(rem(1.5))
            .child(spinner(t.muted, 20.0));
    }
    if unreachable {
        return error_note::message(t, "fleet-machine-unreachable", &format!("Couldn't reach {name}."));
    }
    empty_state::bordered_icon(
        t,
        "fleet-machine-empty",
        &RADAR,
        "No sessions are working or waiting on this machine",
        "Start one from Sessions.",
    )
}

/// The machine `id`'s view: its sessions grouped, and the peek of the picked one.
pub fn view(s: &State, t: &Theme, wall: &Wall, id: &str) -> El {
    let Some(entry) = s.machines.entries().into_iter().find(|e| e.key == id) else { return gone(t) };
    let tiles: Vec<Tile> = wall.tiles.iter().filter(|tile| tile.machine_id == id).cloned().collect();
    let state = wall.machines.iter().find(|m| m.id == id);
    let asking = state.is_none_or(|m| m.asking);
    let unreachable = state.is_some_and(|m| m.status == FeedStatus::Unreachable);
    let shown = selected(s, wall);

    let head = row()
        .gap(7.0)
        .center_y()
        .mb(10.5)
        .child(text(entry.name.clone()).key("fleet-machine-name").font(sans(rem(0.875), 600.0, 1.43)).color(t.text).mr(7.0))
        .child(text("Group by").font(sans(rem(0.75), 400.0, 1.33)).color(t.muted))
        .child(group_by(s, t));
    let mut list = col()
        .key("fleet-machine-list")
        .grow(1.0)
        .shrink()
        .min_w(0.0)
        .h_full()
        .min_h(0.0)
        .padding(rem(1.0))
        .scroll(ScrollSpec { stick_bottom: false })
        .child(head);
    if tiles.is_empty() {
        list = list.child(status(t, &entry.name, asking, unreachable));
    }
    for group in group_tiles(&tiles, s.mission.grouping) {
        let heading = row()
            .gap(3.5)
            .center_y()
            .mb(7.0)
            .child(
                text(group.label.clone())
                    .key(format!("mission-group-label-{}", group.key))
                    .font(sans(rem(0.875), 600.0, 1.43))
                    .color(t.text),
            )
            .child(
                text(group.tiles.len().to_string())
                    .key(format!("mission-group-count-{}", group.key))
                    .font(sans(rem(0.75), 400.0, 1.33))
                    .color(t.muted),
            );
        list = list.child(col().key(format!("mission-group-{}", group.key)).w_full().mb(17.5).child(heading).child(grid(
            s,
            t,
            &group.tiles,
            shown.map(|tile| tile.key.as_str()),
        )));
    }
    let mut page = row().key("fleet-machine-view").grow(1.0).min_h(0.0).w_full().child(list);
    if let Some(tile) = shown {
        page = page.child(peek::panel(s, t, tile));
    }
    page
}
