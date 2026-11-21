//! The Overview's daily chart (`OverviewTab.vue`, drawn there by Chart.js 4.5). Rune has no chart, so the numbers come from
//! `fleet::analytics::chart` (Chart.js's own geometry) and are drawn here: one canvas for the grid, the lines and the
//! hovered points, Rune text for every label, and a second canvas under the tooltip's words.
use crate::State;
use crate::fleet::analytics::Daily;
use crate::fleet::analytics::chart::*;
use crate::theme::{Theme, chart_default_grid, chart_tip_bg, chart_tip_ink, sans};
use rune::{Color, Cursor, El, LineCap, Path, Shape, col, el, text};

const DASH: (f32, f32) = (4.0, 3.0);
/// Segments per curve piece: a day apart is at most a few hundred points, so 24 is smooth.
const STEPS: usize = 24;

/// Chart.js draws on a canvas, which takes no CSS font: its own default family is `'Helvetica Neue', 'Helvetica', 'Arial',
/// sans-serif`, so Fleet's chart text is not Inter. A family that isn't installed falls back to the sans slot.
fn style(size: f32, weight: f32) -> rune::TextStyle {
    let mut s = sans(size, weight, 1.2);
    s.family = rune::Family::Named("Helvetica Neue, Helvetica, Arial");
    s
}

fn measure(text: &str, size: f32, bold: bool) -> f32 {
    rune::text::measure(text, &[], style(size, if bold { 700.0 } else { 400.0 }), None).0
}

/// The year the day labels are written against.
fn this_year(s: &State) -> i32 {
    let ms = s.now_ms.unwrap_or_else(crate::fleet::link::now_ms);
    crate::conv::time::civil(ms, crate::conv::time::local_offset_at(ms)).0 as i32
}

/// The day labels, in the order the rows come.
pub fn labels(s: &State, days: &[Daily]) -> Vec<String> {
    let year = this_year(s);
    days.iter().map(|d| format_day(&d.date, year)).collect()
}

fn lay(s: &State, days: &[Daily], labels: &[String], width: f32) -> Layout {
    let tokens: Vec<f64> = days.iter().map(|d| d.tokens).collect();
    let cost: Vec<f64> = days.iter().map(|d| d.cost).collect();
    use crate::composer::format::{format_money, format_tokens};
    let left: &dyn Fn(f64) -> String = &|v| format_tokens(Some(v));
    let right: &dyn Fn(f64) -> String = &|v| format_money(Some(if v.is_finite() && v > 0.0 { v } else { 0.0 }));
    layout(
        &Input {
            labels,
            tokens: &tokens,
            cost: &cost,
            shown: s.analytics.chart.shown(),
            width,
            height: HEIGHT,
            left_label: left,
            right_label: right,
        },
        &measure,
    )
}

fn ink(t: &Theme, s: Series) -> Color {
    match s {
        Series::Tokens => t.accent,
        Series::Cost => t.running,
    }
}

/// A line on whole-pixel centres, as Chart.js aligns a 1 px line.
fn snap(v: f32) -> f32 {
    (v - 0.5).round() + 0.5
}

