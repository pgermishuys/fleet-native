//! The Overview's daily chart, as numbers (`OverviewTab.vue` drawing with Chart.js 4.5): the axes' nice ticks, the chart
//! area from the label widths and Chart.js's paddings, the labels the x axis keeps, the points, the spline control points,
//! the nearest day for a pointer and the tooltip's box. Plain Rust, no Rune: the widths of text come in through
//! [`Measure`], so a test can give its own and the view gives Rune's.
//!
//! Every constant below is one of Chart.js's defaults (the file it comes from is `chart.js/dist/chart.js`), or Fleet's own
//! option in `OverviewTab.vue`. They are named where the code uses them.
use super::Daily;

/// The chart's height: `.overview-tab__chart { height: 300px }`.
pub const HEIGHT: f32 = 300.0;
/// Tick label size (`ticks.font.size: 11`) and its line height (Chart.js's 1.2).
pub const TICK_SIZE: f32 = 11.0;
pub const TICK_LINE: f32 = TICK_SIZE * 1.2;
/// Legend and tooltip text size (`font.size: 12` on the legend, Chart.js's default 12 on the tooltip).
pub const LEGEND_SIZE: f32 = 12.0;
pub const TIP_SIZE: f32 = 12.0;
pub const TIP_LINE: f32 = TIP_SIZE * 1.2;
/// `grid.tickLength`: the tick marks, and the room the axes keep for them.
pub const TICK_LENGTH: f32 = 8.0;
/// `ticks.padding`: between a tick mark and its label.
pub const TICK_PAD: f32 = 3.0;
/// `ticks.autoSkipPadding` on the x axis (Fleet's option).
pub const SKIP_PAD: f32 = 16.0;
/// The legend's `labels.padding`.
pub const LEGEND_PAD: f32 = 10.0;
/// `tension: 0.35` on both lines.
pub const TENSION: f32 = 0.35;
/// `pointHoverRadius`.
pub const HOVER_RADIUS: f32 = 4.0;
/// The most ticks a linear axis asks for (`getTickLimit`: `maxTicksLimit || 11`).
const MAX_TICKS: usize = 11;

// Tooltip defaults.
pub const TIP_PAD: f32 = 6.0;
const TIP_TITLE_GAP: f32 = 6.0;
const TIP_BODY_GAP: f32 = 2.0;
pub const TIP_BOX: f32 = 12.0;
const TIP_BOX_PAD: f32 = 0.0;
pub const TIP_CARET: f32 = 5.0;
const TIP_CARET_PAD: f32 = 2.0;
pub const TIP_CORNER: f32 = 6.0;

/// Width of `text` at `size` points; `bold` is the tooltip's title.
pub type Measure<'a> = &'a dyn Fn(&str, f32, bool) -> f32;

/// The two datasets: Tokens on the left axis, Cost on the right.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Series {
    Tokens,
    Cost,
}

impl Series {
    pub const ALL: [Series; 2] = [Series::Tokens, Series::Cost];

    pub fn label(self) -> &'static str {
        match self {
            Series::Tokens => "Tokens",
            Series::Cost => "Cost",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Series::Tokens => "tokens",
            Series::Cost => "cost",
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }
}

/// `formatDate` of a bare day ("2026-11-20" is that day's local midnight): "Nov 20", with the year when it isn't `this_year`.
pub fn format_day(date: &str, this_year: i32) -> String {
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let mut parts = date.trim().split('-');
    let (Some(y), Some(m), Some(d)) = (parts.next(), parts.next(), parts.next()) else { return String::new() };
    let (Ok(y), Ok(m), Ok(d)) = (y.parse::<i32>(), m.parse::<usize>(), d.get(..2).unwrap_or(d).parse::<u32>()) else {
        return String::new();
    };
    let Some(month) = m.checked_sub(1).and_then(|i| MONTHS.get(i)) else { return String::new() };
    if y == this_year { format!("{month} {d}") } else { format!("{month} {d}, {y}") }
}

