//! `WorkflowCanvas.vue`: the designer's canvas. The steps top to bottom between the Starts from box and the End box, a + before
//! every step and after the last, and loops drawn as dashed arcs on the left from where each node was laid out. State is
//! `fleet::workflows::canvas`; the frame that holds it is `view::workflows::editor`.
use crate::State;
use crate::fleet::workflows::canvas::Add;
use crate::fleet::workflows::draft::{WorkflowDraft, is_loop, target_title};
use crate::fleet::workflows::editor::{Editor, EditorView};
use crate::fleet::workflows::helpers::model_short_name;
use crate::fleet::workflows::{ModelRoles, Role, Step, StepKind};
use crate::icons::{BOT, CIRCLE_ALERT, CLOCK, FILE_TEXT, GIT_FORK, PLUS, SETTINGS, USER_ROUND};
use crate::theme::{RADIUS_CARD, Theme, mono, sans};
use rune::{Align, Color, Cursor, El, Icon, LineCap, ScrollSpec, Shape, Side, Span, col, el, icon, rich, row, text};
use std::f32::consts::FRAC_PI_2;

/// `--wf-border-strong`: `color-mix(in srgb, var(--text) 16%, transparent)`.
fn strong(t: &Theme) -> Color {
    t.text.alpha(0.16)
}

/// `color-mix(in srgb, <color> P%, <base>)` where the base is translucent, mixed as CSS premultiplies it.
fn over(color: Color, share: f32, base: Color) -> Color {
    let alpha = share * color.a + (1.0 - share) * base.a;
    let m = |x: f32, y: f32| (share * x * color.a + (1.0 - share) * y * base.a) / alpha;
    Color { r: m(color.r, base.r), g: m(color.g, base.g), b: m(color.b, base.b), a: alpha }
}

/// What a step's model says: a role and what it maps to, a pinned model as it is, or "No model".
pub fn model_label(roles: &ModelRoles, harness: &str, model: Option<&str>) -> String {
    let Some(model) = model.filter(|m| !m.is_empty()) else { return "No model".into() };
    match Role::parse(Some(model)) {
        None => format!("{} (pinned)", model_short_name(Some(model))),
        Some(role) => {
            let mapped = roles.get(harness).and_then(|r| r.get(&role)).and_then(|c| c.model.as_deref());
            format!("{} \u{b7} {}", role.name(), model_short_name(mapped))
        }
    }
}

/// The canvas, to fill what is left of the editor above the problems strip.
pub fn view(s: &State, t: &Theme, ed: &Editor) -> El {
    let Some(draft) = ed.draft.as_ref() else { return el().grow(1.0) };
    let canvas = &ed.canvas;
    let [w, _] = canvas.size;
    // `.wf-flow`: `width: min(400px, calc(100% - 120px))`, left margin `max(96px, calc(50% - 200px))`.
    let flow_w = if w > 0.0 { 400.0_f32.min(w - 120.0) } else { 400.0 };
    let margin = 96.0_f32.max(w / 2.0 - 200.0);
    let machine = s.workflows_machine();
    let roles = s.workflow_model_roles(&machine);
    let harness = s.workflow_default_harness(&machine);

    let mut flow = col()
        .key("workflow-flow")
        .w(flow_w.max(0.0))
        .ml(margin)
        .pt(24.0)
        .pb(40.0)
        .on_layout(|s: &mut State, r| s.workflows_nav.editor.canvas.set_flow(r))
        .child(settings_node(t, canvas.selected.is_none()));
    for (slot, step) in draft.steps.iter().map(Some).chain([None]).enumerate() {
        flow = flow.child(connector(t, slot, canvas.add_at == Some(slot)));
        if let Some(step) = step {
            let errors: Vec<&str> = ed.errors().iter().filter(|e| e.step == Some(slot)).map(|e| e.message.as_str()).collect();
            flow = flow.child(node(
                t,
                draft,
                step,
                slot,
                canvas.selected == Some(slot),
                &errors,
                &model_label(&roles, &harness, step.model.as_deref()),
            ));
        }
    }
    flow = flow.child(end_node(t));
    flow = flow.child(loops(t, ed, draft, margin, flow_w));

    let scroller = col().w_full().h_full().min_h(0.0).child(flow).scroll(ScrollSpec { stick_bottom: false });
    // `.wf-canvas`: the dots (`radial-gradient(var(--wf-border-strong) 1px, transparent 1px)` every 16 px) stay where they are
    // while the steps scroll over them.
    col()
        .key("workflow-canvas")
        .label("Workflow canvas")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .shape(dots(t, canvas.size))
        .on_layout(|s: &mut State, [_, _, w, h]| s.workflows_nav.editor.canvas.set_size([w, h]))
        .child(scroller)
}

