//! Ports `components/canvas/ProgressCanvas.vue`: the Progress tab. The plan form (head, flow bar, now line, groups, steps and
//! the live todo list under the current step), the todo form, and the empty state. What each step says and which group is
//! open is worked out in `fleet::progress` and `fleet::progress_feed`. The subagent cards (`ProgressSubagentCard.vue`) are
//! piece H's; this file leaves the place for them.
use crate::State;
use crate::fleet::progress::{
    Plan, PlanGroup, PlanStep, ProgressDetail, TodoItem, TodoStatus, code_spans, current_plan_step, format_time, group_done, group_key,
    group_open_default, is_flat, is_group_complete, last_tick, now_line,
};
use crate::icons::{CHECK, CHEVRON_DOWN, CHEVRON_RIGHT, LIST_CHECKS};
use crate::theme::{RADIUS_BTN, Theme, sans};
use rune::{Align, Cursor, El, Icon, ScrollSpec, Shadow, Span, col, el, icon, rich, row, text};

/// `lucide check` at stroke 3, the ticked step's mark (`stroke-width: 3`); the generated icon is 2.
static CHECK_BOLD: Icon = Icon {
    name: "lucide-check-bold",
    view: 24.0,
    body: r#"<path d="M20 6 9 17l-5-5" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round" fill="none"/>"#,
};

/// The tab's body for the open session.
pub fn progress_tab(s: &State, t: &Theme) -> El {
    let session = s.canvas_session();
    let detail = s.progress.of(&session);
    let offset = crate::conv::time::local_offset_secs();
    let mut body = col().w_full().gap(12.0).padding(14.0).pb(18.0).items_stretch();
    body = match detail {
        Some(d) if d.plan.is_some() => body.child(plan_form(s, t, d, &session, offset)),
        Some(d) if !d.todos.is_empty() => body.child(todo_form(t, d)),
        _ => body.child(empty(t)),
    };
    col()
        .key("progress-tab")
        .role("region")
        .label("Progress")
        .w_full()
        .h_full()
        .child(body.scroll(ScrollSpec { stick_bottom: false }).key("progress-scroll").w_full().grow(1.0).min_h(0.0))
}

/// `.progress-canvas__eyebrow` and the in-step labels: 10 px, 600, uppercase, tracked 0.08em.
fn eyebrow(t: &Theme, key: &str, words: &str) -> El {
    text(words.to_uppercase()).key(key.to_string()).font(sans(10.0, 600.0, 1.6).tracking(0.8)).color(t.muted)
}

fn count(t: &Theme, done: u32, total: u32) -> El {
    let big = |w: f32| sans(20.0, w, 1.1).features("'tnum' 1");
    row()
        .key("progress-count")
        .label(format!("{done} of {total} steps ticked"))
        .center_y()
        .child(text(done.to_string()).font(big(600.0)).color(t.text))
        .child(text(format!("/{total}")).font(big(500.0)).color(t.muted))
}

fn head(t: &Theme, kicker: &str, title: &str, source: Option<String>, done: u32, total: u32) -> El {
    let mut words = col().grow(1.0).shrink().min_w(0.0).gap(3.0).items_stretch();
    words = words.child(eyebrow(t, "progress-eyebrow", kicker));
    words = words.child(text(title.to_string()).key("progress-title").font(sans(14.0, 600.0, 1.5)).color(t.text).wrap());
    if let Some(source) = source {
        words = words.child(text(source).key("progress-source").font(crate::theme::mono(11.0, 400.0, 1.4)).color(t.muted).wrap_anywhere());
    }
    row().w_full().gap(12.0).items_start().child(words).child(count(t, done, total))
}

fn plan_form(s: &State, t: &Theme, d: &ProgressDetail, session: &str, offset: i64) -> El {
    let Some(plan) = d.plan.as_ref() else { return el() };
    let title = plan.title.as_deref().unwrap_or(&plan.path);
    let tracked = format_time(Some(&plan.tracked_since), offset);
    let source = if tracked.is_empty() { plan.path.clone() } else { format!("{} · tracked since {tracked}", plan.path) };
    let current = current_plan_step(plan);
    let mut groups = col().key("progress-groups").w_full().gap(4.0).items_stretch();
    for (i, group) in plan.groups.iter().enumerate() {
        groups = groups.child(group_section(s, t, d, plan, session, i, group, offset));
    }
    col()
        .w_full()
        .gap(12.0)
        .items_stretch()
        .child(head(t, "Plan", title, Some(source), d.summary.done, d.summary.total))
        .child(flow(t, plan, d.summary.done, d.summary.total))
        .child(now(t, plan))
        .child(groups)
        .child(el().w(0.0).h(0.0).key(format!("progress-current-{}", current.map_or(String::new(), |c| c.step.key.clone()))))
}