/// The screen reader's name for the chart (`dailyChartLabel`).
pub fn chart_label(labels: &[String]) -> String {
    match (labels.first(), labels.last()) {
        (Some(first), Some(last)) => {
            format!("Line chart of daily tokens and cost, {first} to {last}. The numbers follow in a table.")
        }
        _ => "Daily tokens and cost".into(),
    }
}

/// What the chart keeps between frames: Fleet's component state (`Chart.js`'s hidden datasets and active elements).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct View {
    /// The chart's width in points, from its last layout; 0 before it has one.
    pub width: f32,
    /// Lines the legend has hidden, tokens then cost.
    pub hidden: [bool; 2],
    /// The day the tooltip is for: set while the pointer is over the chart area, kept while it is over an axis, cleared
    /// when it leaves (Chart.js keeps the last active elements for a move outside the area).
    pub hover: Option<usize>,
}

impl View {
    pub fn shown(&self) -> [bool; 2] {
        [!self.hidden[0], !self.hidden[1]]
    }

    /// A press on a legend entry: Chart.js's default hides the line, and shows it again.
    pub fn toggle(&mut self, s: Series) {
        self.hidden[s.index()] = !self.hidden[s.index()];
    }
}

// ---- the axes -----------------------------------------------------------------------------------------------------

fn almost_equals(x: f64, y: f64, epsilon: f64) -> bool {
    (x - y).abs() < epsilon
}

/// JavaScript's `Math.round`: halves go up.
fn js_round(x: f64) -> f64 {
    (x + 0.5).floor()
}

/// Chart.js's `niceNum`.
fn nice_num(range: f64) -> f64 {
    let rounded = js_round(range);
    let range = if almost_equals(range, rounded, range / 1000.0) { rounded } else { range };
    let nice_range = 10f64.powf(range.log10().floor());
    let fraction = range / nice_range;
    let nice = if fraction <= 1.0 {
        1.0
    } else if fraction <= 2.0 {
        2.0
    } else if fraction <= 5.0 {
        5.0
    } else {
        10.0
    };
    nice * nice_range
}

fn decimal_places(x: f64) -> i32 {
    let (mut e, mut p) = (1.0f64, 0);
    while js_round(x * e) / e != x && p < 300 {
        e *= 10.0;
        p += 1;
    }
    p
}

/// A linear axis that begins at zero: the ticks Chart.js's `generateTicks` makes for data up to `data_max` (`None`: no
/// visible dataset on the axis, which Chart.js reads as 0 to 1), asking for at most `max_ticks`. The last tick is the
/// axis' top (`bounds: "ticks"`).
pub fn nice_ticks(data_max: Option<f64>, max_ticks: usize) -> Vec<f64> {
    let max = data_max.filter(|m| m.is_finite()).unwrap_or(1.0).max(0.0);
    // `beginAtZero` pulls a positive minimum down to zero; a range that is still a single value gets one added.
    let (data_min, data_max) = if max == 0.0 { (0.0, 1.0) } else { (0.0, max) };
    let max_spaces = (max_ticks.max(2) - 1) as f64;
    let mut spacing = nice_num((data_max - data_min) / max_spaces);
    if spacing < 1e-14 {
        return vec![data_min, data_max];
    }
    let spaces = (data_max / spacing).ceil() - (data_min / spacing).floor();
    if spaces > max_spaces {
        spacing = nice_num(spaces * spacing / max_spaces);
    }
    let mut nice_min = (data_min / spacing).floor() * spacing;
    let mut nice_max = (data_max / spacing).ceil() * spacing;
    let mut spaces = (nice_max - nice_min) / spacing;
    spaces = if almost_equals(spaces, spaces.round(), spacing / 1000.0) { spaces.round() } else { spaces.ceil() };
    let decimals = decimal_places(spacing).max(decimal_places(nice_min));
    let factor = 10f64.powi(decimals);
    nice_min = js_round(nice_min * factor) / factor;
    nice_max = js_round(nice_max * factor) / factor;
    let mut ticks = vec![nice_min];
    for j in 1..spaces as usize {
        ticks.push(js_round((nice_min + j as f64 * spacing) * factor) / factor);
    }
    ticks.push(nice_max);
    ticks
}