fn dots(t: &Theme, [w, h]: [f32; 2]) -> Shape {
    let ink = strong(t);
    let mut shape = Shape::new();
    let (cols, rows) = ((w / 16.0).ceil() as usize, (h / 16.0).ceil() as usize);
    for j in 0..rows {
        for i in 0..cols {
            let (x, y) = (i as f32 * 16.0 + 8.0, j as f32 * 16.0 + 8.0);
            shape = shape.fill(vec![[x - 1.0, y - 1.0], [x + 1.0, y - 1.0], [x + 1.0, y + 1.0], [x - 1.0, y + 1.0]], ink);
        }
    }
    shape
}

/// `.wf-node` as the box every node shares: padding 10/12, a 1 px border, the card radius, 13 px.
fn frame(t: &Theme) -> El {
    col().w_full().px(12.0).py(10.0).radius(RADIUS_CARD).border(1.0, strong(t)).border_layout().fill(t.card_bg)
}

/// `.wf-node--term`: a dashed row on the raised ground, 12 px muted.
fn term(t: &Theme) -> El {
    row()
        .w_full()
        .center_y()
        .px(12.0)
        .py(8.0)
        .radius(RADIUS_CARD)
        .border(1.0, strong(t))
        .border_layout()
        .border_dash(3.0, 3.0)
        .fill(t.mix(t.text, t.card_bg, 0.03))
}

/// "<b>Starts from</b> · a sentence you type": the bold part is the text colour at 500.
fn line(t: &Theme, lead: &str, rest: &str) -> El {
    let words = format!("{lead}{rest}");
    rich(words, vec![Span::new(0, lead.len()).weight(500.0).color(t.text)]).font(sans(12.0, 400.0, 1.5)).color(t.muted)
}

fn selected_look(t: &Theme, e: El, on: bool) -> El {
    if on { e.border(1.0, t.accent).outline(3.0, 0.0, t.accent_dim) } else { e }
}

fn settings_node(t: &Theme, selected: bool) -> El {
    let lines =
        col().gap(2.0).child(line(t, "Starts from", " \u{b7} a sentence you type")).child(line(t, "Runs in", " \u{b7} a new worktree"));
    let go = row()
        .gap(4.0)
        .center_y()
        .child(icon(&SETTINGS, 12.0).color(t.muted))
        .child(text("Workflow settings").font(sans(12.0, 400.0, 1.5)).color(t.muted));
    let node = term(t)
        .key("workflow-canvas-settings")
        .role("button")
        .label("Workflow settings")
        .cursor(Cursor::Pointer)
        .focus_ring(2.0, 0.0, t.accent)
        .child(lines)
        .child(row().grow(1.0))
        .child(go)
        .on_press(|s: &mut State| s.workflows_nav.editor.canvas.selected = None);
    selected_look(t, node, selected)
}

fn end_node(t: &Theme) -> El {
    term(t).key("workflow-canvas-end").child(line(t, "End", " \u{b7} the run's sessions stay in Sessions"))
}

