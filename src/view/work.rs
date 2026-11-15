//! The background strip above the composer and its rows. Ports `components/session/BackgroundStrip.vue` and
//! `BackgroundWorkRow.vue` (docs/parity/03-conversation-parts.md, piece K). `conv::work_clock` says what shows and when
//! it changes. What the buttons do (Output, Open, Details, Events, Stop) is `fleet::work_actions`; what opens under a row
//! is `view::work_output`.
use super::conversation::COLUMN;
use super::message::now_ms;
use super::work_output;
use crate::State;
use crate::conv::time::local_offset_secs;
use crate::conv::work_clock::{time_text, time_title, what_text};
use crate::fleet::work::{EndedReason, RunningWorkItem, WorkKind, WorkStatus, is_running, work_failed, work_result};
use crate::icons::*;
use crate::theme::{RADIUS_CARD, Theme, mono, sans};
use rune::{Cursor, El, Icon, col, el, icon, row, text};

/// The strip is as narrow as a phone below this width: the kind label and the word "Stop" go (`@container (max-width: 440px)`).
const NARROW: f32 = 440.0;

impl State {
    /// The open session's work, as the reducer holds it.
    pub fn running_work(&self) -> &[RunningWorkItem] {
        self.stream.as_ref().map_or(&[], |s| s.running_work.as_slice())
    }

    /// Notes what ended, and wakes the strip when a time on it changes or a finished row is due to drop (the shared clock,
    /// row 4.7: it rests while nothing shows). True when the strip must redraw.
    pub fn keep_work_clock(&mut self, tx: &mut rune::Tx) -> bool {
        self.poll_work_outputs(tx);
        let Some(stream) = &self.stream else { return false };
        let now = now_ms(self);
        self.work.observe(&stream.running_work, now);
        if let Some(next) = self.work.next_change_ms(&stream.running_work, now) {
            tx.wake_after(std::time::Duration::from_millis(next.max(50) as u64));
        }
        self.work.changed(&stream.running_work, now)
    }
}

/// `BackgroundStrip` (rows 4.1-4.5): nothing while nothing runs or just finished.
pub fn strip(s: &State, t: &Theme) -> Option<El> {
    let (items, now) = (s.running_work(), now_ms(s));
    let visible = s.work.visible(items, now);
    if visible.is_empty() {
        return None;
    }
    let collapsed = s.work.collapsed;
    let tip = if collapsed { "Show what runs in the background" } else { "Collapse to one line" };
    let header = row()
        .key("background-strip-toggle")
        .w_full()
        .gap(8.0)
        .center_y()
        .pl(12.0)
        .pr(8.0)
        .py(6.0)
        .cursor(Cursor::Pointer)
        .help(tip)
        .label(tip)
        .on_press(|s: &mut State| s.work.collapsed = !s.work.collapsed)
        .child(text("BACKGROUND").font(sans(11.0, 600.0, 1.3).tracking(0.44)).color(t.muted))
        .child(
            text(s.work.summary(items, now))
                .key("background-strip-summary")
                .font(sans(12.0, 400.0, 1.3))
                .color(t.text)
                .truncate()
                .grow(1.0),
        )
        .child(icon(&CHEVRON_DOWN, 14.0).color(t.muted).rotate(if collapsed { -std::f32::consts::FRAC_PI_2 } else { 0.0 }));
    let mut card = col()
        .key("background-strip")
        .label("Running in the background")
        .w_full()
        .max_w(COLUMN)
        .mb(6.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.card_bg)
        .on_layout(|s: &mut State, [_, _, w, _]| s.work.width = w)
        .child(header);
    if !collapsed {
        let narrow = s.work.width > 0.0 && s.work.width <= NARROW;
        let mut list = col().gap(2.0).px(4.0).pb(4.0);
        for item in visible {
            list = list.child(work_row(s, t, item, now, narrow));
        }
        card = card.child(list);
    }
    Some(card)
}

