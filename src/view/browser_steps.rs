//! The steps a call took in the agent's own browser, under the call. Ports `components/session/BrowserSteps.vue`: one
//! line per step (what happened, then the operation and its time), a failed one with its reason, a screenshot step
//! with its picture. What the steps are is `conv::browser`; this file only draws.
//!
//! Fleet's phone layout (640px and under) is not drawn: this is a desktop app.
use super::conversation::Part;
use super::image;
use crate::State;
use crate::conv::browser::{BrowserStep, heading};
use crate::conv::image_sniff::{Shot, encode_component};
use crate::conv::tools::{ToolStatus, get_tool};
use crate::icons::{CHECK, CIRCLE_ALERT, GLOBE};
use crate::theme::{RADIUS_BTN, Theme, mono, sans};
use rune::{El, col, el, icon, row, text};

/// The session whose steps these are: the open one. A scene has no server, so its session carries an invented id.
fn session_of(s: &State) -> String {
    s.open_id().unwrap_or_default()
}

/// What a message's steps are doing, for its row's memo: a step arriving, or its picture arriving or being lost,
/// redraws it.
pub fn key(s: &State, parts: &[Part]) -> usize {
    let session = session_of(s);
    let mut n = 0;
    for part in parts {
        let Part::Tool { tool, call_id, .. } = part else { continue };
        if !get_tool(tool).uses_browser {
            continue;
        }
        for step in s.browser.of(&session, call_id) {
            n = n * 5 + 1;
            if let Some((id, ..)) = &step.screenshot {
                n += usize::from(s.images.state_of(&shot_path(&session, id)));
            }
        }
    }
    n
}

fn shot_path(session: &str, id: &str) -> String {
    format!("/api/sessions/{}/screenshots/{}", encode_component(session), encode_component(id))
}

fn step_line(t: &Theme, s: &State, session: &str, call: &str, step: &BrowserStep) -> El {
    let (glyph, ink) = if step.ok { (&CHECK, t.complete) } else { (&CIRCLE_ALERT, t.error) };
    // `.browser-steps__icon`: 16px wide, as tall as the summary's first line.
    let mark = el().w(16.0).h(18.0).center().child(icon(glyph, 12.0).color(ink));
    let mut summary = col().grow(1.0).shrink().min_w(0.0).child(
        text(step.summary.clone())
            .key(format!("step-{call}-{}-summary", step.seq))
            .font(sans(12.5, 400.0, 18.0 / 12.5))
            .color(t.text)
            .wrap(),
    );
    if let (false, Some(error)) = (step.ok, step.error.as_deref().filter(|e| !e.is_empty())) {
        summary = summary.child(text(error.to_string()).font(sans(12.0, 400.0, 1.45)).color(t.muted).wrap());
    }
    let detail = text(step.detail.clone()).font(mono(11.0, 400.0, 18.0 / 11.0)).color(t.muted);
    let line = row().w_full().gap(8.0).child(mark).child(summary).child(detail);
    let mut item = col().key(format!("step-{call}-{}", step.seq)).w_full().px(6.0).py(4.0).radius(6.0).child(line);
    if let Some((id, width, height)) = &step.screenshot {
        // Spans the summary and detail columns: in from the icon column and its gap, 2px above and below.
        let shot = Shot { path: shot_path(session, id), width: *width, height: *height };
        let title = step.title.as_deref().filter(|t| !t.is_empty()).unwrap_or("Agent's tab");
        item = item.child(image::screenshot_padded(t, s, &format!("{call}-{}", step.seq), &shot, title, (2.0, 0.0, 2.0, 24.0)));
    }
    item
}

/// The card under a browser call, or nothing when the call took no steps.
pub fn steps(t: &Theme, s: &State, call_id: &str, status: ToolStatus) -> El {
    let session = session_of(s);
    let steps = s.browser.of(&session, call_id);
    if steps.is_empty() {
        return el();
    }
    let head = row()
        .key(format!("steps-head-{call_id}"))
        .w_full()
        .center_y()
        .gap(6.0)
        .px(10.0)
        .py(6.0)
        .child(icon(&GLOBE, 12.0).color(t.muted))
        .child(text(heading(status == ToolStatus::Running, steps.len())).font(sans(12.0, 400.0, 1.3)).color(t.muted));
    let list = col().w_full().px(4.0).py(4.0).gap(1.0).children(steps.iter().map(|step| step_line(t, s, &session, call_id, step)));
    let card = col()
        .key(format!("steps-{call_id}"))
        .w_full()
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.card_bg)
        .child(head)
        .child(el().w_full().h(1.0).fill(t.border))
        .child(list);
    // `margin: 0 8px 8px 31px`: lined up with the call's label.
    col().w_full().pr(8.0).pb(8.0).pl(31.0).child(card)
}
