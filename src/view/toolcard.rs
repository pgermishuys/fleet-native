//! What opens under a tool call's row. Ports the body of `components/session/ToolCard.vue` (`.tool-preview`,
//! `.tool-body`, `.tool-summary`, the empty state) and `ToolOutputBody.vue` (the bar, the box, the toggles).
//!
//! The diff in a body is `diff.rs`; Copy waits on Rune (no clipboard write from a
//! handler), so the button is drawn and says why it does nothing.
use super::diff::diff_view;
use crate::State;
use crate::conv::card::{ToolBody, shows_diff};
use crate::conv::find::{Marks, plain_ranges};
use crate::conv::output::{Ink, Look, OutputSpan, ParsedOutput, Window, count, parse_tool_output_cached, thousands};
use crate::conv::tools::is_write_call;
use crate::icons::*;
use crate::theme::{Theme, mono, sans};
use rune::color::{srgb_decode, srgb_encode};
use rune::{Color, Cursor, El, ScrollSpec, Span, Tx, col, el, icon, rich, row, text};
use serde_json::Value;
use std::rc::Rc;

/// A write shows this many lines of its diff until asked for the rest (`DIFF_PREVIEW_LINES`).
const DIFF_PREVIEW_LINES: usize = 12;

/// The box's height when expanded (`.tool-output--expanded`).
const EXPANDED_MAX_H: f32 = 340.0;

/// `color-mix(in srgb, a, b p%)`: Fleet mixes in sRGB, Rune in linear light.
pub(super) fn mix(a: Color, b: Color, from_b: f32) -> Color {
    let m = |x: f32, y: f32| srgb_decode(srgb_encode(x) + (srgb_encode(y) - srgb_encode(x)) * from_b);
    Color { r: m(a.r, b.r), g: m(a.g, b.g), b: m(a.b, b.b), a: a.a + (b.a - a.a) * from_b }
}

fn rgb(r: u8, g: u8, b: u8) -> Color {
    let ch = |v: u8| srgb_decode(f32::from(v) / 255.0);
    Color { r: ch(r), g: ch(g), b: ch(b), a: 1.0 }
}

/// The colour a code names, in Fleet's palette (`ToolOutputBody.vue` `.ansi-fg-*`).
pub fn foreground(t: &Theme, ink: Ink) -> Color {
    match ink {
        Ink::Rgb(r, g, b) => rgb(r, g, b),
        Ink::Named { index, bright } => {
            let base = match index {
                0 => return t.muted.alpha(0.8),
                1 => t.error_text,
                2 => t.running_text,
                3 => t.idle_text,
                4 => t.complete_text,
                5 => t.queued_text,
                6 => mix(t.complete_text, t.running_text, 0.5),
                _ => return t.text,
            };
            // A bright colour is 75% of itself and 25% of the text colour.
            if bright { mix(base, t.text, 0.25) } else { base }
        }
    }
}

/// The same tokens at 22% behind the text (`.ansi-bg-*`); black is 25% muted and white 18% text.
pub fn background(t: &Theme, ink: Ink) -> Color {
    match ink {
        Ink::Rgb(r, g, b) => rgb(r, g, b),
        Ink::Named { index, .. } => match index {
            0 => t.muted.alpha(0.25),
            1 => t.error.alpha(0.22),
            2 => t.running.alpha(0.22),
            3 => t.idle.alpha(0.22),
            4 => t.complete.alpha(0.22),
            5 => t.queued.alpha(0.22),
            6 => mix(t.complete, t.running, 0.5).alpha(0.22),
            _ => t.text.alpha(0.18),
        },
    }
}

/// A run's `Span` over bytes `start..end` of the box's text; `None` for plain text.
fn span_of(t: &Theme, run: &OutputSpan, start: usize, end: usize) -> Option<Span> {
    let l: &Look = &run.look;
    if *l == Look::default() {
        return None;
    }
    let mut s = Span::new(start, end);
    if let Some(ink) = l.fg {
        let c = foreground(t, ink);
        s = s.color(if l.dim { c.alpha(c.a * 0.6) } else { c });
    } else if l.dim {
        s = s.color(t.muted.alpha(0.6));
    }
    if let Some(ink) = l.bg {
        s = s.bg(background(t, ink), 0.0);
    }
    if l.bold {
        s = s.weight(600.0);
    }
    if l.italic {
        s = s.italic();
    }
    if l.underline {
        s = s.underline(l.fg.map_or(t.muted, |i| foreground(t, i)));
    }
    Some(s)
}

