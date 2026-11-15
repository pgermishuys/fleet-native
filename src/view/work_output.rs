//! What opens under a background row: the output (`components/session/WorkOutput.vue`), the Details of a sub-agent with
//! no session of its own and the line that says why a Stop was refused (`BackgroundWorkRow.vue`, rows 4.22, 4.27, 4.28).
//! What is read and kept is `conv::work_output`; the requests are `fleet::work_actions`.
use crate::State;
use crate::conv::work_output::Reader;
use crate::fleet::work::{RunningWorkItem, is_running};
use crate::theme::{Theme, mono, sans};
use rune::{El, ScrollSpec, col, row, text};

/// The row's own indent: under its icon (`margin: .. 4px .. 30px`).
const INDENT: f32 = 30.0;

fn name(item: &RunningWorkItem) -> &str {
    item.label.as_deref().unwrap_or(&item.title)
}

/// `.work-row__error`: a refused Stop, in red under the row.
pub fn stop_error(t: &Theme, item: &RunningWorkItem, why: &str) -> El {
    text(why)
        .key(format!("work-stop-error-{}", item.id))
        .role("alert")
        .font(sans(12.0, 400.0, 1.3))
        .color(t.error_text)
        .wrap()
        .ml(INDENT)
        .mr(4.0)
        .mb(4.0)
}

/// `.work-output`: a note while there is nothing to show, then the text in a box that follows its end unless it was
/// scrolled up.
pub fn output(s: &State, t: &Theme, item: &RunningWorkItem) -> Option<El> {
    let reader: &Reader = s.work.outputs.get(&item.id)?;
    let note = |key: &str, words: &str, alert: bool| {
        let n = text(words.to_string())
            .key(format!("work-output-{key}-{}", item.id))
            .font(sans(12.0, 400.0, 1.3))
            .color(if alert { t.error_text } else { t.muted })
            .wrap()
            .px(8.0)
            .py(4.0);
        if alert { n.role("alert") } else { n }
    };
    let mut panel = col().key(format!("work-output-panel-{}", item.id)).ml(INDENT).mr(4.0).mt(2.0).mb(4.0).min_w(0.0);
    if let Some(why) = &reader.error {
        panel = panel.child(note("error", why, true));
    } else if reader.loading && reader.text.is_empty() {
        panel = panel.child(note("loading", "Reading the output\u{2026}", false));
    } else if reader.text.is_empty() {
        panel = panel.child(note("empty", "No output yet.", false));
    }
    if !reader.text.is_empty() {
        let font = mono(11.5, 400.0, 1.45);
        let mut lines = col().w_full().px(8.0).py(6.0);
        if reader.clipped {
            lines = lines.child(
                text("\u{2026} earlier output not shown")
                    .key(format!("work-output-clipped-{}", item.id))
                    .font(font)
                    .color(t.muted)
                    .w_full(),
            );
        }
        lines = lines.child(
            text(reader.text.clone())
                .key(format!("work-output-text-{}", item.id))
                .font(font)
                .color(t.text)
                .wrap()
                .w_full()
                .selectable(true),
        );
        panel = panel.child(
            col()
                .key(format!("work-output-box-{}", item.id))
                .label(format!("Output of {}", name(item)))
                .w_full()
                .max_h(220.0)
                .scroll(ScrollSpec { stick_bottom: true })
                .radius(6.0)
                .border(1.0, t.border)
                .border_layout()
                .fill(t.panel_bg)
                .child(lines),
        );
    }
    Some(panel)
}

/// `.work-row__details`: what a sub-agent without a session was asked and how it is doing.
pub fn details(t: &Theme, item: &RunningWorkItem) -> El {
    let running = is_running(item);
    let font = sans(12.0, 400.0, 1.3);
    let line = |key: &str, label: &str, value: &str| {
        row().w_full().gap(12.0).child(text(label.to_string()).font(font).color(t.muted).w(50.0)).child(
            text(value.to_string()).key(format!("work-details-{key}-{}", item.id)).font(font).color(t.text).wrap().grow(1.0).min_w(0.0),
        )
    };
    let mut panel = col()
        .key(format!("work-details-{}", item.id))
        .ml(INDENT)
        .mr(4.0)
        .mt(2.0)
        .mb(6.0)
        .px(10.0)
        .py(8.0)
        .gap(2.0)
        .radius(7.0)
        .border(1.0, t.border)
        .border_layout();
    if let Some(label) = &item.label {
        panel = panel.child(line("asked", "Asked to", label));
    }
    if let Some(detail) = &item.detail {
        panel = panel.child(line("now", if running { "Now" } else { "Ended" }, detail));
    }
    if running && !item.can_stop {
        panel = panel.child(
            text("Runs inside the tool call. Interrupt the turn to stop it.")
                .key(format!("work-details-note-{}", item.id))
                .font(font)
                .color(t.muted)
                .wrap()
                .mt(2.0),
        );
    }
    panel
}