/// The flow bar: a segment per step (flat) or per group (phased); the current one carries the "now" ring.
fn flow(t: &Theme, plan: &Plan, done: u32, total: u32) -> El {
    let flat = is_flat(plan);
    let current = current_plan_step(plan);
    let mut bar = row().key("progress-flow").role("img").label(format!("{done} of {total} steps ticked")).w_full().h(8.0).gap(3.0);
    let segments: Vec<(f32, f32, bool, bool, String)> = if flat {
        plan.groups
            .iter()
            .flat_map(|g| &g.steps)
            .map(|st| (1.0, if st.checked { 1.0 } else { 0.0 }, current.is_some_and(|c| c.step.key == st.key), false, st.title.clone()))
            .collect()
    } else {
        plan.groups
            .iter()
            .enumerate()
            .map(|(i, g)| {
                let n = g.steps.len();
                (n as f32, group_done(g) as f32 / n as f32, current.is_some_and(|c| c.group_index == i), is_group_complete(g), group_tip(g))
            })
            .collect()
    };
    for (i, (grow, fill, now, complete, tip)) in segments.into_iter().enumerate() {
        let mut seg = el().key(format!("progress-flow-{i}")).grow(grow).min_w(6.0).h(8.0).radius(3.0).fill(t.wash(9.0)).clip_rounded(3.0);
        if !flat {
            seg = seg.help(tip);
        }
        if fill > 0.0 {
            seg = seg.child(el().key(format!("progress-flow-fill-{i}")).w_frac(fill).h(8.0).fill(if complete {
                t.accent.alpha(0.55)
            } else {
                t.accent
            }));
        }
        // The ring is drawn outside the segment, so it sits on a wrapper that doesn't clip.
        let ring = Shadow { x: 0.0, y: 0.0, blur: 0.0, spread: 1.5, color: t.accent, inset: false };
        let wrap = el().grow(grow).min_w(6.0).h(8.0).radius(3.0).child(seg.w_full());
        bar = bar.child(if now { wrap.key(format!("progress-now-{i}")).box_shadow(ring) } else { wrap });
    }
    bar
}

fn group_tip(g: &PlanGroup) -> String {
    format!("{}: {}/{}", g.title.as_deref().unwrap_or("Steps"), group_done(g), g.steps.len())
}

/// `progress-canvas__now`: the group in bold, then where the next step is.
fn now(t: &Theme, plan: &Plan) -> El {
    let line = now_line(plan);
    let lead = line.lead.clone().unwrap_or_default();
    let content = format!("{lead}{}", line.rest);
    let spans = if lead.is_empty() { vec![] } else { vec![Span::new(0, lead.len()).color(t.text).weight(500.0)] };
    rich(content, spans).key("progress-now").font(sans(12.0, 400.0, 1.4)).color(t.muted).wrap()
}

#[allow(clippy::too_many_arguments)]
fn group_section(s: &State, t: &Theme, d: &ProgressDetail, plan: &Plan, session: &str, index: usize, group: &PlanGroup, offset: i64) -> El {
    let flat = is_flat(plan);
    let key = group_key(index, group);
    let open = s.progress.group_open(session, &key, group_open_default(plan, group));
    let mut sec = col().key(format!("progress-group-{index}")).w_full().items_stretch();
    if !flat {
        sec = sec.child(group_head(s, t, plan, session, index, group, open, offset));
    }
    if open {
        let current = current_plan_step(plan);
        let mut steps = col().key(format!("progress-steps-{index}")).w_full().pb(4.0).items_stretch();
        for step in &group.steps {
            let is_current = current.is_some_and(|c| c.step.key == step.key);
            steps = steps.child(step_row(t, d, step, is_current, offset));
        }
        sec = sec.child(steps);
    }
    sec
}