/// The last `window.shown` lines as one text with a span per styled run.
fn box_text(t: &Theme, parsed: &ParsedOutput, window: Window, marks: Option<&Marks>) -> El {
    let from = window.total - window.shown;
    let mut content = String::new();
    let mut spans = Vec::new();
    for (i, line) in parsed.lines[from..].iter().enumerate() {
        if i > 0 {
            content.push('\n');
        }
        for run in line {
            let start = content.len();
            content.push_str(&run.text);
            spans.extend(span_of(t, run, start, content.len()));
        }
    }
    // Find's marks over what the box draws. The current match counts from the top of the whole output, so those in the
    // lines above the box are left out of the numbering.
    let washes = marks.map_or_else(Vec::new, |m| {
        let above =
            parsed.lines[..from].iter().map(|l| l.iter().map(|r| r.text.as_str()).collect::<String>()).collect::<Vec<_>>().join("\n");
        let before = plain_ranges(&above, &m.query).len();
        super::marks::highlights(t, &plain_ranges(&content, &m.query), m.current.and_then(|c| c.checked_sub(before)))
    });
    rich(content, spans).font(mono(12.0, 400.0, 1.55)).color(t.muted).selectable(true).text_highlights(washes)
}

/// The grey text button above and below the box ("Show 188 earlier lines").
fn toggle(t: &Theme, key: String, glyph: &'static rune::Icon, label: String, on_press: impl Fn(&mut State, &mut Tx) + 'static) -> El {
    row()
        .key(key)
        .gap(4.0)
        .center_y()
        .cursor(Cursor::Pointer)
        .hover_group()
        .label(label.clone())
        .on_press_tx(on_press)
        .child(icon(glyph, 12.0).color(t.muted).group_color(t.text))
        .child(text(label).font(sans(12.0, 400.0, 1.3)).color(t.muted).group_color(t.text))
}

/// `.tool-output-body`: the bar (a shell call's header, Copy), the box, and the toggles for the lines past the last 12.
pub fn output_body(t: &Theme, s: &State, id: &str, body: &ToolBody) -> El {
    let parsed: Rc<ParsedOutput> = parse_tool_output_cached(&body.output);
    let expanded = s.tool_wide.contains(id);
    let window = Window::of(parsed.lines.len(), expanded);
    let header = body.header.clone().unwrap_or_default();
    let mut bar = row().key(format!("tool-{id}-bar")).w_full().gap(8.0).center_y().mb(5.0).child(
        text(header.clone())
            .key(format!("tool-{id}-header"))
            .font(mono(12.0, 400.0, 1.3))
            .color(t.muted)
            .truncate()
            .grow(1.0)
            .help_when_truncated(header),
    );
    // Copy takes the whole output as plain text; Rune can't write the clipboard from a handler yet (docs/RUNE.md).
    bar = bar.child(
        el().key(format!("tool-{id}-copy"))
            .size(22.0, 22.0)
            .center()
            .radius(6.0)
            .opacity(0.5)
            .help("Copy output (waits on Rune: no clipboard write from a handler yet)")
            .label("Copy output")
            .child(icon(&COPY, 13.0).color(t.muted)),
    );
    let mut out = col().key(format!("tool-{id}-output-body")).w_full().mb(4.0).child(bar);
    let key = id.to_string();
    if window.is_long && !expanded {
        let label = format!("Show {}", count(window.hidden(), "earlier line"));
        out =
            out.child(el().mb(5.0).child(toggle(t, format!("tool-{id}-expand"), &CHEVRON_UP, label, move |s: &mut State, tx: &mut Tx| {
                s.tool_wide.insert(key.clone());
                // The box opens on the same last lines the row showed, so the eye doesn't lose its place.
                tx.scroll_to_bottom_instant(format!("tool-{key}-output"));
            })));
    }
    if window.capped {
        out = out.child(
            text(format!("Showing the last {} of {}. Copy takes all of them.", count(window.shown, "line"), thousands(window.total)))
                .key(format!("tool-{id}-capped"))
                .font(sans(12.0, 400.0, 1.3))
                .color(t.muted)
                .wrap()
                .mb(5.0),
        );
    }
    // `.tool-output`: 8x10 padding, a 1px border, the main background at 60%.
    let mut inner = el()
        .key(format!("tool-{id}-output"))
        .w_full()
        .px(10.0)
        .py(8.0)
        .radius(8.0)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.main_bg.alpha(0.6));
    inner = inner.child(box_text(t, &parsed, window, s.find_marks(id, true).as_ref()));
    if expanded {
        inner = inner.max_h(EXPANDED_MAX_H).scroll(ScrollSpec { stick_bottom: true });
    }
    out = out.child(inner);
    if window.is_long && expanded {
        let key = id.to_string();
        let label = "Show only the last 12 lines".to_string();
        out = out.child(el().mt(5.0).child(toggle(
            t,
            format!("tool-{id}-collapse"),
            &CHEVRON_DOWN,
            label,
            move |s: &mut State, _: &mut Tx| {
                s.tool_wide.remove(&key);
            },
        )));
    }
    out
}