/// Chart.js's `autoSkip` with no major ticks: every `ceil(count / limit)`th tick, from the first. Returns the kept indices.
fn auto_skip(count: usize, limit: f64) -> Vec<usize> {
    let spacing = (count as f64 / limit).max(1.0).ceil();
    let mut kept = Vec::new();
    let (mut taken, mut next) = (0.0f64, 0.0f64);
    for i in 0..count {
        if i as f64 == next {
            kept.push(i);
            taken += 1.0;
            next = js_round(taken * spacing);
        }
    }
    kept
}

/// How many ticks fit along an axis (`determineMaxTicks`): `tick_size` is the room one label needs.
fn skip_limit(length: f32, max_length: f32, tick_size: f32) -> f64 {
    let tick_size = f64::from(tick_size.max(0.001));
    (f64::from(length) / tick_size + 1.0).min(f64::from(max_length) / tick_size).floor()
}

// ---- the layout ---------------------------------------------------------------------------------------------------

/// What the chart draws and how: the days, the two series and which of them are shown.
pub struct Input<'a> {
    pub labels: &'a [String],
    pub tokens: &'a [f64],
    pub cost: &'a [f64],
    pub shown: [bool; 2],
    pub width: f32,
    pub height: f32,
    /// The text on a y tick, per axis (`formatTokens`, `formatMoney`).
    pub left_label: &'a dyn Fn(f64) -> String,
    pub right_label: &'a dyn Fn(f64) -> String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Rect {
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.left && x <= self.right && y >= self.top && y <= self.bottom
    }
    pub fn width(&self) -> f32 {
        self.right - self.left
    }
    pub fn height(&self) -> f32 {
        self.bottom - self.top
    }
}

/// One legend entry: where its line, its text and its press target are.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LegendItem {
    pub series: Series,
    /// The press target: `left`, `top`, its width and its height.
    pub hit: [f32; 4],
    /// The line is centred on `line_x`, `line_y`, `LEGEND_LINE` long.
    pub line_x: f32,
    pub line_y: f32,
    pub text_x: f32,
    pub text_w: f32,
}

/// `boxHeight * SQRT2` for the legend's `pointStyle: "line"`: the line is that long and 2 thick.
pub const LEGEND_LINE: f32 = LEGEND_SIZE * std::f32::consts::SQRT_2;

#[derive(Clone, Debug, PartialEq)]
pub struct YTick {
    pub text: String,
    pub y: f32,
    pub w: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct XTick {
    pub index: usize,
    pub text: String,
    pub x: f32,
    pub w: f32,
}

/// A point and the control points of the curve into it and out of it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub at: [f32; 2],
    pub cp1: [f32; 2],
    pub cp2: [f32; 2],
}

#[derive(Clone, Debug)]
pub struct Layout {
    pub width: f32,
    pub height: f32,
    pub area: Rect,
    pub legend: Vec<LegendItem>,
    pub left: Vec<YTick>,
    pub right: Vec<YTick>,
    pub x: Vec<XTick>,
    pub tokens: Vec<Point>,
    pub cost: Vec<Point>,
}

fn max_of(values: &[f64]) -> Option<f64> {
    values.iter().copied().filter(|v| v.is_finite()).fold(None, |m, v| Some(m.map_or(v, |m: f64| m.max(v))))
}

/// A y axis: its kept ticks (value, text, width) and its top value.
struct YAxis {
    ticks: Vec<(f64, String, f32)>,
    top: f64,
}