#[allow(clippy::too_many_arguments)]
fn group_head(s: &State, t: &Theme, _plan: &Plan, session: &str, index: usize, group: &PlanGroup, open: bool, offset: i64) -> El {
    let _ = s;
    let done = is_group_complete(group);
    let title = group.title.clone().unwrap_or_else(|| "Steps".into());
    let mut meta = row().gap(4.0).center_y();
    if done {
        meta = meta.child(icon(&CHECK, 12.0).color(t.running));
    }
    let mut counts = format!("{}/{}", group_done(group), group.steps.len());
    if done && let Some(at) = last_tick(group).map(|iso| format_time(Some(iso), offset)).filter(|c| !c.is_empty()) {
        counts.push_str(&format!(" · {at}"));
    }
    meta = meta.child(text(counts).font(sans(11.5, 400.0, 1.3).features("'tnum' 1")).color(t.muted));
    let (session, key) = (session.to_string(), group_key(index, group));
    let default = !done;
    row()
        .key(format!("progress-group-head-{index}"))
        .role("button")
        .label(title.clone())
        .w_full()
        .gap(7.0)
        .padding(6.0)
        .center_y()
        .radius(RADIUS_BTN)
        .cursor(Cursor::Pointer)
        .hover(t.wash(5.0))
        .focusable(true)
        .focus_ring(2.0, -2.0, t.accent)
        .child(icon(if open { &CHEVRON_DOWN } else { &CHEVRON_RIGHT }, 12.0).color(t.muted))
        .child(
            text(title)
                .font(sans(12.0, if done { 500.0 } else { 600.0 }, 1.3))
                .color(if done { t.muted } else { t.text })
                .truncate()
                .grow(1.0)
                .min_w(0.0),
        )
        .child(meta)
        .on_press(move |s: &mut State| s.progress.toggle_group(&session, &key, default))
}

fn glyph(t: &Theme, step: &PlanStep, current: bool) -> El {
    let ring = el().size(14.0, 14.0).min_w(14.0).mt(1.0).radius(if step.checked { 4.0 } else { 7.0 }).center();
    if step.checked {
        ring.fill(t.accent).border(1.5, t.accent).child(icon(&CHECK_BOLD, 10.0).color(t.primary_fg))
    } else if current {
        ring.fill(t.card_bg).border(1.5, t.accent).child(el().size(6.0, 6.0).radius(3.0).fill(t.accent))
    } else {
        ring.border(1.5, t.wash(22.0))
    }
}

/// A step title: `` `code` `` pieces as inline code, then the sub-checklist count.
fn step_title(t: &Theme, step: &PlanStep) -> El {
    let ink = if step.checked { t.muted } else { t.text };
    let (mut content, mut spans) = (String::new(), Vec::new());
    for piece in code_spans(&step.title) {
        let at = content.len();
        content.push_str(&piece.text);
        if piece.code {
            spans.push(Span::new(at, content.len()).mono().size(11.0).color(ink).bg(t.wash(7.0), 3.0));
        }
    }
    if step.sub_total > 0 {
        content.push_str("  ");
        let at = content.len();
        content.push_str(&format!("{}/{}", step.sub_done, step.sub_total));
        spans.push(Span::new(at, content.len()).size(11.0).color(t.muted));
    }
    rich(content, spans).font(sans(12.5, 400.0, 1.4)).color(ink).wrap_anywhere().grow(1.0).shrink().min_w(0.0)
}

