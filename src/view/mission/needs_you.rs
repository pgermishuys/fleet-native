//! The Needs you column (`NeedsYouColumn.vue`): a card for every session that waits on the user, answered where it sits. A
//! question with one pick and nothing after it answers with its options; a permission answers Allow once, Always or Deny; a
//! card whose machine said no says why under it.
use crate::State;
use crate::fleet::asks::Reply;
use crate::fleet::mission::wall::Wall;
use crate::fleet::mission::{Ask, Tile};
use crate::theme::{RADIUS_CARD, Theme, mono, sans};
use crate::ui::button::{Size, Variant, button, disabled};
use crate::ui::{empty_state, error_note, rem};
use crate::view::files_tab::spinner;
use rune::{El, ScrollSpec, col, el, row, text};

/// `aside` "Needs you": the heading with its count, then the cards, the spinner or the empty note.
pub fn column(s: &State, t: &Theme, wall: &Wall) -> El {
    let heading = row()
        .gap(7.0)
        .center_y()
        .child(text("Needs you").font(sans(rem(0.875), 600.0, 1.43)).color(t.text))
        .child(text(wall.waiting.len().to_string()).key("needs-you-count").font(sans(rem(0.875), 600.0, 1.43)).color(t.accent_text));
    let mut body = col()
        .key("mission-needs-you")
        .label("Needs you")
        .grow(1.0)
        .shrink()
        .min_w(0.0)
        .h_full()
        .min_h(0.0)
        .gap(10.5)
        .padding(rem(1.0))
        .scroll(ScrollSpec { stick_bottom: false })
        .child(heading);
    if wall.waiting.is_empty() && wall.all_asked {
        body = body.child(
            row()
                .key("mission-needs-you-loading")
                .label("Asking your machines what's waiting…")
                .w_full()
                .justify_center()
                .padding(rem(1.0))
                .child(spinner(t.muted, 20.0)),
        );
    } else if wall.waiting.is_empty() {
        body = body.child(empty_state::bordered_compact(t, "mission-needs-you-empty", "Nothing is waiting on you"));
    }
    body.children(wall.waiting.iter().map(|tile| card(s, t, tile)))
}

/// One card: `rounded-card border border-l-4 border-l-accent bg-main-bg p-3`.
fn card(s: &State, t: &Theme, tile: &Tile) -> El {
    let key = &tile.key;
    let busy = s.mission.busy.contains(key);
    let mut chips = row().gap(5.25).mt(3.5).flex_wrap().center_y().child(
        row()
            .px(5.25)
            .py(1.0)
            .radius(4.0)
            .border(1.0, t.border)
            .border_layout()
            .child(text(tile.machine_name.clone()).key(format!("ask-machine-{key}")).font(mono(rem(0.75), 400.0, 1.33)).color(t.text)),
    );
    if matches!(tile.ask, Some(Ask::Permission(_))) {
        chips = chips.child(text("wants to run").font(sans(rem(0.75), 400.0, 1.33)).color(t.muted));
    }
    let mut inner = col()
        .grow(1.0)
        .shrink()
        .min_w(0.0)
        .px(rem(0.75))
        // The card's 1 px top and bottom border takes room in Fleet's layout; here it is only painted.
        .py(rem(0.75) + 1.0)
        .child(text(tile.title.clone()).key(format!("ask-title-{key}")).font(sans(rem(0.875), 500.0, 1.43)).color(t.text).wrap().w_full())
        .child(chips);
    inner = match &tile.ask {
        Some(Ask::Question { request_id, question, more }) => {
            let mut body = col().child(
                text(question.question.clone())
                    .key(format!("ask-question-{key}"))
                    .font(sans(rem(0.875), 400.0, 1.43))
                    .color(t.text)
                    .wrap()
                    .w_full()
                    .mt(7.0),
            );
            if !question.multiple && *more == 0 {
                let options = question.options.iter().enumerate().map(|(i, option)| {
                    let (tile, request_id, label) = (tile.clone(), request_id.clone(), option.label.clone());
                    let b = button(t, Variant::Outline, Size::Sm, None, &option.label).key(format!("ask-option-{key}-{i}"));
                    let b = if option.description.is_empty() { b } else { b.help(option.description.clone()) };
                    if busy { disabled(b) } else { b.on_press(move |s: &mut State| s.mission_answer_question(&tile, &request_id, &label)) }
                });
                body = body.child(row().gap(5.25).mt(7.0).flex_wrap().children(options));
            } else {
                body = body.child(open_button(t, tile, "Open to answer").mt(7.0));
            }
            inner.child(body)
        }
        Some(Ask::Permission(ask)) => {
            let shown = ask.title.clone().filter(|title| !title.is_empty()).unwrap_or_else(|| ask.tool.clone());
            let reply = |label: &str, variant: Variant, reply: Reply, id: &str| {
                let tile = tile.clone();
                let b = button(t, variant, Size::Sm, None, label).key(format!("ask-{id}-{key}"));
                if busy { disabled(b) } else { b.on_press(move |s: &mut State| s.mission_answer_permission(&tile, reply)) }
            };
            inner
                .child(
                    row()
                        .mt(7.0)
                        .w_full()
                        .px(7.0)
                        .py(3.5)
                        .radius(4.0)
                        .fill(t.panel_bg)
                        .child(text(shown).key(format!("ask-code-{key}")).font(mono(rem(0.75), 400.0, 1.33)).color(t.text).wrap().w_full()),
                )
                .child(
                    row()
                        .gap(5.25)
                        .mt(7.0)
                        .flex_wrap()
                        .child(reply("Allow once", Variant::Default, Reply::Once, "once"))
                        .child(reply("Always", Variant::Outline, Reply::Always, "always"))
                        .child(reply("Deny", Variant::Outline, Reply::Reject, "reject")),
                )
        }
        None => inner
            .child(
                text(tile.now.clone())
                    .key(format!("ask-now-{key}"))
                    .font(sans(rem(0.875), 400.0, 1.43))
                    .color(t.muted)
                    .wrap()
                    .w_full()
                    .mt(7.0),
            )
            .child(open_button(t, tile, "Open").mt(7.0)),
    };
    if let Some(words) = s.mission.errors.get(key).filter(|w| !w.is_empty()) {
        inner = inner.child(row().mt(7.0).w_full().child(error_note::compact(t, &format!("ask-error-{key}"), words)));
    }
    row()
        .key(format!("ask-{key}"))
        .w_full()
        .radius(RADIUS_CARD)
        .clip_rounded(RADIUS_CARD)
        .fill(t.main_bg)
        .border_sides([1.0, 1.0, 1.0, 0.0], t.border)
        .child(el().w(4.0).h_full().fill(t.accent))
        .child(inner)
}

/// A small outline button that opens the tile's session.
fn open_button(t: &Theme, tile: &Tile, label: &str) -> El {
    let tile = tile.clone();
    let key = format!("ask-open-{}", tile.key);
    row().child(
        button(t, Variant::Outline, Size::Sm, None, label)
            .key(key)
            .on_press_tx(move |s: &mut State, tx: &mut rune::Tx| s.mission_open_tile(&tile, tx)),
    )
}
