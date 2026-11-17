//! Ports `components/canvas/TurnsCanvas.vue`: the Turns tab. The summary bar, the list with its loading and empty states,
//! a turn's row, and the older-turns line. What a row says is worked out in `fleet::turns`; the open turn's detail (the
//! files it wrote and their diffs, "Also ran", sub-agents, the jump to the chat) is in `view::turn_detail`.
use crate::State;
use crate::fleet::turns::{SessionTurn, additions_text, deletions_text, facts, model_display_name, prompt_line, turns_text};
use crate::fleet::turns_tab::TurnsTab;
use crate::icons::{CHEVRON_RIGHT, HISTORY, LOADER_CIRCLE};
use crate::theme::{RADIUS_CARD, Theme, mono, sans};
use crate::view::conversation::Older;
use rune::{Align, Cursor, Cycle, El, col, el, icon, row, text};
use std::f32::consts::FRAC_PI_2;

/// The tab's body for the open session.
pub fn turns_tab(s: &State, t: &Theme) -> El {
    let tab = &s.turns_tab;
    let mut frame = col().key("turns-tab").role("region").label("Turns").w_full().h_full().min_h(0.0).items_stretch();
    frame = frame.child(summary(t, tab)).child(list(s, t));
    frame.on_layout(|s: &mut State, [_, _, w, _]| s.turns_tab.width = w)
}

/// A number or a label in the bar's mono face: tabular figures.
fn figure(words: String, ink: rune::Color) -> El {
    text(words).font(mono(11.5, 400.0, 1.5).features("'tnum' 1")).color(ink)
}

fn summary(t: &Theme, tab: &TurnsTab) -> El {
    let on = tab.changed_only;
    let filter = row()
        .key("turns-filter")
        .role("button")
        .label("Only turns that changed files")
        .h(26.0)
        .px(10.0)
        .center_y()
        .radius(999.0)
        .border(1.0, if on { t.accent.alpha(0.45) } else { t.border })
        .fill(if on { t.accent_dim } else { t.card_bg })
        .cursor(Cursor::Pointer)
        .hover_group()
        .focusable(true)
        .focus_ring(2.0, 2.0, t.accent)
        .on_press(|s: &mut State| s.turns_tab.changed_only = !s.turns_tab.changed_only)
        .child(
            text("Only turns that changed files").font(sans(12.0, 400.0, 1.5)).color(if on { t.text } else { t.muted }).group_color(t.text),
        );
    let totals = tab.totals();
    let mut sums = row().key("turns-total").gap(8.0).center_y().child(figure(turns_text(totals.turns), t.muted));
    if let Some(a) = additions_text(totals.additions) {
        sums = sums.child(figure(a, t.running_text));
    }
    if let Some(d) = deletions_text(totals.deletions) {
        sums = sums.child(figure(d, t.error_text));
    }
    row()
        .key("turns-summary")
        .w_full()
        .flex_wrap()
        .center_y()
        .gap(8.0)
        .px(12.0)
        .py(9.0)
        .border_sides([0.0, 0.0, 1.0, 0.0], t.border)
        .child(filter)
        .child(text("Newest first").key("turns-order").font(sans(12.0, 400.0, 1.5)).color(t.muted))
        .child(el().grow(1.0))
        .child(sums)
}

/// `Spinner` large: lucide `LoaderCircle` 20. Rune can't turn it, so it fades, as Fleet's does under reduced motion.
fn spinner(t: &Theme) -> El {
    static FADE: [(f32, f32); 3] = [(0.0, 1.0), (0.5, 0.35), (1.0, 1.0)];
    icon(&LOADER_CIRCLE, 20.0).color(t.muted).cycle(Cycle { period: 1.2, phase: 0.0, stops: &FADE, still: 1.0, fps: 30.0 })
}

/// `EmptyState compact` with the history icon.
fn empty(t: &Theme, key: &str, title: &str, line: Option<&str>) -> El {
    let mut e = col()
        .key(key.to_string())
        .w_full()
        .items_center()
        .gap(3.5)
        .px(10.5)
        .py(14.0)
        .child(icon(&HISTORY, 20.0).color(t.muted).mb(7.0))
        .child(text(title.to_string()).font(sans(12.25, 500.0, 1.5)).color(t.text));
    if let Some(line) = line {
        e = e.child(text(line.to_string()).font(sans(10.5, 400.0, 1.5)).color(t.muted).wrap().w_full().align(Align::Center));
    }
    e
}

fn list(s: &State, t: &Theme) -> El {
    let tab = &s.turns_tab;
    let mut body = col().key("turns-list-body").w_full().padding(6.0).gap(1.0).items_stretch();
    let visible = tab.visible();
    if tab.turns().is_empty() && s.loading {
        body = body.child(
            row()
                .key("turns-loading")
                .role("status")
                .label("Loading this session's turns\u{2026}")
                .w_full()
                .justify_center()
                .padding(21.0)
                .child(spinner(t)),
        );
    } else if tab.turns().is_empty() {
        body = body.child(empty(t, "turns-empty", "No turns yet", Some("The first prompt of this session starts one.")));
    } else if visible.is_empty() {
        body = body.child(empty(t, "turns-filter-empty", "No turn in the loaded history changed a file", None));
    }
    let models = s.turn_models();
    for turn in visible {
        body = body.child(turn_box(s, t, turn, models));
    }
    if !tab.turns().is_empty()
        && let Some(older) = s.older()
    {
        body = body.child(older_line(t, older));
    }
    body.scroll(rune::ScrollSpec { stick_bottom: false }).key("turns-list").w_full().grow(1.0).min_h(0.0)
}