/// Chart.js's hover colour (`getHoverColor`): the colour saturated by half again and darkened a tenth, in HSL, which is why
/// a hovered point is a little stronger than its line.
fn hover_ink(c: Color) -> Color {
    let [r, g, b] = [c.r, c.g, c.b].map(rune::color::srgb_encode);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let (mut h, mut s, mut l) = (0.0, 0.0, (max + min) / 2.0);
    if max != min {
        let d = max - min;
        s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
        h = if max == r {
            (g - b) / d + if g < b { 6.0 } else { 0.0 }
        } else if max == g {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        };
        h /= 6.0;
    }
    s = (s * 1.5).min(1.0);
    l -= l * 0.1;
    let channel = |p: f32, q: f32, mut t: f32| {
        t = t.rem_euclid(1.0);
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    let [r, g, b] = if s == 0.0 {
        [l, l, l]
    } else {
        let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
        let p = 2.0 * l - q;
        [channel(p, q, h + 1.0 / 3.0), channel(p, q, h), channel(p, q, h - 1.0 / 3.0)]
    };
    let [r, g, b] = [r, g, b].map(rune::color::srgb_decode);
    Color { r, g, b, a: c.a }
}

fn circle(c: [f32; 2], r: f32) -> Vec<[f32; 2]> {
    (0..24).map(|i| i as f32 / 24.0 * std::f32::consts::TAU).map(|a| [c[0] + r * a.cos(), c[1] + r * a.sin()]).collect()
}

/// The grid, the axis line, the area and the two lines, the hovered points and the legend's lines.
fn picture(t: &Theme, l: &Layout, shown: [bool; 2], hover: Option<usize>) -> Shape {
    let a = l.area;
    let mut sh = Shape::new();
    for tick in &l.left {
        let y = snap(tick.y);
        sh = sh.line([a.left, y], [a.right, y], 1.0, LineCap::Butt, t.border).line(
            [a.left - TICK_LENGTH, y],
            [a.left, y],
            1.0,
            LineCap::Butt,
            t.border,
        );
    }
    for tick in &l.right {
        let y = snap(tick.y);
        sh = sh.line([a.right, y], [a.right + TICK_LENGTH, y], 1.0, LineCap::Butt, chart_default_grid());
    }
    let base = snap(a.bottom);
    sh = sh.line([a.left, base], [a.right, base], 1.0, LineCap::Butt, t.border);
    if shown[0] && !l.tokens.is_empty() {
        let line = curve(&l.tokens, STEPS);
        if l.tokens.len() > 1 {
            let mut edge = line.clone();
            edge.push([l.tokens[l.tokens.len() - 1].at[0], a.bottom]);
            edge.push([l.tokens[0].at[0], a.bottom]);
            sh = sh.fill(edge, t.accent.alpha(0.14));
        }
        sh = sh.stroke_with(line, false, 2.0, LineCap::Butt, t.accent);
    }
    if shown[1] && !l.cost.is_empty() {
        for d in dashes(&curve(&l.cost, STEPS), DASH.0, DASH.1) {
            sh = sh.stroke_with(d, false, 2.0, LineCap::Butt, t.running);
        }
    }
    if let Some(i) = hover {
        for s in Series::ALL {
            if let (true, Some(p)) = (shown[s.index()], l.points(s).get(i)) {
                sh = sh.fill(circle(p.at, HOVER_RADIUS), hover_ink(ink(t, s)));
            }
        }
    }
    for item in &l.legend {
        let half = LEGEND_LINE / 2.0;
        let (x0, x1, y) = (item.line_x - half, item.line_x + half, item.line_y);
        // Both entries are solid: Chart.js doesn't dash a point-style legend line, whatever the dataset's own dash.
        sh = sh.line([x0, y], [x1, y], 2.0, LineCap::Butt, ink(t, item.series));
        if !shown[item.series.index()] {
            // Chart.js's strikethrough through the word.
            sh = sh.line([item.text_x, y], [item.text_x + item.text_w, y], 2.0, LineCap::Butt, t.muted);
        }
    }
    sh
}

/// A label placed by its left and top.
fn word(key: String, words: String, left: f32, top: f32, font: rune::TextStyle, color: Color) -> El {
    el().absolute(left, top).inert().child(text(words).key(key).font(font).color(color))
}

fn tooltip_path(b: &TipBox) -> Path {
    let (x, y, w, h, r) = (b.x, b.y, b.w, b.h, TIP_CORNER);
    let caret = caret_points(b);
    let add = |mut p: Path| {
        for c in caret {
            p = p.line_to(c);
        }
        p
    };
    let mut p = Path::new().move_to([x + r, y]);
    if b.y_align == YAlign::Top {
        p = add(p);
    }
    p = p.line_to([x + w - r, y]).quad_to([x + w, y], [x + w, y + r]);
    if b.y_align == YAlign::Center && b.x_align != XAlign::Left {
        p = add(p);
    }
    p = p.line_to([x + w, y + h - r]).quad_to([x + w, y + h], [x + w - r, y + h]);
    if b.y_align == YAlign::Bottom {
        p = add(p);
    }
    p = p.line_to([x + r, y + h]).quad_to([x, y + h], [x, y + h - r]);
    if b.y_align == YAlign::Center && b.x_align == XAlign::Left {
        p = add(p);
    }
    p.line_to([x, y + r]).quad_to([x, y], [x + r, y]).close()
}

/// A tooltip colour box: the line's colour with a lighter ring inside its edge, as Fleet draws it.
fn colour_box(c: Color, x: f32, y: f32) -> Shape {
    let size = TIP_BOX;
    let ring = c.mix(Color::WHITE, 0.23);
    Shape::new().fill(vec![[x, y], [x + size, y], [x + size, y + size], [x, y + size]], c).stroke_with(
        vec![[x + 1.5, y + 1.5], [x + size - 1.5, y + 1.5], [x + size - 1.5, y + size - 1.5], [x + 1.5, y + size - 1.5]],
        true,
        1.0,
        LineCap::Butt,
        ring,
    )
}

fn tooltip(t: &Theme, l: &Layout, tip: &Tip, b: &TipBox) -> Vec<El> {
    let mut out = vec![
        rune::canvas(Shape::new().fill_path(&tooltip_path(b), chart_tip_bg()))
            .key("analytics-chart-tooltip-box")
            .absolute(0.0, 0.0)
            .size(l.width, l.height)
            .inert(),
        word(
            "analytics-chart-tooltip-title".into(),
            tip.title.clone(),
            b.x + TIP_PAD,
            b.y + TIP_PAD,
            style(TIP_SIZE, 700.0),
            chart_tip_ink(),
        ),
    ];
    let text_x = b.x + TIP_PAD + TIP_BOX + 2.0;
    let mut y = b.y + TIP_PAD + TIP_LINE + 6.0;
    let step = TIP_LINE + 2.0;
    for (i, row) in tip.rows.iter().enumerate() {
        let box_y = y + (TIP_LINE - TIP_BOX) / 2.0;
        let swatch = colour_box(ink(t, row.series), b.x + TIP_PAD, box_y);
        out.push(
            rune::canvas(swatch)
                .key(format!("analytics-chart-tooltip-swatch-{}", row.series.key()))
                .absolute(0.0, 0.0)
                .size(l.width, l.height)
                .inert(),
        );
        out.push(word(format!("analytics-chart-tooltip-row-{i}"), row.text.clone(), text_x, y, style(TIP_SIZE, 400.0), chart_tip_ink()));
        y += step;
        if let Some(after) = &row.after {
            out.push(word(format!("analytics-chart-tooltip-after-{i}"), after.clone(), text_x, y, style(TIP_SIZE, 400.0), chart_tip_ink()));
            y += step;
        }
    }
    out
}

/// What a screen reader gets for the chart: a table of the days (`analytics-daily-trend-table`).
fn table(days: &[Daily], labels: &[String]) -> El {
    use crate::composer::format::{format_money, format_tokens};
    let rows = days.iter().zip(labels).map(|(d, label)| {
        let words = format!("{label}: {} tokens, {}", format_tokens(Some(d.tokens)), format_money(Some(d.cost.max(0.0))));
        el().role("row").label(words)
    });
    col().key("analytics-daily-trend-table").role("table").label("Daily tokens and cost").absolute(0.0, 0.0).size(0.0, 0.0).children(rows)
}

/// `<Line>` in its 300 px slot: laid out to its own width, drawn, and hovered.
pub fn chart(s: &State, t: &Theme, days: &[Daily]) -> El {
    let view = &s.analytics.chart;
    let labels = labels(s, days);
    let mut root = col()
        .key("analytics-chart-slot")
        .role("img")
        .label(chart_label(&labels))
        .w_full()
        .h(HEIGHT)
        .on_layout(|s: &mut State, [_, _, w, _]| {
            if (s.analytics.chart.width - w).abs() > 0.01 {
                s.analytics.chart.width = w;
            }
        })
        .on_pointer_move_local(|s: &mut State, p: rune::PointerEvent, _| {
            let days = s.analytics.daily.data.clone();
            let labels = labels_for(s, &days);
            let l = lay(s, &days, &labels, s.analytics.chart.width);
            // Over the area the nearest day is picked; over an axis the last one stays (Chart.js's `inChartArea`).
            if l.area.contains(p.x, p.y) {
                s.analytics.chart.hover = l.nearest(p.x);
            }
        })
        .on_hover_change(|s: &mut State, inside: bool, _| {
            if !inside {
                s.analytics.chart.hover = None;
            }
        });
    if view.width < 1.0 || days.is_empty() {
        return root;
    }
    let l = lay(s, days, &labels, view.width);
    let shown = view.shown();
    // A hover can name a day the rows no longer have (the filters read again).
    let hover = view.hover.filter(|i| *i < days.len());
    root = root
        .child(rune::canvas(picture(t, &l, shown, hover)).key("analytics-chart-canvas").absolute(0.0, 0.0).size(l.width, l.height).inert());
    let tick = style(TICK_SIZE, 400.0);
    for (i, tk) in l.left.iter().enumerate() {
        root = root.child(word(
            format!("analytics-chart-left-{i}"),
            tk.text.clone(),
            l.area.left - TICK_LENGTH - TICK_PAD - tk.w,
            tk.y - TICK_LINE / 2.0,
            tick,
            t.muted,
        ));
    }
    for (i, tk) in l.right.iter().enumerate() {
        root = root.child(word(
            format!("analytics-chart-right-{i}"),
            tk.text.clone(),
            l.area.right + TICK_LENGTH + TICK_PAD,
            tk.y - TICK_LINE / 2.0,
            tick,
            t.muted,
        ));
    }
    for tk in &l.x {
        root = root.child(word(
            format!("analytics-chart-x-{}", tk.index),
            tk.text.clone(),
            tk.x - tk.w / 2.0,
            l.area.bottom + TICK_LENGTH + TICK_PAD,
            tick,
            t.muted,
        ));
    }
    for item in &l.legend {
        let top = item.hit[1] + LEGEND_SIZE / 2.0 - LEGEND_SIZE * 1.2 / 2.0;
        root = root.child(word(
            format!("analytics-legend-word-{}", item.series.key()),
            item.series.label().into(),
            item.text_x,
            top,
            style(LEGEND_SIZE, 400.0),
            t.muted,
        ));
        let series = item.series;
        let [x, y, w, h] = item.hit;
        root = root.child(
            el().key(format!("analytics-legend-{}", series.key()))
                .role("button")
                .label(format!("{}{}", series.label(), if shown[series.index()] { "" } else { ", hidden" }))
                .absolute(x, y)
                .size(w, h)
                .cursor(Cursor::Pointer)
                .on_press(move |s: &mut State| s.analytics.chart.toggle(series)),
        );
    }
    if let Some(i) = hover {
        let tip = tip(&days[i], &labels[i], shown);
        let points: Vec<[f32; 2]> = Series::ALL.iter().filter(|x| shown[x.index()]).map(|x| l.points(*x)[i].at).collect();
        if let Some(b) = tip.place(&l, &points, &measure) {
            for e in tooltip(t, &l, &tip, &b) {
                root = root.child(e);
            }
        }
    }
    root.child(table(days, &labels))
}

fn labels_for(s: &State, days: &[Daily]) -> Vec<String> {
    labels(s, days)
}
