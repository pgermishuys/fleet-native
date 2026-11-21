//! The automation composer (`AutomationComposer.vue` in `ComposerFrame.vue`; rows 7.2-7.6, 7.8, 7.9, 8.1, 9.1's
//! strip): the message box with the schedule words highlighted, the toolbar's Send (a new one) or Save (a saved one, once
//! something changed), the strip under it with the When chip and its menu
//! (`when.rs`) and where it runs (`strip.rs`), and the plan line saying what Enter will make, with the "Just once" ask as a
//! link. The machine, agent and model, and workflow chips are pieces L to N.
use crate::fleet::automations::schedule::When;
use crate::fleet::automations::words::{Part, Plan};
use crate::icons::{ARROW_UP, LOADER_CIRCLE};
use crate::theme::{Theme, sans};
use crate::ui::text_menu::text_input;
use crate::{State, Tx};
use rune::{Color, Cursor, El, Link, Shadow, Span, TextHighlight, auto_grow, col, icon, rich, row, spacer, text};

/// The box's key (`automation-message`).
pub const MESSAGE: &str = "automation-message";

const PLACEHOLDER: &str = "What should it do, and when? e.g. “Every Monday at 9, summarise the open PRs”";

/// The parts are 760 wide at most, centred.
const COLUMN: f32 = 760.0;

/// The plan line's ask link (`automation-plan-ask`); Rune draws a link as its own element under its id.
pub const PLAN_ASK: &str = "automation-plan-ask";

/// What the view reads off the composer, worked out once a frame.
struct Shown {
    highlight: Option<std::ops::Range<usize>>,
    when: Option<When>,
    parsed: bool,
    plan: Plan,
    show_send: bool,
    can_submit: bool,
    /// The message has schedule words, whether or not they decide.
    has_words: bool,
}

/// `.automation-composer`: padding 4/24/18, its parts 760 wide at most and centred.
pub fn section(s: &State, t: &Theme) -> El {
    let editing = s.composer_editing();
    let busy = s.automations_nav.submitting;
    let shown = s.with_composer(editing, |c, ctx| Shown {
        highlight: c.highlight(),
        when: c.when(),
        parsed: c.active_hit().is_some(),
        plan: c.plan(ctx),
        show_send: c.show_send(ctx),
        can_submit: c.can_submit(ctx, busy),
        has_words: c.hit().is_some(),
    });
    col()
        .key("automation-composer")
        .label(if editing { "Automation" } else { "New automation" })
        .w_full()
        .items_center()
        .pt(4.0)
        .px(24.0)
        .pb(18.0)
        .child(frame(s, t, editing, busy, &shown))
        .child(strip(s, t, &shown, busy))
        .child(plan_line(t, &shown.plan))
}

/// `ComposerFrame`: the raised box with the field over the toolbar.
fn frame(s: &State, t: &Theme, editing: bool, busy: bool, shown: &Shown) -> El {
    let nav = &s.automations_nav;
    let edit = if editing { &nav.edit_box } else { &nav.draft_box };
    let mut field = text_input(edit, sans(14.0, 400.0, 1.5))
        .key(MESSAGE)
        .label("What the automation does, and when")
        .placeholder(PLACEHOLDER, t.muted)
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .w_full()
        .pt(14.0)
        .px(16.0)
        .pb(6.0)
        .min_h(70.0)
        .disabled(busy);
    if !busy {
        field = if editing {
            field.on_edit(|s: &mut State| &mut s.automations_nav.edit_box)
        } else {
            field.on_edit(|s: &mut State| &mut s.automations_nav.draft_box)
        }
        .on_input_change(move |s: &mut State, _: &mut Tx| s.automations_nav.box_changed(editing))
        // Enter creates or saves; Shift+Enter is a new line.
        .on_submit(|s: &mut State, tx: &mut Tx| s.automation_submit(tx));
    }
    // The mirror's `<mark>`: the accent at 16 % behind the schedule words, while they decide.
    if let Some(range) = shown.highlight.clone() {
        field = field.text_highlights(vec![TextHighlight::new(range, t.accent.alpha(0.16)).pad(1.0)]);
    }
    let mut toolbar = row().key("automation-toolbar").w_full().center_y().gap(2.0).pt(4.0).px(8.0).pb(8.0).child(spacer());
    // Nothing to save yet: the toolbar is its padding alone, as Fleet's empty slot is.
    if shown.show_send {
        toolbar = toolbar.child(send_button(t, editing, busy, shown.can_submit));
    }
    col()
        .key("automation-frame")
        .w_full()
        .max_w(COLUMN)
        .radius(14.0)
        .fill(t.card_bg)
        .border(1.0, t.border)
        .box_shadow(Shadow { x: 0.0, y: 10.0, blur: 28.0, spread: -18.0, color: Color::BLACK.alpha(0.5), inset: false })
        .focus_within_ring(1.0, -1.0, t.accent_border(55.0))
        .child(auto_grow(field, 2, 8))
        .child(toolbar)
}

