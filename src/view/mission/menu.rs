//! The fleet-menu (`FleetNavPanel.vue`, `use-fleet-menu.ts`): the Fleet view's panel. Needs you first under "Attention", then one
//! entry per machine under "Machines", each with how many sessions it holds. The one the address names is marked; pressing
//! another opens its route, pressing the open one does nothing.
use crate::State;
use crate::fleet::labs::{FleetView, fleet_view_path};
use crate::fleet::mission::FeedStatus;
use crate::fleet::mission::wall::Wall;
use crate::theme::{RADIUS_BTN, Theme, sans};
use crate::ui::rem;
use rune::{Cursor, El, ScrollSpec, col, row, text};

/// One entry of the menu (`FleetMenuItem`).
pub struct Item {
    pub key: String,
    pub label: String,
    pub view: FleetView,
    pub count: usize,
    pub unreachable: bool,
}

/// Needs you, then one per machine in the machines list's order (home first).
pub fn items(s: &State, wall: &Wall) -> (Item, Vec<Item>) {
    let attention = Item {
        key: "needs-you".into(),
        label: "Needs you".into(),
        view: FleetView::NeedsYou,
        count: wall.waiting.len(),
        unreachable: false,
    };
    let machines = s
        .machines
        .entries()
        .into_iter()
        .map(|entry| Item {
            key: format!("machine-{}", entry.key),
            label: entry.name,
            view: FleetView::Machine(entry.key.clone()),
            count: wall.tiles.iter().filter(|tile| tile.machine_id == entry.key).count(),
            unreachable: wall.machines.iter().any(|m| m.id == entry.key && m.status == FeedStatus::Unreachable),
        })
        .collect();
    (attention, machines)
}

fn label(t: &Theme, words: &str) -> El {
    row().px(12.0).mt(8.0).mb(2.0).child(text(words.to_uppercase()).font(sans(rem(0.71429), 600.0, 1.5).tracking(0.5)).color(t.muted))
}

fn item(s: &State, t: &Theme, entry: &Item) -> El {
    let open = s.labs.fleet_view == entry.view;
    let count_ink = if entry.count > 0 && entry.view == FleetView::NeedsYou { t.accent_text } else { t.muted };
    let count_weight = if entry.count > 0 && entry.view == FleetView::NeedsYou { 600.0 } else { 400.0 };
    let mut b = row()
        .key(format!("fleet-view-{}", entry.key))
        .role("button")
        .label(entry.label.clone())
        .w_full()
        .min_h(36.0)
        .px(12.0)
        .gap(10.0)
        .center_y()
        .radius(RADIUS_BTN)
        .border(1.0, if open { t.accent.alpha(0.4) } else { rune::Color::TRANSPARENT })
        .border_layout()
        .cursor(Cursor::Pointer)
        .focus_ring(2.0, 2.0, t.accent)
        .child(
            text(entry.label.clone())
                .font(sans(rem(0.85714), 500.0, 1.4))
                .color(if open { t.text } else { t.muted })
                .truncate()
                .grow(1.0)
                .shrink()
                .min_w(0.0),
        );
    if entry.unreachable {
        b = b.child(text("offline").key(format!("fleet-view-{}-offline", entry.key)).font(sans(rem(0.71429), 400.0, 1.4)).color(t.muted));
    }
    b = b.child(
        text(entry.count.to_string()).key(format!("fleet-view-{}-count", entry.key)).font(sans(11.0, count_weight, 1.4)).color(count_ink),
    );
    b = if open { b.fill(t.mix(t.accent, t.panel_bg, 0.12)) } else { b.hover(t.main_bg) };
    let view = entry.view.clone();
    b.on_press(move |s: &mut State| {
        if s.labs.fleet_view != view {
            s.open_fleet_address(&fleet_view_path(&view));
        }
    })
}

/// `section` "Fleet navigation".
pub fn panel(s: &State, t: &Theme, wall: &Wall) -> El {
    let (attention, machines) = items(s, wall);
    let header = row().px(4.0).pt(18.0).pb(10.0).child(text("FLEET").font(sans(rem(0.71429), 600.0, 1.5).tracking(0.5)).color(t.muted));
    let nav = col()
        .key("fleet-views")
        .label("Fleet views")
        .w_full()
        .child(
            col()
                .key("fleet-views-attention")
                .label("Attention")
                .w_full()
                .gap(4.0)
                .mb(8.0)
                .child(label(t, "Attention"))
                .child(item(s, t, &attention)),
        )
        .child(
            col()
                .key("fleet-views-machines")
                .label("Machines")
                .w_full()
                .gap(4.0)
                .mb(8.0)
                .child(label(t, "Machines"))
                .children(machines.iter().map(|m| item(s, t, m))),
        );
    col()
        .key("fleet-menu")
        .label("Fleet navigation")
        .w(s.sidebar_w)
        .h_full()
        .min_h(0.0)
        .px(12.0)
        .pb(12.0)
        .scroll(ScrollSpec { stick_bottom: false })
        .child(header)
        .child(nav)
}
