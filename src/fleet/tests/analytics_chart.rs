//! Component 20, piece G: the chart's numbers against Chart.js 4.5's own. `analytics_chart_cases.json` holds, for each case,
//! what Chart.js answered for the same data in a headless browser whose text had a fixed width (0.55 of the size per
//! character, 0.6 bold), so the numbers don't depend on a machine's fonts: the chart area, the tick labels and their
//! positions, the x labels kept, the points with their control points, the legend's boxes and the tooltip's box.
use super::super::analytics::Daily;
use super::super::analytics::chart::*;
use crate::composer::format::{format_count, format_money, format_tokens};
use serde_json::Value;

fn fixed(text: &str, size: f32, bold: bool) -> f32 {
    text.chars().count() as f32 * size * if bold { 0.6 } else { 0.55 }
}

fn cases() -> Vec<Value> {
    serde_json::from_str(include_str!("analytics_chart_cases.json")).expect("the cases file")
}

/// Numbers from the cases file; Chart.js has no control points for a single day (`NaN`, which JSON writes as null).
fn nums(v: &Value) -> Vec<f32> {
    v.as_array().unwrap().iter().map(|n| n.as_f64().map_or(f32::NAN, |n| n as f32)).collect()
}

fn floats(v: &Value) -> Vec<f64> {
    v.as_array().unwrap().iter().map(|n| n.as_f64().unwrap()).collect()
}

fn near(a: f32, b: f32, what: &str, case: &str) {
    assert!((a - b).abs() < 0.01, "{case}: {what}: got {a}, Chart.js has {b}");
}

struct Run {
    name: String,
    layout: Layout,
    tip_box: Option<TipBox>,
    shown: [bool; 2],
    expect: Value,
}

fn run(c: &Value) -> Run {
    let name = c["name"].as_str().unwrap().to_string();
    let labels: Vec<String> = c["labels"].as_array().unwrap().iter().map(|l| l.as_str().unwrap().to_string()).collect();
    let (tokens, cost) = (floats(&c["tokens"]), floats(&c["cost"]));
    let hidden = c.get("hidden").map(|h| h.as_array().unwrap().iter().map(|b| b.as_bool().unwrap()).collect::<Vec<_>>());
    let hidden = hidden.unwrap_or_else(|| vec![false, false]);
    let shown = [!hidden[0], !hidden[1]];
    let left: &dyn Fn(f64) -> String = &|v| format_tokens(Some(v));
    let right: &dyn Fn(f64) -> String = &|v| format_money(Some(v));
    let layout = layout(
        &Input {
            labels: &labels,
            tokens: &tokens,
            cost: &cost,
            shown,
            width: c["width"].as_f64().unwrap() as f32,
            height: HEIGHT,
            left_label: left,
            right_label: right,
        },
        &fixed,
    );
    let hover = c["hover"].as_u64().unwrap() as usize;
    let day = Daily { date: String::new(), tokens: tokens[hover], cost: cost[hover], ..Daily::default() };
    let tip = tip(&day, &labels[hover], shown);
    let points: Vec<[f32; 2]> = Series::ALL.iter().filter(|s| shown[s.index()]).map(|s| layout.points(*s)[hover].at).collect();
    let tip_box = tip.place(&layout, &points, &fixed);
    Run { name, layout, tip_box, shown, expect: c["expect"].clone() }
}

#[test]
fn the_chart_area_is_chart_js_s_in_every_case() {
    for c in cases() {
        let r = run(&c);
        let a = nums(&r.expect["area"]);
        let got = r.layout.area;
        for (g, e, w) in [(got.left, a[0], "left"), (got.top, a[1], "top"), (got.right, a[2], "right"), (got.bottom, a[3], "bottom")] {
            near(g, e, w, &r.name);
        }
    }
}

#[test]
fn the_axes_keep_chart_js_s_ticks_with_the_same_words_and_places() {
    for c in cases() {
        let r = run(&c);
        for (side, ticks) in [("left", &r.layout.left), ("right", &r.layout.right)] {
            let expect = r.expect[side].as_array().unwrap();
            let words: Vec<&str> = ticks.iter().map(|t| t.text.as_str()).collect();
            let want: Vec<&str> = expect.iter().map(|t| t[0].as_str().unwrap()).collect();
            assert_eq!(words, want, "{}: the {side} axis's labels", r.name);
            for (t, e) in ticks.iter().zip(expect) {
                near(t.y, e[1].as_f64().unwrap() as f32, &format!("{side} tick {}", t.text), &r.name);
            }
        }
    }
}