fn y_axis(max: Option<f64>, height: f32, label: &dyn Fn(f64) -> String, measure: Measure) -> YAxis {
    // `computeTickLimit`: one tick per label line, at most 11.
    let limit = ((f64::from(height) / f64::from(TICK_LINE)).ceil() as usize).clamp(2, MAX_TICKS);
    let values = nice_ticks(max, limit);
    let top = values.last().copied().unwrap_or(1.0);
    // Then `autoSkip` along the axis, a label and `autoSkipPadding` (Chart.js's 3) per tick.
    let kept = auto_skip(values.len(), skip_limit(height, height, TICK_LINE + 3.0));
    let ticks = kept
        .into_iter()
        .map(|i| {
            let text = label(values[i]);
            let w = measure(&text, TICK_SIZE, false);
            (values[i], text, w)
        })
        .collect();
    YAxis { ticks, top }
}

fn axis_width(a: &YAxis) -> f32 {
    TICK_LENGTH + a.ticks.iter().map(|t| t.2).fold(0.0, f32::max) + TICK_PAD * 2.0
}

/// The curve through `at`: Chart.js's `splineCurve` for each point, with the control points held inside `area`.
fn spline(at: &[[f32; 2]], area: Rect) -> Vec<Point> {
    let n = at.len();
    let mut out: Vec<Point> = Vec::with_capacity(n);
    for i in 0..n {
        let cur = at[i];
        let prev = if i == 0 { cur } else { at[i - 1] };
        let next = if i + 1 == n { cur } else { at[i + 1] };
        let d01 = (cur[0] - prev[0]).hypot(cur[1] - prev[1]);
        let d12 = (next[0] - cur[0]).hypot(next[1] - cur[1]);
        let sum = d01 + d12;
        let (s01, s12) = if sum == 0.0 { (0.0, 0.0) } else { (d01 / sum, d12 / sum) };
        let (fa, fb) = (TENSION * s01, TENSION * s12);
        let (dx, dy) = (next[0] - prev[0], next[1] - prev[1]);
        let cap = |p: [f32; 2]| [p[0].clamp(area.left, area.right), p[1].clamp(area.top, area.bottom)];
        out.push(Point { at: cur, cp1: cap([cur[0] - fa * dx, cur[1] - fa * dy]), cp2: cap([cur[0] + fb * dx, cur[1] + fb * dy]) });
    }
    out
}