/// `.tool-preview`: the line under a failed call's closed row.
pub fn preview(t: &Theme, id: &str, line: &str) -> El {
    text(line.to_string())
        .key(format!("tool-{id}-preview"))
        .font(mono(12.0, 400.0, 1.5))
        .color(t.muted)
        .wrap()
        .w_full()
        .pl(31.0)
        .pr(8.0)
        .pb(6.0)
}

/// `.tool-body`: summary, output, or the empty state. A mod's `ToolResult` tree (`mod_body`: the call's id and the checked tree) replaces
/// them: a `Fleet` node draws them in place, and a `null` tree leaves the body empty (not "No output captured").
pub fn body(t: &Theme, s: &State, id: &str, tool: &str, b: &ToolBody, mod_body: Option<(&str, &Value)>) -> El {
    let inner = col().key(format!("tool-{id}-body")).w_full().pl(31.0).pr(8.0).pt(2.0).pb(8.0);
    match mod_body {
        Some((call, tree)) => {
            let own = fleet_body(t, s, id, tool, b, col().w_full());
            inner.child(super::mods::row::body_tree(s, t, call, tree, own))
        }
        None => fleet_body(t, s, id, tool, b, inner),
    }
}

/// Fleet's own body, added to `inner`.
fn fleet_body(t: &Theme, s: &State, id: &str, tool: &str, b: &ToolBody, mut inner: El) -> El {
    let diff = shows_diff(tool, b, s.inline_diffs);
    if !b.summary.is_empty() {
        inner = inner
            .child(text(b.summary.clone()).key(format!("tool-{id}-summary")).font(sans(12.0, 400.0, 1.6)).color(t.muted).wrap().mb(8.0));
    }
    if diff {
        inner = inner.child(diff_body(t, s, id, tool, b));
    }
    if let Some(visual) = &b.visual {
        inner = inner.child(visual_box(t, id, visual));
    } else if !b.output.is_empty() && !(is_write_call(tool) && diff) {
        // A write that has its diff shows that instead of what it printed.
        inner = inner.child(output_body(t, s, id, b));
    }
    if b.summary.is_empty() && b.output.is_empty() && !diff {
        inner = inner.child(text("No output captured").key(format!("tool-{id}-empty")).font(sans(14.0, 400.0, 1.4)).color(t.muted).py(7.0));
    }
    inner
}

/// `.tool-visual`: what the agent rendered, in a box of its own, with Expand under it. Markdown is drawn here as it is in the tab;
/// an HTML page is a card, since a web view can't sit inside a list (decision 1).
fn visual_box(t: &Theme, id: &str, visual: &crate::fleet::canvases::VisualPayload) -> El {
    use crate::fleet::canvases::VisualType;
    let key = format!("tool-{id}-visual");
    let open = visual.clone();
    let expand = move |s: &mut State| s.expand_visual(open.clone());
    let inner = match visual.kind {
        VisualType::Markdown => col()
            .w_full()
            .gap(8.0)
            .child(super::prose::prose(&super::prose::style(t), &format!("{key}-markdown"), &visual.content, false))
            .child(super::web_card::expand_button(t, &format!("{key}-expand"), expand)),
        VisualType::Html => super::web_card::html_card(t, &format!("{key}-html"), &crate::fleet::canvases::visual_title(visual), expand),
    };
    // margin 8 0, padding 12, a 1 px border, `--radius-card`, `--panel-bg`.
    col()
        .key(key)
        .w_full()
        .my(8.0)
        .padding(12.0)
        .radius(crate::theme::RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.panel_bg)
        .child(inner)
}

/// The card's diff; a write shows its first 12 lines and "Show all N lines" (`.tool-diff-more`).
fn diff_body(t: &Theme, s: &State, id: &str, tool: &str, b: &ToolBody) -> El {
    let key = format!("tool-{id}-diff");
    let cut = is_write_call(tool) && !s.diff_all.contains(id) && b.diff.len() > DIFF_PREVIEW_LINES;
    let shown = if cut { &b.diff[..DIFF_PREVIEW_LINES] } else { &b.diff[..] };
    let mut out = col().w_full().child(diff_view(t, s, &key, shown));
    if cut {
        let all = id.to_string();
        let label = format!("Show all {} lines", b.diff.len());
        out = out.child(
            row()
                .key(format!("{key}-more"))
                .mt(4.0)
                .cursor(Cursor::Pointer)
                .label(label.clone())
                .on_press(move |s: &mut State| {
                    s.diff_all.insert(all.clone());
                })
                .child(text(label).font(sans(12.0, 400.0, 1.3)).color(t.muted)),
        );
    }
    out.mb(if cut { 4.0 } else { 0.0 })
}
