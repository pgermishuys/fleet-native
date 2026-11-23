//! `BoardActivityPanel.vue`: Fleet's invented activity feed, as decision D1 has it (FB4): four made-up sessions' events, the newest
//! four at once and one more every 2.5 s, newest first. Nothing here reads a real session.
use crate::State;
use crate::conv::time::{civil, local_offset_secs};
use crate::fleet::board::panel::{Entry, Tone};
use crate::theme::{Theme, sans};
use crate::ui::empty_state;
use rune::{El, col, row, text};

/// `formatTime(ms, { seconds: true })`, en-US: "3:05:27 PM".
fn clock_seconds(ms: i64) -> String {
    let offset = local_offset_secs();
    let (_, _, _, hour, minute) = civil(ms, offset);
    let second = (ms + offset * 1000).rem_euclid(60_000) / 1000;
    format!("{}:{minute:02}:{second:02} {}", if hour % 12 == 0 { 12 } else { hour % 12 }, if hour < 12 { "AM" } else { "PM" })
}

fn line(words: String, ink: rune::Color) -> El {
    text(words).font(sans(10.0, 400.0, 1.5)).color(ink).wrap()
}

fn item(t: &Theme, e: &Entry) -> El {
    let ink = match e.tone {
        Tone::Running => t.accent_text,
        Tone::Complete => t.complete_text,
        Tone::Error => t.error_text,
        Tone::Queued => t.queued_text,
        Tone::Muted => t.muted,
    };
    let mut c = col()
        .key(format!("board-{}", e.id))
        .w_full()
        .py(8.0)
        .border_sides([0.0, 0.0, 1.0, 0.0], t.board_activity_rule())
        .border_layout()
        .child(
            row()
                .w_full()
                .items_start()
                .gap(8.0)
                .child(line(clock_seconds(e.occurred_at), t.board_dim()))
                .child(text(e.title.clone()).font(sans(10.0, 500.0, 1.5)).color(t.text).wrap().min_w(0.0)),
        )
        .child(line(e.action.clone(), ink).mt(2.0));
    if let Some(project) = &e.project {
        c = c.child(line(project.clone(), t.board_dim()).mt(2.0));
    }
    c
}

pub fn view(s: &State, t: &Theme) -> El {
    let feed = col().key("board-activity").label("Board activity feed").w_full();
    let Some(entries) = s.board.right.feed.as_ref().map(|f| f.ordered()).filter(|e| !e.is_empty()) else {
        return feed.child(empty_state::compact_bare(t, "board-activity-empty", "Waiting for activity events"));
    };
    feed.children(entries.into_iter().map(|e| item(t, e)))
}
