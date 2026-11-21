//! The peek (`PeekPanel.vue`): what the picked tile's session did last, its running helpers, and a field to nudge it with. It sits
//! beside the list of tiles, 320 wide.
use crate::State;
use crate::fleet::mission::Tile;
use crate::fleet::mission::peeking::Sent;
use crate::theme::{RADIUS_CARD, Theme, mono, sans};
use crate::ui::button::{Size, Variant, button};
use crate::ui::text_menu::text_input;
use crate::ui::{empty_state, error_note, rem};
use rune::{El, ScrollSpec, col, row, text};

fn heading(t: &Theme, words: &str) -> El {
    text(words.to_uppercase()).font(sans(rem(0.7), 600.0, 1.43).tracking(0.245)).color(t.muted)
}

/// `aside` "Peek": `w-80 border-l p-4 gap-4`, scrolling.
pub fn panel(s: &State, t: &Theme, tile: &Tile) -> El {
    let peek = &s.mission.peek;
    let words =
        [Some(tile.harness.as_str()), tile.model.as_deref(), tile.branch.as_deref()].into_iter().flatten().collect::<Vec<_>>().join(" · ");
    let head = col()
        .child(
            text(format!("{} · {}", tile.state.label(), tile.machine_name))
                .key("peek-state")
                .font(sans(rem(0.75), 400.0, 1.33))
                .color(t.muted)
                .wrap()
                .w_full(),
        )
        .child(text(tile.title.clone()).key("peek-title").font(sans(rem(1.0), 600.0, 1.5)).color(t.text).wrap().w_full().mt(3.5))
        .child(text(words).key("peek-facts").font(sans(rem(0.75), 400.0, 1.33)).color(t.muted).wrap().w_full().mt(3.5));

    let mut events = col().key("peek-events").w_full().gap(5.25).mt(7.0);
    for event in &peek.events {
        events = events.child(
            text(event.text.clone())
                .key(format!("peek-event-{}", event.key))
                .font(mono(rem(0.75), 400.0, 1.33))
                .color(t.text)
                .truncate()
                .w_full(),
        );
    }
    if peek.events.is_empty() {
        events = events.child(empty_state::compact_bare(t, "peek-nothing", "Nothing yet"));
    }
    let just_now = col().w_full().child(heading(t, "Just now")).child(events);

    let mut body = col().w_full().gap(14.0).child(head).child(just_now);
    if !peek.helpers.is_empty() {
        let list = col().key("peek-helpers").w_full().gap(5.25).mt(7.0).children(peek.helpers.iter().map(|h| {
            let mut line =
                row().gap(4.0).w_full().flex_wrap().child(text(h.title.clone()).font(sans(rem(0.75), 600.0, 1.33)).color(t.text));
            if let Some(label) = &h.label {
                line = line.child(text(label.clone()).font(sans(rem(0.75), 400.0, 1.33)).color(t.muted));
            }
            row()
                .key(format!("peek-helper-{}", h.id))
                .w_full()
                .px(7.0)
                .py(5.25)
                .radius(RADIUS_CARD)
                .border(1.0, t.border)
                .border_layout()
                .child(line)
        }));
        body = body.child(col().w_full().child(heading(t, "Helpers")).child(list));
    }

    let field = text_input(&peek.nudge, sans(rem(0.875), 400.0, 1.43))
        .key("peek-nudge")
        .single_line()
        .label("Nudge this session")
        .placeholder("Nudge it…", t.muted)
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .w_full()
        .px(10.5)
        .py(7.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .fill(t.main_bg)
        .focus_ring(1.0, -1.0, t.accent)
        .on_edit(|s: &mut State| &mut s.mission.peek.nudge)
        .on_submit(|s: &mut State, _| s.peek_nudge());
    let mut foot = col().w_full().gap(7.0).child(field);
    match peek.sent {
        Some(Sent::Sent) => {
            foot = foot.child(text("Sent.").key("peek-sent").font(sans(rem(0.75), 400.0, 1.33)).color(t.muted));
        }
        Some(Sent::Queued) => {
            foot =
                foot.child(text("Queued: it goes when the turn ends.").key("peek-sent").font(sans(rem(0.75), 400.0, 1.33)).color(t.muted));
        }
        None => {}
    }
    if let Some(words) = &peek.error {
        foot = foot.child(error_note::compact(t, "peek-error", words));
    }
    let key = tile.clone();
    foot = foot.child(
        row().child(
            button(t, Variant::Default, Size::Sm, None, "Open")
                .key("peek-open")
                .on_press_tx(move |s: &mut State, tx: &mut rune::Tx| s.mission_open_tile(&key, tx)),
        ),
    );

    // `mt-auto`: the field and Open sit at the bottom when the page is short.
    col()
        .key("mission-peek")
        .label("Peek")
        .w(rem(20.0))
        .h_full()
        .min_h(0.0)
        .padding(rem(1.0))
        .gap(14.0)
        .border_sides([0.0, 0.0, 0.0, 1.0], t.border)
        .border_layout()
        .scroll(ScrollSpec { stick_bottom: false })
        .child(body)
        .child(col().grow(1.0))
        .child(foot)
}