/// One row (`BackgroundWorkRow`, rows 4.8-4.16, 4.21, 4.25): also what the status bar's list will draw.
pub fn work_row(s: &State, t: &Theme, item: &RunningWorkItem, now: i64, narrow: bool) -> El {
    let running = is_running(item);
    let failed = work_failed(item);
    let what = what_text(item);
    let (glyph, tone): (&'static Icon, _) = if running {
        let glyph = match item.kind {
            WorkKind::Shell => &TERMINAL,
            WorkKind::Subagent => &BOT,
            WorkKind::Monitor => &ACTIVITY,
            WorkKind::Task => &LIST_TODO,
        };
        (glyph, t.muted)
    } else if failed && item.ended_reason != Some(EndedReason::Lost) {
        (&X, t.error)
    } else if item.ended_reason == Some(EndedReason::Lost) {
        (&CIRCLE_ALERT, t.idle)
    } else if item.status == WorkStatus::Cancelled {
        (&BAN, t.muted)
    } else {
        (&CHECK, t.running)
    };
    let ink = if running { t.text } else { t.muted };
    let font = if item.kind == WorkKind::Shell { mono(11.5, 400.0, 1.3) } else { sans(12.0, 400.0, 1.3) };
    let mut line = row()
        .key(format!("work-row-{}", item.id))
        .w_full()
        .min_h(28.0)
        .gap(8.0)
        .center_y()
        .pl(8.0)
        .pr(4.0)
        .radius(7.0)
        .hover(t.wash(5.0))
        .child(icon(glyph, 14.0).color(tone));
    if !narrow {
        line = line
            .child(el().w(66.0).child(text(item.kind.label().to_uppercase()).font(sans(11.0, 600.0, 1.3).tracking(0.44)).color(t.muted)));
    }
    let mut said = row().grow(1.0).shrink().min_w(0.0).help(what.clone());
    said = said.child(text(what).key(format!("work-what-{}", item.id)).font(font).color(ink).truncate().shrink().min_w(0.0));
    // " · x" in muted: its model and a chain's step are for the Pi and sub-agent rows (the model needs the child's session).
    let meta = if running { item.detail.clone() } else { work_result(item) };
    if let Some(meta) = meta {
        said =
            said.child(text(format!(" \u{b7} {meta}")).key(format!("work-meta-{}", item.id)).font(sans(12.0, 400.0, 1.3)).color(t.muted));
    }
    line = line.child(said).child(
        text(time_text(item, now))
            .key(format!("work-time-{}", item.id))
            .font(sans(12.0, 400.0, 1.3).features("'tnum' 1"))
            .color(t.muted)
            .help(time_title(item, now, local_offset_secs())),
    );
    let id = item.id.clone();
    let has_details = item.kind == WorkKind::Subagent && item.child_session_id.is_none();
    let output_open = item.can_read_output && s.work.outputs.contains_key(&item.id);
    let details_open = has_details && s.work.details.contains(&item.id);
    if item.can_read_output {
        let id = id.clone();
        let tip = if running { "Show the output so far" } else { "Show the output" };
        line = line.child(
            action(t, item, "output", (&SCROLL_TEXT, 0.0), "Output", narrow, tip).on_press(move |s: &mut State| s.toggle_work_output(&id)),
        );
    }
    if let Some(child) = &item.child_session_id {
        let (parent, child) = (item.session_id.clone(), child.clone());
        line = line.child(action(t, item, "open", (&ARROW_UP_RIGHT, 0.0), "Open", narrow, "Open its session").on_press_tx(
            move |s: &mut State, tx: &mut rune::Tx| {
                s.open_child(&parent, &child, tx);
            },
        ));
    }
    if has_details {
        let id = id.clone();
        let turn = if details_open { std::f32::consts::PI } else { 0.0 };
        line = line.child(
            action(t, item, "details", (&CHEVRON_DOWN, turn), "Details", narrow, "What it was asked, and how it is doing")
                .on_press(move |s: &mut State| s.toggle_work_details(&id)),
        );
    }
    if let (WorkKind::Monitor, Some(call)) = (item.kind, &item.tool_call_id) {
        let call = call.clone();
        line = line.child(
            action(t, item, "events", (&ARROW_DOWN, 0.0), "Events", narrow, "Jump to its events in the conversation")
                .on_press_tx(move |s: &mut State, tx: &mut rune::Tx| s.show_work_events(&call, tx)),
        );
    }
    // Stop, or an empty slot of its width so rows line up whether or not they offer it.
    let slot = if narrow { 28.0 } else { 58.0 };
    line = line.child(if running && item.can_stop { stop(s, t, item, narrow, slot) } else { el().w(slot).h(1.0) });
    let mut block = col().w_full().child(line);
    if let Some(why) = s.work.stop_errors.get(&item.id) {
        block = block.child(work_output::stop_error(t, item, why));
    }
    if details_open {
        block = block.child(work_output::details(t, item));
    }
    if output_open {
        block = block.children(work_output::output(s, t, item));
    }
    block
}

/// A ghost button of the row (`.work-row__action`): 24px tall, 12px muted text that turns to the text colour on hover.
fn action(t: &Theme, item: &RunningWorkItem, key: &str, glyph: (&'static Icon, f32), word: &'static str, narrow: bool, tip: &str) -> El {
    let mut b = row()
        .key(format!("work-{key}-{}", item.id))
        .h(24.0)
        .px(if narrow { 6.0 } else { 8.0 })
        .gap(4.0)
        .center_y()
        .radius(8.0)
        .hover(t.accent.alpha(0.10))
        .hover_group()
        .cursor(Cursor::Pointer)
        .help(tip)
        .label(word);
    b = b.child(icon(glyph.0, 14.0).color(t.muted).group_color(t.text).rotate(glyph.1));
    b.child(text(word).font(sans(12.0, 400.0, 1.3)).color(t.muted).group_color(t.text))
}

/// Stop (rows 4.21, 4.23): 58px wide with its word, 28px with only its icon in a narrow strip. Whether it is in flight is
/// `State::work.stopping`.
fn stop(s: &State, t: &Theme, item: &RunningWorkItem, narrow: bool, slot: f32) -> El {
    let stopping = s.work.stopping.contains(&item.id);
    let name = item.label.as_deref().unwrap_or(&item.title);
    let tip = if stopping { "Stopping\u{2026}" } else { "Stop" };
    let mut b = row()
        .key(format!("work-stop-{}", item.id))
        .w(slot)
        .h(24.0)
        .gap(4.0)
        .center()
        .radius(8.0)
        .help(tip)
        .label(format!("Stop {name}"))
        .child(icon(&CIRCLE_STOP, 14.0).color(t.muted).group_color(t.text));
    if !narrow {
        b = b.child(text("Stop").font(sans(12.0, 400.0, 1.3)).color(t.muted).group_color(t.text));
    }
    if stopping {
        return b.opacity(0.5).disabled(true);
    }
    let id = item.id.clone();
    b.hover(t.accent.alpha(0.10)).hover_group().cursor(Cursor::Pointer).on_press(move |s: &mut State| s.stop_work(&id))
}