#[test]
fn the_x_axis_skips_labels_as_chart_js_does_at_12_31_and_90_days() {
    for c in cases() {
        let r = run(&c);
        let expect = r.expect["x"].as_array().unwrap();
        let words: Vec<&str> = r.layout.x.iter().map(|t| t.text.as_str()).collect();
        let want: Vec<&str> = expect.iter().map(|t| t[0].as_str().unwrap()).collect();
        assert_eq!(words, want, "{}: the labels kept", r.name);
        for (t, e) in r.layout.x.iter().zip(expect) {
            near(t.x, e[1].as_f64().unwrap() as f32, &format!("x label {}", t.text), &r.name);
        }
    }
    let kept = |name: &str| run(&cases().into_iter().find(|c| c["name"] == name).unwrap()).layout.x.len();
    assert_eq!((kept("twelve"), kept("thirty-one"), kept("ninety")), (12, 8, 12));
}

#[test]
fn the_points_and_their_control_points_are_chart_js_s_and_never_leave_the_area() {
    for c in cases() {
        let r = run(&c);
        for (s, points) in [("tokens", &r.layout.tokens), ("cost", &r.layout.cost)] {
            // Chart.js doesn't lay out a hidden line's points at all (they stay at its off-screen mark).
            if !r.shown[usize::from(s == "cost")] {
                continue;
            }
            let expect = r.expect[s].as_array().unwrap();
            assert_eq!(points.len(), expect.len(), "{}: {s} points", r.name);
            for (i, (p, e)) in points.iter().zip(expect).enumerate() {
                let e = nums(e);
                let got = [p.at[0], p.at[1], p.cp1[0], p.cp1[1], p.cp2[0], p.cp2[1]];
                for (k, name) in ["x", "y", "cp1x", "cp1y", "cp2x", "cp2y"].iter().enumerate() {
                    if !e[k].is_nan() {
                        near(got[k], e[k], &format!("{s} point {i} {name}"), &r.name);
                    }
                }
                let a = r.layout.area;
                for cp in [p.cp1, p.cp2] {
                    assert!(a.contains(cp[0], cp[1]), "{}: {s} point {i}'s control point {cp:?} left the area", r.name);
                }
            }
        }
    }
}

#[test]
fn the_legend_boxes_are_chart_js_s_right_aligned_with_the_line_before_the_word() {
    for c in cases() {
        let r = run(&c);
        let expect = r.expect["legend"].as_array().unwrap();
        for (item, e) in r.layout.legend.iter().zip(expect) {
            let e = nums(e);
            for (k, (got, want)) in item.hit.iter().zip(&e).enumerate() {
                near(*got, *want, &format!("{} legend box {k}", item.series.label()), &r.name);
            }
            near(item.text_x, item.hit[0] + 18.0, "the word starts after the line and half a font size", &r.name);
        }
    }
}

#[test]
fn the_tooltip_box_sits_where_chart_js_puts_it_and_flips_side_near_an_edge() {
    for c in cases() {
        let r = run(&c);
        let b = r.tip_box.expect("a tooltip");
        let e = nums(&r.expect["tip"]["box"]);
        let lines = if r.shown[1] { "with the Estimated line" } else { "without it" };
        // The harness's cost line carries "Estimated: $0.00" under Cost; the rest is the same words.
        for (g, e, w) in [(b.x, e[0], "x"), (b.y, e[1], "y"), (b.w, e[2], "width"), (b.h, e[3], "height")] {
            near(g, e, &format!("the tooltip's {w} ({lines})"), &r.name);
        }
        let align = |x: XAlign| match x {
            XAlign::Left => "left",
            XAlign::Center => "center",
            XAlign::Right => "right",
        };
        let valign = |y: YAlign| match y {
            YAlign::Top => "top",
            YAlign::Center => "center",
            YAlign::Bottom => "bottom",
        };
        assert_eq!(align(b.x_align), r.expect["tip"]["xa"].as_str().unwrap(), "{}: the side", r.name);
        assert_eq!(valign(b.y_align), r.expect["tip"]["ya"].as_str().unwrap(), "{}: above, beside or below", r.name);
    }
}