/// The whole chart for `input`; `measure` sizes every label.
pub fn layout(input: &Input, measure: Measure) -> Layout {
    let (width, height) = (input.width, input.height);
    let n = input.labels.len();
    // The legend: one row of two entries, its height `itemHeight + padding`, plus `padding`.
    let item_w = |s: Series| LEGEND_SIZE + LEGEND_SIZE / 2.0 + measure(s.label(), LEGEND_SIZE, false);
    let legend_h = (LEGEND_SIZE + LEGEND_PAD) + LEGEND_PAD;
    let row_w: f32 = Series::ALL.iter().map(|s| item_w(*s) + LEGEND_PAD).sum();
    let mut cursor = width - row_w;
    let legend = Series::ALL
        .iter()
        .map(|s| {
            let w = item_w(*s);
            let item = LegendItem {
                series: *s,
                hit: [cursor, LEGEND_PAD, w, LEGEND_SIZE],
                line_x: cursor + LEGEND_SIZE / 2.0,
                line_y: LEGEND_PAD + LEGEND_SIZE / 2.0,
                text_x: cursor + LEGEND_SIZE + LEGEND_SIZE / 2.0,
                text_w: w - LEGEND_SIZE - LEGEND_SIZE / 2.0,
            };
            cursor += w + LEGEND_PAD;
            item
        })
        .collect();

    // Axes. A hidden dataset leaves its axis with no data, which is 0 to 1.
    let guess = height - legend_h;
    let shown_max = |s: Series, v: &[f64]| input.shown[s.index()].then(|| max_of(v)).flatten();
    let left = y_axis(shown_max(Series::Tokens, input.tokens), guess, input.left_label, measure);
    let right = y_axis(shown_max(Series::Cost, input.cost), guess, input.right_label, measure);

    // The x axis: its labels are kept evenly by what fits, first and last decide how far the area is pushed in.
    let widest = input.labels.iter().map(|l| measure(l, TICK_SIZE, false)).fold(0.0, f32::max);
    let bottom_h = TICK_LENGTH + TICK_LINE + TICK_PAD * 2.0;
    let side = |w: f32| -> f32 { TICK_PAD + w / 2.0 };
    let mut area = Rect { left: 0.0, top: 0.0, right: width, bottom: height };
    let mut kept: Vec<usize> = Vec::new();
    // The area depends on the kept labels (the ends) and they on the area's width: two passes settle it, as Chart.js's do.
    let mut inset = (axis_width(&left), axis_width(&right));
    for _ in 0..2 {
        area.left = inset.0;
        area.right = width - inset.1;
        kept = auto_skip(n, skip_limit(area.width(), width, widest + SKIP_PAD));
        let first = kept.first().map_or(0.0, |i| measure(&input.labels[*i], TICK_SIZE, false));
        let last = kept.last().map_or(0.0, |i| measure(&input.labels[*i], TICK_SIZE, false));
        inset = (axis_width(&left).max(side(first)), axis_width(&right).max(side(last)));
    }
    area.left = inset.0;
    area.right = width - inset.1;
    area.top = legend_h.max(TICK_LINE / 2.0 + TICK_PAD);
    area.bottom = height - bottom_h.max(TICK_LINE / 2.0 + TICK_PAD);

    let x_at = |i: usize| if n < 2 { area.left } else { area.left + area.width() * i as f32 / (n - 1) as f32 };
    let y_ticks = |a: &YAxis| -> Vec<YTick> {
        a.ticks
            .iter()
            .map(|(v, text, w)| YTick { text: text.clone(), y: area.bottom - area.height() * (*v / a.top) as f32, w: *w })
            .collect()
    };
    let y_of = |top: f64, v: f64| area.bottom - area.height() * (v / top) as f32;
    let points = |values: &[f64], a: &YAxis| -> Vec<Point> {
        let at: Vec<[f32; 2]> = values.iter().enumerate().map(|(i, v)| [x_at(i), y_of(a.top, *v)]).collect();
        spline(&at, area)
    };
    let x = kept
        .iter()
        .map(|i| XTick { index: *i, text: input.labels[*i].clone(), x: x_at(*i), w: measure(&input.labels[*i], TICK_SIZE, false) })
        .collect();
    Layout {
        width,
        height,
        area,
        legend,
        left: y_ticks(&left),
        right: y_ticks(&right),
        x,
        tokens: points(input.tokens, &left),
        cost: points(input.cost, &right),
    }
}

impl Layout {
    /// The day under a pointer at `x` (`mode: "index"`, `intersect: false`: the nearest by x, the first of equals).
    pub fn nearest(&self, x: f32) -> Option<usize> {
        let points = if self.tokens.is_empty() { &self.cost } else { &self.tokens };
        let mut best: Option<(usize, f32)> = None;
        for (i, p) in points.iter().enumerate() {
            let d = (p.at[0] - x).abs();
            if best.is_none_or(|(_, b)| d < b) {
                best = Some((i, d));
            }
        }
        best.map(|b| b.0)
    }

    pub fn points(&self, s: Series) -> &[Point] {
        match s {
            Series::Tokens => &self.tokens,
            Series::Cost => &self.cost,
        }
    }
}

// ---- the tooltip --------------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XAlign {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum YAlign {
    Top,
    Center,
    Bottom,
}