fn step_row(t: &Theme, d: &ProgressDetail, step: &PlanStep, current: bool, offset: i64) -> El {
    let number = step.number.clone().unwrap_or_default();
    let meta = if step.checked {
        format_time(step.ticked_at.as_deref(), offset)
    } else if current {
        "next".into()
    } else {
        String::new()
    };
    let meta_ink = if current && !step.checked { t.accent_text } else { t.muted };
    let meta_weight = if current && !step.checked { 500.0 } else { 400.0 };
    let line = row()
        .w_full()
        .gap(6.0)
        .items_start()
        .child(row().w(16.0).min_w(16.0).child(glyph(t, step, current)))
        .child(
            row()
                .w(20.0)
                .min_w(20.0)
                .pt(1.0)
                .justify_end()
                .child(text(number).font(sans(11.5, 400.0, 1.4).features("'tnum' 1")).color(t.muted)),
        )
        .child(step_title(t, step))
        .child(text(meta).font(sans(11.0, meta_weight, 1.4).features("'tnum' 1")).color(meta_ink).pt(2.0));
    let mut r = col().key(format!("progress-step-{}", step.key)).w_full().padding(6.0).radius(RADIUS_BTN).items_stretch();
    if current {
        r = r.fill(t.accent.alpha(0.07));
    }
    r = r.child(line);
    if current && !d.todos.is_empty() {
        let done = d.todos.iter().filter(|x| x.status == TodoStatus::Completed).count();
        let label = format!("Todos · {done} of {}", d.todos.len());
        // Under the title and meta columns: past the glyph, the number and their gaps (16 + 6 + 20 + 6, less the row's own 6 of padding).
        r = r.child(
            col()
                .w_full()
                .gap(6.0)
                .mt(7.0)
                .pl(48.0)
                .items_stretch()
                .child(eyebrow(t, "progress-todos-label", &label))
                .child(todo_list(t, d, 3.0)),
        );
    }
    r
}

fn todo_list(t: &Theme, d: &ProgressDetail, gap: f32) -> El {
    let mut list = col().key("progress-todos").w_full().gap(gap).items_stretch();
    for (i, item) in d.todos.iter().enumerate() {
        list = list.child(todo(t, i, item));
    }
    list
}

fn todo(t: &Theme, i: usize, item: &TodoItem) -> El {
    let ink = match item.status {
        TodoStatus::InProgress => t.text,
        TodoStatus::Completed | TodoStatus::Cancelled => t.muted,
        TodoStatus::Pending => t.text.alpha(0.75),
    };
    let weight = if item.status == TodoStatus::InProgress { 500.0 } else { 400.0 };
    let mark = el().size(10.0, 10.0).min_w(10.0).mt(3.0).radius(5.0).center();
    let mark = match item.status {
        TodoStatus::InProgress => mark.border(1.5, t.accent).child(el().size(4.5, 4.5).radius(2.25).fill(t.accent)),
        TodoStatus::Completed => mark.border(1.5, t.running.alpha(0.7)).fill(t.running.alpha(0.7)),
        _ => mark.border(1.5, t.wash(22.0)),
    };
    let mut words = text(item.content.clone()).font(sans(12.0, weight, 1.4)).color(ink).wrap().grow(1.0).shrink().min_w(0.0);
    if item.status == TodoStatus::Cancelled {
        words = words.strikes([(0, item.content.len(), None)]);
    }
    row().key(format!("progress-todo-{i}")).w_full().gap(7.0).items_start().child(mark).child(words)
}

fn todo_form(t: &Theme, d: &ProgressDetail) -> El {
    col()
        .w_full()
        .gap(12.0)
        .items_stretch()
        .child(head(t, "Todos", "The agent's todo list", None, d.summary.done, d.summary.total).key("progress-head"))
        .child(todo_list(t, d, 5.0))
        .child(
            text("This session hasn't written a markdown checklist, so there's no plan to show.")
                .key("progress-todo-note")
                .font(sans(12.0, 400.0, 1.4))
                .color(t.muted)
                .wrap(),
        )
}

/// `EmptyState compact`: the icon, the title, and where progress comes from.
fn empty(t: &Theme) -> El {
    let words = "Progress shows up when the agent writes a todo list, or a markdown checklist such as a plan in .weave/plans/.";
    let at = words.find(".weave").unwrap_or(0);
    col()
        .key("progress-empty")
        .w_full()
        .items_center()
        .gap(3.5)
        .px(10.5)
        .py(14.0)
        .child(icon(&LIST_CHECKS, 20.0).color(t.muted).mb(7.0))
        .child(text("Nothing to track yet").font(sans(12.25, 500.0, 1.4)).color(t.text))
        .child(
            rich(words, vec![Span::new(at, at + ".weave/plans/".len()).mono()])
                .font(sans(10.5, 400.0, 1.4))
                .color(t.muted)
                .wrap()
                .w_full()
                .align(Align::Center),
        )
}