/// `.wf-conn`: 30 px high with a 1 px line down its middle and a 20 px + on it that shows while the pointer is over the
/// connector, while the + has the keyboard, or while its menu is open.
fn connector(t: &Theme, slot: usize, open: bool) -> El {
    let add = col()
        .key(format!("workflow-add-{slot}"))
        .role("button")
        .label("Add a step here")
        .size(20.0, 20.0)
        .center()
        .radius(999.0)
        .border(1.0, strong(t))
        .border_layout()
        .fill(t.panel_bg)
        .cursor(Cursor::Pointer)
        .focus_ring(2.0, 0.0, t.accent)
        .child(icon(&PLUS, 12.0).color(t.muted))
        .on_press(move |s: &mut State| s.workflows_nav.editor.canvas.toggle_add(slot));
    let add = if open { add } else { add.group_opacity(0.0, 1.0) };
    let lane = col().w(20.0).h(30.0).center().child(el().w(1.0).h(30.0).fill(strong(t)).absolute(9.5, 0.0)).child(add);
    row().key(format!("workflow-conn-{slot}")).w_full().h(30.0).justify_center().hover_group().child(lane)
}

fn node(t: &Theme, draft: &WorkflowDraft, step: &Step, slot: usize, selected: bool, errors: &[&str], model: &str) -> El {
    let you = step.kind == StepKind::You;
    let mut e = frame(t).key(format!("workflow-node-{}", step.id)).role("button").cursor(Cursor::Pointer).focus_ring(2.0, 0.0, t.accent);
    if you {
        e = e.border(1.0, over(t.idle, 0.35, strong(t))).fill(t.mix(t.idle, t.card_bg, 0.06));
    }
    if step.optional {
        e = e.border_dash(3.0, 3.0);
    }
    e = selected_look(t, e, selected);
    if !errors.is_empty() {
        e = e.border(1.0, t.error).outline(3.0, 0.0, t.error.alpha(0.15));
    }
    let title = if step.title.is_empty() { "Untitled" } else { step.title.as_str() };
    let icon_box = row()
        .size(24.0, 24.0)
        .center()
        .radius(6.0)
        .fill(if you { t.idle.alpha(0.10) } else { t.wash(8.0) })
        .child(icon(if you { &USER_ROUND } else { &BOT }, 14.0).color(if you { t.idle } else { t.muted }));
    let top = row()
        .w_full()
        .center_y()
        .gap(8.0)
        .child(icon_box)
        .child(text(title.to_string()).font(sans(13.0, 600.0, 1.5)).color(t.text).truncate().grow(1.0).shrink().min_w(0.0))
        .child(text(if you { "You decide" } else { "Agent" }).font(sans(11.0, 400.0, 1.5)).color(t.muted));
    let mut e = e.child(top);
    e = if you { e.child(you_body(t, draft, step)) } else { e.child(agent_body(t, draft, step, slot, model)) };
    for (i, message) in errors.iter().enumerate() {
        e = e.child(
            row()
                .key(format!("workflow-node-error-{i}"))
                .gap(5.0)
                .center_y()
                .mt(6.0)
                .child(icon(&CIRCLE_ALERT, 12.0).color(t.error_text))
                .child(text(message.to_string()).font(sans(11.5, 400.0, 1.5)).color(t.error_text).wrap().grow(1.0).shrink().min_w(0.0)),
        );
    }
    e.on_layout(move |s: &mut State, r| s.workflows_nav.editor.canvas.set_node(slot, r))
        .on_press(move |s: &mut State| s.workflows_nav.editor.canvas.selected = Some(slot))
}

fn you_body(t: &Theme, draft: &WorkflowDraft, step: &Step) -> El {
    let ask = text(step.ask.clone().unwrap_or_default()).font(sans(12.0, 400.0, 1.5)).color(t.muted).wrap().w_full().mt(4.0);
    let mut choices = col().w_full().gap(4.0).mt(8.0);
    for choice in &step.choices {
        let mut r = row()
            .w_full()
            .center_y()
            .px(8.0)
            .py(4.0)
            .radius(6.0)
            .border(1.0, t.border)
            .border_layout()
            .fill(t.panel_bg)
            .child(text(choice.label.clone()).font(sans(12.0, 400.0, 1.5)).color(t.text));
        if choice.note {
            r = r.child(text("\u{a0}(with a note)").font(sans(11.0, 400.0, 1.5)).color(t.muted));
        }
        choices =
            choices
                .child(r.child(row().grow(1.0).min_w(8.0)).child(
                    text(format!("\u{2192} {}", target_title(draft, Some(&choice.to)))).font(sans(11.0, 400.0, 1.5)).color(t.muted),
                ));
    }
    col().w_full().child(ask).child(choices)
}

