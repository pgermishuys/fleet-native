//! The context ring's popover: the size against the window, the chart of each turn, the last call's split, the
//! harness's usage limits and Compact now. Ports the template and styles of `ContextRing.vue`'s `PopoverContent`
//! (docs/parity/05-composer.md, piece H). What it says is worked out in `composer/details.rs`; this only draws it.
use crate::State;
use crate::composer::details::{Bar, Chart, Details, Facts, LimitTone, Meter, details};
use crate::composer::ring::Tone;
use crate::fleet::usage::current_windows;
use crate::theme::{RADIUS_BTN, RADIUS_PANEL, Theme, sans};
use crate::ui::overlay::{anchored, scrim};
use crate::view::ring::srgb_mix;
use rune::{Align, Color, Cursor, El, LineCap, Shape, Side, canvas, col, el, icon, row, text};

const WIDTH: f32 = 340.0;
/// The width inside the 1px border and the 14px padding.
const INNER: f32 = WIDTH - 2.0 - 28.0;
const CHART_H: f32 = 54.0;

/// What the popover shows now, for the open session's context.
pub fn facts(s: &State) -> Facts<'_> {
    let row = &s.session().row;
    let id = s.open_id().unwrap_or_default();
    let now = crate::view::message::now_ms(s);
    let usage = row.harness_type.as_deref().and_then(|h| s.pickers.usage.harnesses.get(h));
    Facts {
        cost: row.cost,
        can_compact: row.can_compact,
        compact_disabled_reason: row.compact_disabled_reason.as_deref(),
        requesting: s.compact.requesting.contains(&id),
        request_error: s.compact.errors.get(&id).map(String::as_str),
        windows: current_windows(usage, now),
        now,
        offset_secs: crate::conv::time::local_offset_secs(),
        hovered: s.ring.hovered,
    }
}

/// The scrim and the card above the ring (`side="top" align="end" :side-offset="8"`).
pub fn overlay(s: &State, t: &Theme) -> Vec<El> {
    let Some(context) = s.context() else { return Vec::new() };
    let d = details(context, &facts(s));
    let card = card(s, t, &d)
        .key("context-popover")
        .role("dialog")
        .label("Context window")
        .w(WIDTH)
        .pt(14.0)
        .px(14.0)
        .pb(12.0)
        .radius(RADIUS_PANEL)
        .fill(t.card_bg)
        .border(1.0, t.border)
        .border_layout()
        .items_stretch();
    let card = t.menu_shadows().into_iter().fold(card, |e, shadow| e.box_shadow(shadow)).focus_return("context-ring".to_string()).modal(
        true,
        |s: &mut State| {
            s.ui.open = None;
            s.ring.hovered = None;
        },
    );
    vec![
        scrim("context-scrim", |s: &mut State| {
            s.ui.open = None;
            s.ring.hovered = None;
        }),
        anchored(card, "context-ring", Side::Top, Align::End, 8.0),
    ]
}

fn tone_color(t: &Theme, tone: Tone) -> Color {
    match tone {
        Tone::Ok => t.running,
        Tone::Warn => t.idle,
        Tone::Danger => t.error,
    }
}

fn card(s: &State, t: &Theme, d: &Details) -> El {
    let tone = tone_color(t, d.tone);
    let mut c = col().w_full().items_stretch().child(head(s, t, d, tone)).child(
        text(d.size_line.clone()).key("context-size").font(sans(12.5, 400.0, 1.5).features("'tnum' 1")).color(t.muted).mt(3.0).wrap(),
    );
    if let Some(m) = &d.meter {
        c = c.child(meter(t, m, d.tone)).child(legend(t, m));
    }
    if let Some(chart) = &d.chart {
        c = c.child(label_row(t, "After each turn", &chart.caption, false)).child(chart_view(t, chart, d.tone));
    }
    if !d.lines.is_empty() {
        c = c.child(rows(t, d));
    }
    if !d.limits.is_empty() {
        c = c.child(limits(t, d));
    }
    if let Some(error) = &d.error {
        c = c.child(text(error.clone()).key("context-error").font(sans(12.0, 400.0, 1.5)).color(t.error_text).mt(12.0).wrap());
    }
    c.child(foot(t, d))
}