/// `.composer-frame__send`: a new automation's round arrow, a saved one's **Save** pill; a spinner while it saves; faded and
/// still unless it can be sent.
fn send_button(t: &Theme, editing: bool, busy: bool, can: bool) -> El {
    let mut b = row()
        .key("automation-submit")
        .role("button")
        .h(30.0)
        .center()
        .radius(999.0)
        .fill(t.accent)
        .opacity(if can || busy { 1.0 } else { 0.5 })
        .help(if editing { "Save changes (Enter)" } else { "Create automation (Enter)" })
        .label(if editing { "Save changes" } else { "Create automation" });
    b = if editing { b.px(14.0) } else { b.w(30.0) };
    b = if busy {
        b.child(icon(&LOADER_CIRCLE, 14.0).color(t.primary_fg).cycle(crate::view::ring_popover::spinner()))
    } else if editing {
        b.child(text("Save").font(sans(12.5, 500.0, 1.2)).color(t.primary_fg))
    } else {
        b.child(icon(&ARROW_UP, 14.0).color(t.primary_fg))
    };
    if can {
        b.hover(Color::BLACK.alpha(0.1))
            .cursor(Cursor::Pointer)
            .focusable(true)
            .focus_ring(2.0, 2.0, t.accent)
            .on_press_tx(|s: &mut State, tx: &mut Tx| s.automation_submit(tx))
    } else {
        b.disabled(true)
    }
}

/// `.automation-composer__strip`: the chips under the box. The When chip and its menu are `when.rs`; where it runs is
/// `strip.rs`; the machine, the agent and model, and a workflow target are L to N.
fn strip(s: &State, t: &Theme, shown: &Shown, busy: bool) -> El {
    let when = super::when::Shown { when: shown.when.clone(), parsed: shown.parsed, message_has_schedule: shown.has_words };
    row()
        .key("automation-strip")
        .w_full()
        .max_w(COLUMN)
        .flex_wrap()
        .center_y()
        .gap(2.0)
        .pt(6.0)
        .px(4.0)
        .child(super::when::chip(s, t, &when, busy))
        .children(super::strip::chips(s, t, busy))
}

/// `.automation-composer__plan`: one line of words (12 px muted, line-height 1.55) with the schedule in the text colour,
/// warnings in the waiting colour, paths in mono, and the ask's answer underlined as a link (row 7.9).
fn plan_line(t: &Theme, plan: &Plan) -> El {
    let mut content = String::new();
    let mut spans = Vec::new();
    let push = |content: &mut String, spans: &mut Vec<Span>, part: &Part| {
        let at = content.len();
        content.push_str(&part.text);
        let end = content.len();
        if part.strong {
            spans.push(Span::new(at, end).color(t.text).weight(500.0));
        } else if part.code {
            spans.push(Span::new(at, end).color(t.text).mono().size(11.5));
        } else if part.warn {
            spans.push(Span::new(at, end).color(t.status_waiting_text));
        }
    };
    for part in &plan.schedule {
        push(&mut content, &mut spans, part);
    }
    let mut link = None;
    if let Some(ask) = &plan.ask {
        content.push(' ');
        if !ask.question.is_empty() {
            push(&mut content, &mut spans, &Part { text: ask.question.clone(), warn: true, ..Part::default() });
            content.push(' ');
        }
        let at = content.len();
        content.push_str(&ask.answer);
        let range = at..content.len();
        spans.push(Span::new(range.start, range.end).color(t.text).underline(t.muted.alpha(0.55)));
        link = Some(Link::new(range, PLAN_ASK.to_string()));
    }
    if !content.is_empty() && !plan.rest.is_empty() {
        content.push(' ');
    }
    for part in &plan.rest {
        push(&mut content, &mut spans, part);
    }
    let mut line = rich(content, spans).key("automation-plan").font(sans(12.0, 400.0, 1.55)).color(t.muted).wrap().w_full();
    if let Some(link) = link {
        line = line.link(link).on_link(|s: &mut State, _: &str, _: &mut Tx| s.automation_plan_answer());
    }
    row().w_full().max_w(COLUMN).min_h(22.0).pt(4.0).px(11.0).child(line)
}
