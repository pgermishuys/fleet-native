//! The Board's order (`stores/board.ts` 114-223, 342-372): lanes by position, cards by lane then position, and the two
//! optimistic changes the store makes before the server answers, reordering lanes and moving a card. Each returns a new
//! list; the store keeps the old one to put back if the server refuses.
//!
//! Fleet compares ids and times with `localeCompare`; here they compare as text, which orders ISO times and GUIDs the same.
use super::{Card, Lane};
use std::cmp::Ordering;

/// The gap between neighbours after a rebalance (`POSITION_GAP`): the server writes (i+1)*1024 too.
pub const POSITION_GAP: f64 = 1024.0;

fn by_position(a: f64, b: f64) -> Ordering {
    a.partial_cmp(&b).unwrap_or(Ordering::Equal)
}

/// `sortBoardLanes`: by position, then by when each was made.
pub fn sort_lanes(lanes: &mut [Lane]) {
    lanes.sort_by(|a, b| by_position(a.position, b.position).then_with(|| a.created_at.cmp(&b.created_at)));
}

/// `sortBoardCards`: by lane id, then position, then when each was made.
pub fn sort_cards(cards: &mut [Card]) {
    cards.sort_by(|a, b| {
        a.lane_id.cmp(&b.lane_id).then_with(|| by_position(a.position, b.position)).then_with(|| a.created_at.cmp(&b.created_at))
    });
}

pub fn sorted_lanes(lanes: &[Lane]) -> Vec<Lane> {
    let mut out = lanes.to_vec();
    sort_lanes(&mut out);
    out
}

pub fn sorted_cards(cards: &[Card]) -> Vec<Card> {
    let mut out = cards.to_vec();
    sort_cards(&mut out);
    out
}

/// `inboxLane`: the first inbox lane in order (the server keeps at most one).
pub fn inbox_lane(lanes: &[Lane]) -> Option<Lane> {
    sorted_lanes(lanes).into_iter().find(|l| l.is_inbox)
}

/// `lanesWithCards`: every lane in order with its **active** cards, each lane's by position then when made. An archived
/// card is in no lane, and a card whose lane isn't listed is in none either.
pub fn lanes_with_cards(lanes: &[Lane], cards: &[Card]) -> Vec<(Lane, Vec<Card>)> {
    sorted_lanes(lanes)
        .into_iter()
        .map(|lane| {
            let mut own: Vec<Card> = cards.iter().filter(|c| !c.is_archived() && c.lane_id == lane.id).cloned().collect();
            own.sort_by(|a, b| by_position(a.position, b.position).then_with(|| a.created_at.cmp(&b.created_at)));
            (lane, own)
        })
        .collect()
}

/// `patchLane`: the server's answer for one lane replaces it (or joins the list), and the list is sorted again.
pub fn patch_lane(lanes: &[Lane], next: Lane) -> Vec<Lane> {
    let mut out = lanes.to_vec();
    match out.iter().position(|l| l.id == next.id) {
        Some(i) => out[i] = next,
        None => out.push(next),
    }
    sort_lanes(&mut out);
    out
}

/// `patchCard`: the server's answer for one card replaces it (or joins the list), and the list is sorted again.
pub fn patch_card(cards: &[Card], next: Card) -> Vec<Card> {
    let mut out = cards.to_vec();
    match out.iter().position(|c| c.id == next.id) {
        Some(i) => out[i] = next,
        None => out.push(next),
    }
    sort_cards(&mut out);
    out
}

/// `reorderLaneCollection`: the named lanes first in the order given (an unknown id is skipped), then the others in their
/// order, every one renumbered (i+1)*1024. The list comes back in its new order.
pub fn reorder_lanes(lanes: &[Lane], ids: &[String]) -> Vec<Lane> {
    let named = ids.iter().filter_map(|id| lanes.iter().find(|l| &l.id == id).cloned());
    let rest = sorted_lanes(lanes).into_iter().filter(|l| !ids.contains(&l.id));
    named
        .chain(rest)
        .enumerate()
        .map(|(i, mut lane)| {
            lane.position = (i + 1) as f64 * POSITION_GAP;
            lane
        })
        .collect()
}

/// `moveCardCollection`: takes the card out of its lane and puts it at `index` (clamped to the lane's length) of the target
/// lane, then renumbers the active cards of both lanes (i+1)*1024. Archived cards keep their place and number; an unknown
/// card changes nothing.
pub fn move_card(cards: &[Card], id: &str, lane: &str, index: usize) -> Vec<Card> {
    let mut out = cards.to_vec();
    let Some(moving) = out.iter().position(|c| c.id == id) else { return out };
    let from = out[moving].lane_id.clone();
    // Each lane's active cards (the moving one left out), as indexes into `out`, in the list's order: the store's lists are
    // kept sorted, so that is each lane's order.
    let in_lane = |lane_id: &str| -> Vec<usize> {
        out.iter().enumerate().filter(|(_, c)| !c.is_archived() && c.id != id && c.lane_id == lane_id).map(|(i, _)| i).collect()
    };
    let mut target = in_lane(lane);
    target.insert(index.min(target.len()), moving);
    let source = if from == lane { Vec::new() } else { in_lane(&from) };
    out[moving].lane_id = lane.to_string();
    for order in [source, target] {
        for (i, at) in order.into_iter().enumerate() {
            out[at].position = (i + 1) as f64 * POSITION_GAP;
        }
    }
    sort_cards(&mut out);
    out
}