/// `.context-card__head`: the eyebrow over the number, and the state label with its dot or spinner.
fn head(_s: &State, t: &Theme, d: &Details, tone: Color) -> El {
    let mut big = row().items_end().child(
        text(d.headline.clone()).key("context-headline").font(sans(26.0, 600.0, 1.1).tracking(-0.52).features("'tnum' 1")).color(t.text),
    );
    if d.percent_sign {
        big = big.child(text("%").font(sans(13.0, 500.0, 1.1)).color(t.muted).ml(2.0).mb(3.0));
    }
    let mark = if d.compacting {
        icon(&crate::icons::LOADER_CIRCLE, 12.0).color(t.muted).cycle(spinner())
    } else {
        el().key("context-dot").size(6.0, 6.0).radius(3.0).fill(tone)
    };
    row()
        .w_full()
        .justify_between()
        .items_start()
        .gap(12.0)
        .child(col().child(text("Context window").font(sans(11.0, 400.0, 1.5)).color(t.muted)).child(big))
        .child(
            row()
                .key("context-state")
                .gap(5.0)
                .center_y()
                .mt(4.0)
                .child(mark)
                .child(text(d.state_label).font(sans(11.0, 500.0, 1.5)).color(t.muted)),
        )
}

/// The small spinner turns; Rune has no looping rotation, so it fades, as Fleet's does under reduced motion.
pub(crate) fn spinner() -> rune::Cycle {
    static FADE: [(f32, f32); 3] = [(0.0, 1.0), (0.5, 0.35), (1.0, 1.0)];
    rune::Cycle { period: 1.2, phase: 0.0, stops: &FADE, still: 1.0, fps: 30.0 }
}

/// `.context-card__meter`: 6 px high; the fill is the accent, the tone itself once it is getting full.
fn meter(t: &Theme, m: &Meter, tone: Tone) -> El {
    let fill = if tone == Tone::Ok { t.accent } else { tone_color(t, tone) };
    let mut track = el().key("context-meter").w_full().h(6.0).mt(12.0).mb(4.0).radius(3.0).fill(srgb_mix(t.text, t.card_bg, 0.09));
    track = track.child(el().key("context-meter-fill").absolute(0.0, 0.0).w(INNER * m.fill as f32 / 100.0).h(6.0).radius(3.0).fill(fill));
    if let Some(tick) = m.tick {
        track = track.child(
            el().key("context-meter-tick")
                .absolute(INNER * tick as f32 / 100.0 - 1.0, -3.0)
                .size(2.0, 12.0)
                .radius(2.0)
                .fill(srgb_mix(t.muted, t.card_bg, 0.7)),
        );
    }
    track
}

fn legend(t: &Theme, m: &Meter) -> El {
    let font = sans(11.0, 400.0, 1.5).features("'tnum' 1");
    row()
        .w_full()
        .justify_between()
        .gap(8.0)
        .child(text("0").font(font).color(t.muted))
        .child(text(m.legend.clone()).font(font).color(t.muted))
}

/// `.context-card__label`: 11px muted, the two halves at the ends.
fn label_row(t: &Theme, left: &str, right: &str, ink_right: bool) -> El {
    let font = sans(11.0, 400.0, 1.5).features("'tnum' 1");
    let r = text(right.to_string()).font(font).color(if ink_right { t.text } else { t.muted });
    row().w_full().justify_between().gap(8.0).mt(14.0).child(text(left.to_string()).font(font).color(t.muted)).child(r)
}

/// `.context-card__chart`: 54 px, one bar per turn (at most 18 wide), a dashed line where the harness compacts.
fn chart_view(t: &Theme, chart: &Chart, tone: Tone) -> El {
    let n = chart.bars.len() as f32;
    let width = ((INNER - 2.0 * (n - 1.0)) / n).clamp(2.0, 18.0);
    let mut c = row().key("context-chart").w_full().h(CHART_H).mt(10.0).items_end().gap(2.0).on_hover_change(|s: &mut State, inside, _| {
        if !inside {
            s.ring.hovered = None;
        }
    });
    if let Some(line) = chart.limit_line {
        c = c.child(dashed_line(t, CHART_H * (1.0 - line as f32 / 100.0)));
    }
    for (i, bar) in chart.bars.iter().enumerate() {
        c = c.child(bar_view(t, bar, i, width, tone));
    }
    c
}

/// `border-top: 1px dashed` at 25% of the text colour, `bottom` points above the chart's floor.
fn dashed_line(t: &Theme, y: f32) -> El {
    let ink = srgb_mix(t.text, t.card_bg, 0.25);
    let mut shape = Shape::new();
    let mut x = 0.0;
    while x < INNER {
        shape = shape.stroke_with(vec![[x, 0.5], [(x + 3.0).min(INNER), 0.5]], false, 1.0, LineCap::Butt, ink);
        x += 6.0;
    }
    canvas(shape).size(INNER, 1.0).absolute(0.0, y)
}