/// One body entry: a colour box and its line, and optionally a line without a box under it.
#[derive(Clone, Debug, PartialEq)]
pub struct TipRow {
    pub series: Series,
    pub text: String,
    pub after: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tip {
    pub title: String,
    pub rows: Vec<TipRow>,
}

/// What the tooltip says for day `i`: the title, then each shown series (Cost carries its "Estimated:" line).
pub fn tip(day: &Daily, label: &str, shown: [bool; 2]) -> Tip {
    use crate::composer::format::{format_count, format_money};
    let money = |v: f64| format_money(Some(if v.is_finite() && v > 0.0 { v } else { 0.0 }));
    let mut rows = Vec::new();
    if shown[0] {
        rows.push(TipRow { series: Series::Tokens, text: format!("Tokens: {}", format_count(Some(day.tokens))), after: None });
    }
    if shown[1] {
        rows.push(TipRow {
            series: Series::Cost,
            text: format!("Cost: {}", money(day.cost)),
            after: Some(format!("Estimated: {}", money(day.estimated_cost))),
        });
    }
    Tip { title: label.to_string(), rows }
}

/// The tooltip's box, where it sits and which way its caret points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TipBox {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub x_align: XAlign,
    pub y_align: YAlign,
    /// The point it points at (the average of the shown points).
    pub caret: [f32; 2],
}

impl Tip {
    fn body_lines(&self) -> usize {
        self.rows.iter().map(|r| 1 + usize::from(r.after.is_some())).sum()
    }

    pub fn size(&self, measure: Measure) -> [f32; 2] {
        let mut h = TIP_PAD * 2.0 + TIP_LINE + TIP_TITLE_GAP;
        let (items, lines) = (self.rows.len() as f32, self.body_lines() as f32);
        if lines > 0.0 {
            h += items * TIP_LINE.max(TIP_BOX) + (lines - items) * TIP_LINE + (lines - 1.0) * TIP_BODY_GAP;
        }
        let mut w = measure(&self.title, TIP_SIZE, true);
        let indent = TIP_BOX + 2.0 + TIP_BOX_PAD;
        for r in &self.rows {
            w = w.max(measure(&r.text, TIP_SIZE, false) + indent);
            if let Some(a) = &r.after {
                w = w.max(measure(a, TIP_SIZE, false) + indent);
            }
        }
        [w + TIP_PAD * 2.0, h]
    }

    /// `positioners.average`, `determineAlignment` and `getBackgroundPoint`: the tooltip beside the average of `points`,
    /// on the side with room, and kept inside the chart.
    pub fn place(&self, layout: &Layout, points: &[[f32; 2]], measure: Measure) -> Option<TipBox> {
        if points.is_empty() {
            return None;
        }
        let n = points.len() as f32;
        let caret = [points.iter().map(|p| p[0]).sum::<f32>() / n, points.iter().map(|p| p[1]).sum::<f32>() / n];
        let [w, h] = self.size(measure);
        let y_align = if caret[1] < h / 2.0 {
            YAlign::Top
        } else if caret[1] > layout.height - h / 2.0 {
            YAlign::Bottom
        } else {
            YAlign::Center
        };
        let reach = TIP_CARET + TIP_CARET_PAD;
        let mut x_align = if y_align == YAlign::Center {
            if caret[0] <= (layout.area.left + layout.area.right) / 2.0 { XAlign::Left } else { XAlign::Right }
        } else if caret[0] <= w / 2.0 {
            XAlign::Left
        } else if caret[0] >= layout.width - w / 2.0 {
            XAlign::Right
        } else {
            XAlign::Center
        };
        if (x_align == XAlign::Left && caret[0] + w + reach > layout.width) || (x_align == XAlign::Right && caret[0] - w - reach < 0.0) {
            x_align = XAlign::Center;
        }
        let mut x = match x_align {
            XAlign::Right => caret[0] - w,
            XAlign::Center => caret[0] - w / 2.0,
            XAlign::Left => caret[0],
        };
        let y = match y_align {
            YAlign::Top => caret[1] + reach,
            YAlign::Bottom => caret[1] - (h + reach),
            YAlign::Center => caret[1] - h / 2.0,
        };
        if y_align == YAlign::Center {
            match x_align {
                XAlign::Left => x += reach,
                XAlign::Right => x -= reach,
                XAlign::Center => {}
            }
        } else {
            match x_align {
                XAlign::Left => x -= TIP_CORNER + TIP_CARET,
                XAlign::Right => x += TIP_CORNER + TIP_CARET,
                XAlign::Center => {}
            }
        }
        let limit = |v: f32, max: f32| v.clamp(0.0, max.max(0.0));
        Some(TipBox { x: limit(x, layout.width - w), y: limit(y, layout.height - h), w, h, x_align, y_align, caret })
    }
}