fn agent_body(t: &Theme, draft: &WorkflowDraft, step: &Step, slot: usize, model: &str) -> El {
    let ink = |s: f32| sans(s, 400.0, 1.5);
    let mut meta = row().w_full().flex_wrap().gaps(4.0, 10.0).mt(6.0).child(
        row().gap(4.0).center_y().child(icon(&BOT, 12.0).color(t.muted)).child(text(model.to_string()).font(ink(12.0)).color(t.muted)),
    );
    if let Some(agent) = step.agent.as_deref().filter(|a| !a.is_empty()) {
        meta = meta.child(text(agent.to_string()).font(ink(12.0)).color(t.muted));
    }
    if let Some(skill) = step.skill.as_deref().filter(|a| !a.is_empty()) {
        meta = meta.child(text(skill.to_string()).font(mono(12.0, 400.0, 1.5)).color(t.muted));
    }
    let mut c = col().w_full().child(meta);
    if step.optional || step.finish_you || step.finish_agent || !step.writes.is_empty() {
        let badge = |edge: Color| row().gap(3.0).center_y().px(6.0).radius(999.0).border(1.0, edge).border_layout();
        let mut badges = row().w_full().flex_wrap().gaps(4.0, 4.0).mt(6.0);
        if step.optional {
            badges = badges.child(badge(strong(t)).child(text("Optional \u{b7} off").font(ink(10.5)).color(t.muted)));
        }
        if step.finish_you {
            badges = badges.child(
                badge(t.accent.alpha(0.45))
                    .child(icon(&USER_ROUND, 12.0).color(t.accent_text))
                    .child(text("You finish").font(ink(10.5)).color(t.accent_text)),
            );
        } else if step.finish_agent {
            badges = badges.child(badge(strong(t)).child(text("Always the agent").font(ink(10.5)).color(t.muted)));
        }
        if !step.writes.is_empty() {
            badges = badges.child(
                badge(strong(t))
                    .child(icon(&FILE_TEXT, 12.0).color(t.muted))
                    .child(text(format!("writes {}", step.writes.len())).font(mono(10.5, 400.0, 1.5)).color(t.muted)),
            );
        }
        c = c.child(badges);
    }
    let mut outcomes = row().w_full().flex_wrap().gaps(4.0, 4.0).mt(6.0);
    for outcome in &step.outcomes {
        let to = step.routes.iter().find(|(o, _)| o == outcome).map(|(_, to)| to.as_str()).filter(|to| !to.is_empty());
        let looping = is_loop(draft, slot, to);
        let words = match to {
            Some(to) => {
                let max = step.max_loops.filter(|m| looping && *m > 0).map(|m| format!(" \u{2264}{m}")).unwrap_or_default();
                format!("{outcome} \u{2192} {}{max}", target_title(draft, Some(to)))
            }
            None => outcome.clone(),
        };
        let (edge, ink_color) = if looping { (t.queued.alpha(0.40), t.queued_text) } else { (t.border, t.muted) };
        outcomes = outcomes.child(
            row()
                .key(format!("workflow-outcome-{outcome}"))
                .px(7.0)
                .py(1.0)
                .radius(999.0)
                .border(1.0, edge)
                .border_layout()
                .child(text(words).font(mono(11.0, 400.0, 1.5)).color(ink_color)),
        );
    }
    c.child(outcomes)
}

// ─── The loops ──────────────────────────────────────────────────────────────────────────────────────────────────

const DASH: f32 = 4.0;
const GAP: f32 = 3.0;
const STROKE: f32 = 1.5;

/// A quadratic curve's points after `from`, to `to` through `control`.
fn quad(path: &mut Vec<[f32; 2]>, control: [f32; 2], to: [f32; 2]) {
    let from = *path.last().unwrap_or(&to);
    for i in 1..=8 {
        let u = i as f32 / 8.0;
        let (a, b, c) = ((1.0 - u) * (1.0 - u), 2.0 * u * (1.0 - u), u * u);
        path.push([a * from[0] + b * control[0] + c * to[0], a * from[1] + b * control[1] + c * to[1]]);
    }
}