fn bar_view(t: &Theme, bar: &Bar, index: usize, width: f32, tone: Tone) -> El {
    let rest = srgb_mix(t.accent, t.card_bg, 0.55);
    let last = if tone == Tone::Ok { t.accent } else { tone_color(t, tone) };
    let mut b = el()
        .key(format!("context-bar{index}"))
        .role("img")
        .label(bar.describe.clone())
        .w(width)
        .h((CHART_H * bar.height as f32 / 100.0).max(2.0))
        .fill(if bar.last { last } else { rest })
        .corners([2.0, 2.0, 0.0, 0.0])
        .hover(if bar.last { last } else { t.text })
        .on_hover(move |s: &mut State| s.ring.hovered = Some(index));
    if bar.after_compaction {
        b = b.child(el().absolute(width / 2.0 - 2.0, -7.0).size(4.0, 4.0).radius(2.0).fill(t.muted));
    }
    b
}

/// `.context-card__rows`: a label and its value, the value at the right.
fn rows(t: &Theme, d: &Details) -> El {
    let font = sans(12.5, 400.0, 1.5).features("'tnum' 1");
    let mut c = col().key("context-rows").w_full().gap(6.0).mt(14.0);
    for line in &d.lines {
        let mut value = row().gap(4.0).child(text(line.value.clone()).font(font).color(t.text));
        if let Some(m) = &line.muted {
            value = value.child(text(m.clone()).font(font).color(t.muted));
        }
        c = c.child(row().w_full().justify_between().gap(14.0).child(text(line.label).font(font).color(t.muted)).child(value));
    }
    c
}

/// `.context-card__limits`: the harness's windows, each with a 4 px meter.
fn limits(t: &Theme, d: &Details) -> El {
    let mut c = col()
        .key("context-limits")
        .label("Usage limits")
        .w_full()
        .gap(8.0)
        .mt(14.0)
        .pt(12.0)
        .border_sides([1.0, 0.0, 0.0, 0.0], t.border)
        .border_layout();
    for (i, l) in d.limits.iter().enumerate() {
        let fill = match l.tone {
            LimitTone::Ok => t.accent,
            LimitTone::Warn => t.idle,
            LimitTone::Danger => t.error,
        };
        let font = sans(11.0, 400.0, 1.5).features("'tnum' 1");
        c = c.child(
            col()
                .key(format!("context-limit{i}"))
                .w_full()
                .child(
                    row()
                        .w_full()
                        .justify_between()
                        .gap(8.0)
                        .child(text(l.label.clone()).font(font).color(t.muted))
                        .child(text(l.value.clone()).font(font).color(t.text)),
                )
                .child(
                    el().w_full()
                        .h(4.0)
                        .mt(5.0)
                        .radius(2.0)
                        .fill(srgb_mix(t.text, t.card_bg, 0.09))
                        .child(el().absolute(0.0, 0.0).w(INNER * l.fill.clamp(0, 100) as f32 / 100.0).h(4.0).radius(2.0).fill(fill)),
                ),
        );
    }
    c
}

/// `.context-card__foot`: the hint and Compact now.
fn foot(t: &Theme, d: &Details) -> El {
    let primary = d.tone != Tone::Ok;
    let (fill, ink, edge) =
        if primary { (t.accent, t.primary_fg, Color::TRANSPARENT) } else { (srgb_mix(t.text, t.card_bg, 0.05), t.text, t.border) };
    let hover = if primary { srgb_mix(t.accent, Color::BLACK, 0.88) } else { srgb_mix(t.text, t.card_bg, 0.10) };
    let mut button = el()
        .key("context-compact")
        .role("button")
        .label(d.compact_label)
        .help(d.compact_title.clone())
        .px(10.0)
        .py(6.0)
        .radius(RADIUS_BTN)
        .fill(fill)
        .border(1.0, edge)
        .border_layout()
        .child(text(d.compact_label).font(sans(12.0, 400.0, 1.5)).color(ink));
    button = if d.can_compact {
        button.hover(hover).cursor(Cursor::Pointer).focus_ring(2.0, 2.0, t.accent).on_press(|s: &mut State| s.compact_now())
    } else {
        button.opacity(0.5).disabled(true)
    };
    row()
        .w_full()
        .gap(8.0)
        .center_y()
        .mt(14.0)
        .pt(12.0)
        .border_sides([1.0, 0.0, 0.0, 0.0], t.border)
        .border_layout()
        .child(text(d.hint).font(sans(11.5, 400.0, 1.4)).color(t.muted).wrap().grow(1.0).shrink())
        .child(button)
}