fn turn_box(s: &State, t: &Theme, turn: &SessionTurn, models: &[crate::composer::choice::ModelOption]) -> El {
    let tab = &s.turns_tab;
    let open = tab.is_open(turn);
    let mut boxed =
        col().key(format!("turn-{}", turn.id)).w_full().radius(RADIUS_CARD).items_stretch().child(head(t, tab, turn, models, open));
    if open {
        boxed = boxed.child(super::turn_detail::detail(s, t, turn, models));
    }
    boxed
}

/// The row's button: caret, number, the prompt and what ran, and the line counts.
fn head(t: &Theme, tab: &TurnsTab, turn: &SessionTurn, models: &[crate::composer::choice::ModelOption], open: bool) -> El {
    let narrow = tab.narrow();
    let id = turn.id.clone();
    let mut sub = row().w_full().gap(6.0).center_y().min_w(0.0);
    sub = sub.child(
        row()
            .gap(5.0)
            .center_y()
            .child(el().size(5.0, 5.0).radius(2.5).fill(t.accent.alpha(0.75)))
            .child(text(model_display_name(turn.model_id.as_deref(), models)).font(sans(11.5, 400.0, 1.5)).color(t.muted)),
    );
    if turn.model_changed && !narrow {
        sub = sub.child(
            row()
                .key(format!("turn-changed-{}", turn.id))
                .px(6.0)
                .py(1.0)
                .radius(999.0)
                .fill(t.accent_dim)
                .child(text("model changed").font(sans(10.5, 400.0, 1.5)).color(t.text.alpha(0.78))),
        );
    }
    let facts = facts(turn);
    if !narrow && !facts.is_empty() {
        sub = sub.child(
            text(facts).key(format!("turn-facts-{}", turn.id)).font(sans(11.5, 400.0, 1.5)).color(t.muted).truncate().shrink().min_w(0.0),
        );
    }
    let mut stats = row().key(format!("turn-stats-{}", turn.id)).gap(7.0).center_y();
    let (a, d) = (turn.additions, turn.deletions);
    if a > 0 {
        stats = stats.child(figure(format!("+{a}"), t.running_text));
    }
    if d > 0 {
        stats = stats.child(figure(format!("\u{2212}{d}"), t.error_text));
    }
    if a == 0 && d == 0 {
        stats = stats.child(figure("\u{2014}".into(), t.muted));
    }
    let mut copy = col().grow(1.0).shrink().min_w(0.0).gap(1.0).items_stretch().child(
        text(prompt_line(turn).to_string()).key(format!("turn-prompt-{}", turn.id)).font(sans(13.0, 400.0, 1.5)).color(t.text).truncate(),
    );
    copy = copy.child(sub);
    let mut head = row()
        .key(format!("turn-row-{}", turn.id))
        .role("button")
        .label(prompt_line(turn).to_string())
        .w_full()
        .gap(9.0)
        .px(10.0)
        .py(8.0)
        .center_y()
        .radius(RADIUS_CARD)
        .fill(if open { t.wash(6.0) } else { rune::Color::TRANSPARENT })
        .hover(t.wash(if open { 6.0 } else { 5.0 }))
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, -2.0, t.accent)
        .on_press(move |s: &mut State| s.turns_tab.toggle_turn(&id))
        .child(icon(&CHEVRON_RIGHT, 12.0).color(t.muted).rotate(if open { FRAC_PI_2 } else { 0.0 }))
        .child(text(turn.number.to_string()).font(mono(11.0, 400.0, 1.5).features("'tnum' 1")).color(t.muted).min_w(22.0))
        .child(copy)
        .child(stats);
    if !turn.prompt_full.is_empty() {
        head = head.help(turn.prompt_full.clone());
    }
    head
}

/// "Older turns are not loaded yet." and the button that asks (`turns-canvas__older`).
fn older_line(t: &Theme, older: Older) -> El {
    let (label, busy) = match older {
        Older::Loading => ("Loading\u{2026}", true),
        Older::Failed => ("Couldn't load \u{b7} Try again", false),
        Older::Ask => ("Load older", false),
    };
    let mut button = row()
        .key("turns-load-older")
        .role("button")
        .label(label)
        .px(9.0)
        .py(3.0)
        .radius(8.0)
        .border(1.0, t.border)
        .fill(t.card_bg)
        .focusable(true)
        .focus_ring(2.0, 2.0, t.accent)
        .child(text(label).font(sans(12.0, 400.0, 1.5)).color(t.text.alpha(0.82)).group_color(t.text));
    button = if busy {
        button.opacity(0.6)
    } else {
        button.hover(t.wash(6.0)).hover_group().cursor(Cursor::Pointer).on_press(|s: &mut State| s.load_older())
    };
    row()
        .key("turns-older")
        .w_full()
        .flex_wrap()
        .center_y()
        .gap(8.0)
        .padding(10.0)
        .child(text("Older turns are not loaded yet.").font(sans(12.0, 400.0, 1.5)).color(t.muted))
        .child(button)
}