/// Rune strokes whole polylines and has no dash pattern: walk the line and make each dash its own open stroke.
fn dashed(shape: Shape, line: &[[f32; 2]], color: Color) -> Shape {
    let mut shape = shape;
    let (mut on, mut left) = (true, DASH);
    let mut current: Vec<[f32; 2]> = vec![line[0]];
    for pair in line.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let len = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
        let mut at = 0.0;
        while len - at > left {
            at += left;
            let p = [a[0] + (b[0] - a[0]) * at / len, a[1] + (b[1] - a[1]) * at / len];
            if on {
                current.push(p);
                shape = shape.stroke_with(std::mem::take(&mut current), false, STROKE, LineCap::Butt, color);
            } else {
                current = vec![p];
            }
            on = !on;
            left = if on { DASH } else { GAP };
        }
        left -= len - at;
        if on {
            current.push(b);
        }
    }
    if on && current.len() > 1 {
        shape = shape.stroke_with(current, false, STROKE, LineCap::Butt, color);
    }
    shape
}

/// One loop, from a step's left edge back to the step it sends work to: the path in the flow's coordinates and where its label
/// sits.
struct Arc {
    path: Vec<[f32; 2]>,
    label: String,
    at: [f32; 2],
    missing_max: bool,
    /// Where the arrow head's tip is.
    tip: [f32; 2],
}

/// `drawLoops`: each looping outcome gets the next lane, 24 px further left.
fn arcs(ed: &Editor, draft: &WorkflowDraft) -> Vec<Arc> {
    let mut out = Vec::new();
    let mut lane = 0;
    for (index, step) in draft.steps.iter().enumerate() {
        for outcome in &step.outcomes {
            let target = step.routes.iter().find(|(o, _)| o == outcome).map(|(_, to)| to.as_str());
            if !is_loop(draft, index, target) {
                continue;
            }
            let Some(to_index) = draft.steps.iter().position(|s| Some(s.id.as_str()) == target) else { continue };
            let (Some(a), Some(b)) = (ed.canvas.place(index), ed.canvas.place(to_index)) else { continue };
            let y1 = a[1] + (a[3] / 2.0).min(22.0);
            let y2 = b[1] + (b[3] / 2.0).min(22.0);
            let x0 = a[0];
            let x = -18.0 - lane as f32 * 24.0;
            let r = 8.0;
            let top = y1.min(y2);
            let mut path = vec![[x0, y1]];
            let end = if index == to_index {
                path = vec![[x0, y1 + 6.0], [x + r, y1 + 6.0]];
                quad(&mut path, [x, y1 + 6.0], [x, y1 - 2.0]);
                quad(&mut path, [x, y1 - 10.0], [x + r, y1 - 10.0]);
                path.push([x0, y1 - 10.0]);
                [x0, y1 - 10.0]
            } else {
                path.push([x + r, y1]);
                quad(&mut path, [x, y1], [x, y1 - r]);
                path.push([x, top + r]);
                quad(&mut path, [x, y2], [x + r, y2]);
                path.push([x0, y2]);
                [x0, y2]
            };
            out.push(Arc {
                path,
                label: format!(
                    "{outcome} \u{b7} {}",
                    match step.max_loops.filter(|m| *m > 0) {
                        Some(m) => format!("at most {m}\u{d7}"),
                        None => "needs a max".to_string(),
                    }
                ),
                at: [x, (y1 + y2) / 2.0],
                missing_max: step.max_loops.is_none_or(|m| m == 0),
                tip: end,
            });
            lane += 1;
        }
    }
    out
}