#[test]
fn the_tooltip_says_the_day_then_tokens_then_cost_and_the_estimate() {
    let day = Daily { date: "2026-11-12".into(), tokens: 1_018_299.0, cost: 1.94, estimated_cost: 2.11, ..Daily::default() };
    let t = tip(&day, "Nov 12", [true, true]);
    assert_eq!(t.title, "Nov 12");
    let words: Vec<(&str, Option<&str>)> = t.rows.iter().map(|r| (r.text.as_str(), r.after.as_deref())).collect();
    assert_eq!(words, [("Tokens: 1,018,299", None), ("Cost: $1.94", Some("Estimated: $2.11"))]);
    // A hidden line's row is left out, and with it the estimate.
    let only = tip(&day, "Nov 12", [true, false]);
    assert_eq!(only.rows.len(), 1);
    assert_eq!(format_count(Some(1_018_299.0)), "1,018,299");
}

#[test]
fn a_pointer_picks_the_nearest_day_with_the_first_of_two_the_same_distance() {
    let c = cases().into_iter().find(|c| c["name"] == "twelve").unwrap();
    let r = run(&c);
    let xs: Vec<f32> = r.layout.tokens.iter().map(|p| p.at[0]).collect();
    assert_eq!(r.layout.nearest(-50.0), Some(0), "left of the first day");
    assert_eq!(r.layout.nearest(10_000.0), Some(11), "right of the last day");
    assert_eq!(r.layout.nearest(xs[4] + 1.0), Some(4));
    assert_eq!(r.layout.nearest((xs[4] + xs[5]) / 2.0), Some(4), "halfway: the first");
    assert_eq!(r.layout.nearest((xs[4] + xs[5]) / 2.0 + 0.5), Some(5));
}

#[test]
fn nice_ticks_for_the_pictures_data_start_at_zero_and_are_round() {
    assert_eq!(nice_ticks(Some(8_000_000.0), 11), [0.0, 1e6, 2e6, 3e6, 4e6, 5e6, 6e6, 7e6, 8e6].to_vec());
    assert_eq!(nice_ticks(Some(74.0), 11).last(), Some(&80.0));
    assert_eq!(nice_ticks(Some(0.0), 11).first(), Some(&0.0), "all zeros: 0 to 1");
    assert_eq!(nice_ticks(Some(0.0), 11).last(), Some(&1.0));
    assert_eq!(nice_ticks(None, 11).last(), Some(&1.0), "a hidden line's axis has no data: 0 to 1");
    let tiny = nice_ticks(Some(0.0012), 11);
    assert_eq!(tiny.first(), Some(&0.0));
    assert!((tiny.last().unwrap() - 0.0012).abs() < 1e-12);
}

#[test]
fn dashes_are_four_long_with_three_between_from_the_start() {
    let line = [[0.0, 0.0], [20.0, 0.0]];
    let d = dashes(&line, 4.0, 3.0);
    let spans: Vec<(f32, f32)> = d.iter().map(|s| (s[0][0], s.last().unwrap()[0])).collect();
    assert_eq!(spans, [(0.0, 4.0), (7.0, 11.0), (14.0, 18.0)]);
}

#[test]
fn a_bare_day_reads_as_formatdate_writes_it() {
    assert_eq!(format_day("2026-11-20", 2026), "Nov 20");
    assert_eq!(format_day("2025-12-03", 2026), "Dec 3, 2025");
    assert_eq!(format_day("", 2026), "");
    assert_eq!(
        chart_label(&["Oct 23".into(), "Nov 20".into()]),
        "Line chart of daily tokens and cost, Oct 23 to Nov 20. The numbers follow in a table."
    );
    assert_eq!(chart_label(&[]), "Daily tokens and cost");
}

#[test]
fn money_in_the_estimate_is_never_negative_or_unreadable() {
    assert_eq!(format_money(Some(0.0)), "$0.00");
}