/// The caret's three corners, in Chart.js's order (`getCaretPosition`).
pub fn caret_points(b: &TipBox) -> [[f32; 2]; 3] {
    let c = TIP_CARET;
    if b.y_align == YAlign::Center {
        let y2 = b.y + b.h / 2.0;
        if b.x_align == XAlign::Left {
            let x1 = b.x;
            [[x1, y2 + c], [x1 - c, y2], [x1, y2 - c]]
        } else {
            let x1 = b.x + b.w;
            [[x1, y2 - c], [x1 + c, y2], [x1, y2 + c]]
        }
    } else {
        let x2 = match b.x_align {
            XAlign::Left => b.x + TIP_CORNER + c,
            XAlign::Right => b.x + b.w - TIP_CORNER - c,
            XAlign::Center => b.caret[0],
        };
        if b.y_align == YAlign::Top {
            let y1 = b.y;
            [[x2 - c, y1], [x2, y1 - c], [x2 + c, y1]]
        } else {
            let y1 = b.y + b.h;
            [[x2 + c, y1], [x2, y1 + c], [x2 - c, y1]]
        }
    }
}

// ---- drawing helpers ----------------------------------------------------------------------------------------------

/// Points along the curve through `points`, `steps` per segment.
pub fn curve(points: &[Point], steps: usize) -> Vec<[f32; 2]> {
    let mut out: Vec<[f32; 2]> = points.first().map(|p| vec![p.at]).unwrap_or_default();
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        for k in 1..=steps {
            let t = k as f32 / steps as f32;
            let u = 1.0 - t;
            let (b0, b1, b2, b3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            out.push([
                b0 * a.at[0] + b1 * a.cp2[0] + b2 * b.cp1[0] + b3 * b.at[0],
                b0 * a.at[1] + b1 * a.cp2[1] + b2 * b.cp1[1] + b3 * b.at[1],
            ]);
        }
    }
    out
}

/// A polyline cut into dashes `on` long with `off` between, counted along its length from the start (`borderDash: [4, 3]`).
pub fn dashes(line: &[[f32; 2]], on: f32, off: f32) -> Vec<Vec<[f32; 2]>> {
    let mut out = Vec::new();
    let mut current: Vec<[f32; 2]> = Vec::new();
    let (mut drawing, mut left) = (true, on);
    if let Some(first) = line.first() {
        current.push(*first);
    }
    for pair in line.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let len = (b[0] - a[0]).hypot(b[1] - a[1]);
        let mut at = 0.0;
        while len - at > left {
            at += left;
            let p = [a[0] + (b[0] - a[0]) * at / len, a[1] + (b[1] - a[1]) * at / len];
            if drawing {
                current.push(p);
                out.push(std::mem::take(&mut current));
            } else {
                current.push(p);
            }
            drawing = !drawing;
            left = if drawing { on } else { off };
        }
        left -= len - at;
        if drawing {
            current.push(b);
        }
    }
    if drawing && current.len() > 1 {
        out.push(current);
    }
    out
}