/// The arcs' ground, over the flow and to its left, and the labels: not hit by the pointer.
fn loops(t: &Theme, ed: &Editor, draft: &WorkflowDraft, margin: f32, flow_w: f32) -> El {
    let [_, flow_h] = ed.canvas.flow_size();
    let found = arcs(ed, draft);
    let mut shape = Shape::new();
    let mut labels = Vec::new();
    for arc in &found {
        let ink = if arc.missing_max { t.error } else { t.queued };
        let shifted: Vec<[f32; 2]> = arc.path.iter().map(|p| [p[0] + margin, p[1]]).collect();
        shape = dashed(shape, &shifted, ink);
        // `marker-end`: a 7-unit box on a 1.5 wide stroke, so its 10-unit triangle is 10.5 points long and 10.5 high, with
        // the end of the line 8 units in; the tip is 2.1 points past the end, pointing right.
        let [x, y] = [arc.tip[0] + margin, arc.tip[1]];
        shape = shape.fill(vec![[x + 2.1, y], [x - 8.4, y - 5.25], [x - 8.4, y + 5.25]], t.queued);
        let (ink_text, edge) = if arc.missing_max { (t.error_text, t.error) } else { (t.queued_text, t.queued.alpha(0.35)) };
        // `translate(-50%, -50%) rotate(-90deg)`: a box centred on the point and turned about its centre.
        let pill = row()
            .key("workflow-loop-label")
            .px(6.0)
            .py(1.0)
            .radius(999.0)
            .border(1.0, edge)
            .border_layout()
            .fill(t.panel_bg)
            .child(text(arc.label.clone()).font(sans(11.0, 400.0, 1.5)).color(ink_text));
        labels.push(col().absolute(arc.at[0] + margin - 100.0, arc.at[1] - 12.0).size(200.0, 24.0).center().rotate(-FRAC_PI_2).child(pill));
    }
    col().key("workflow-loops").absolute(-margin, 0.0).size(margin + flow_w, flow_h).shape(shape).children(labels)
}

// ─── The + menu ─────────────────────────────────────────────────────────────────────────────────────────────────

/// `.wf-add-menu` under the + whose menu is open: Agent step and You decide, and two that arrive later.
pub fn overlay(s: &State, t: &Theme) -> Vec<El> {
    let ed = &s.workflows_nav.editor;
    let Some(at) = ed.canvas.add_at.filter(|_| ed.view == EditorView::Designer && ed.draft.is_some()) else { return Vec::new() };
    let item = |key: &str, glyph: &'static Icon, title: &str, hint: &str, run: Option<fn(&mut State)>| {
        let words = col()
            .grow(1.0)
            .shrink()
            .min_w(0.0)
            .child(text(title.to_string()).font(sans(13.0, 400.0, 1.5)).color(t.text))
            .child(text(hint.to_string()).font(sans(11.0, 400.0, 1.5)).color(t.muted).wrap());
        let r = row()
            .key(key.to_string())
            .role("menuitem")
            .label(title.to_string())
            .w_full()
            .gap(8.0)
            .items_start()
            .px(8.0)
            .py(6.0)
            .radius(6.0)
            .child(icon(glyph, 14.0).color(t.text).mt(2.0))
            .child(words);
        match run {
            Some(go) => r.cursor(Cursor::Pointer).hover(t.wash(5.0)).on_press(go),
            None => r.opacity(0.5).disabled(true),
        }
    };
    let panel = col()
        .key("workflow-add-menu")
        .role("menu")
        .w(250.0)
        .padding(4.0)
        .radius(RADIUS_CARD)
        .border(1.0, strong(t))
        .border_layout()
        .fill(t.card_bg)
        .shadow(12.0, 32.0, Color::BLACK.alpha(0.35))
        .child(item(
            "workflow-add-agent",
            &BOT,
            "Agent step",
            "A new session with instructions and a model",
            Some(|s| s.workflow_canvas_add(Add::Agent)),
        ))
        .child(item(
            "workflow-add-you",
            &USER_ROUND,
            "You decide",
            "Stop and ask; each choice leads somewhere",
            Some(|s| s.workflow_canvas_add(Add::You)),
        ))
        .child(item("workflow-add-parallel", &GIT_FORK, "In parallel", "Arrives with Stage 2", None))
        .child(item("workflow-add-wait", &CLOCK, "Wait", "Arrives with Stage 3", None))
        .modal(true, |s: &mut State| s.workflows_nav.editor.canvas.add_at = None);
    vec![crate::ui::overlay::anchored(panel, &format!("workflow-add-{at}"), Side::Bottom, Align::Center, 1.0)]
}
