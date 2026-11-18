//! Ports `components/canvas/AgentTabView.vue`: Agent's view, a live picture of the agent's own browser tab. A picture, not
//! a page: it is watched, not clicked. The element the latest step acted on gets a ring, and the step a caption, for six
//! seconds. The browser canvas (component 14) hosts it; the fetching and the 700 ms clock are `fleet::agent_tab`.
//!
//! Fleet eases the ring's move over 200 ms; Rune has no transition for a position, so it jumps.
use crate::State;
use crate::fleet::agent_tab::{HEIGHT, WIDTH, placed, tag_shown};
use crate::theme::{Theme, sans};
use rune::{El, Shadow, SurfaceFit, col, el, text};

/// The empty stage's words (`.agent-tab__empty`): while the first picture is on its way, and once a load has failed.
pub fn empty_words(failed: bool) -> &'static str {
    if failed { "The agent's tab is closed." } else { "Loading the agent's tab…" }
}

/// The agent's tab `tab_id` of `session`, filling the canvas's page area.
pub fn agent_tab(s: &State, t: &Theme, session: &str, tab_id: &str) -> El {
    let view = &s.agent_view;
    let [stage_w, stage_h] = view.stage;
    let place = placed(view.stage);
    let mut stage = el()
        .key("agent-tab-view")
        .w_full()
        .h_full()
        .clip_rounded(0.0)
        .fill(t.mix(t.text, t.panel_bg, 0.06))
        .on_layout(|s: &mut State, [_, _, w, h]| s.agent_view.stage = [w, h]);
    let picture = view.shown.as_deref().and_then(|key| s.images.ready(key));
    match (picture, place) {
        (Some((image, _)), Some(p)) => {
            stage = stage.child(
                el().key("agent-tab-picture")
                    .role("img")
                    .label("What the agent's tab shows now")
                    .pin(Some(p.left), Some(0.0), None, None)
                    .w(WIDTH * p.scale)
                    .h(HEIGHT * p.scale)
                    .pixels_fit(image.pixels(), SurfaceFit::Fill),
            );
        }
        // Until a picture has loaded; one already up stays through later failures.
        (None, _) => {
            stage = stage.child(
                col()
                    .key("agent-tab-empty")
                    .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
                    .center()
                    .child(text(empty_words(view.failed)).font(sans(13.0, 400.0, 1.4)).color(t.muted)),
            );
        }
        _ => {}
    }
    let step =
        s.agent_step().filter(|st| st.tab_id.as_deref() == Some(tab_id) && s.agent_view.target.as_ref().is_some_and(|(x, _)| x == session));
    if let (Some(step), Some(p)) = (step, place) {
        if let Some(b) = &step.bounds {
            let [x, y, w, h] = p.ring(b);
            stage = stage.child(
                el().key("agent-tab-ring")
                    .pin(Some(x), Some(y), None, None)
                    .w(w)
                    .h(h)
                    .border(2.0, t.accent)
                    .border_layout()
                    .radius(6.0)
                    .box_shadow(Shadow { x: 0.0, y: 0.0, blur: 0.0, spread: 4.0, color: t.accent.alpha(0.22), inset: false }),
            );
        }
        let fill = if step.ok { t.agent_caption_bg() } else { t.agent_caption_failed_bg() };
        stage = stage.child(
            el().key("agent-tab-caption")
                .role("status")
                .label(step.summary.clone())
                .pin(Some(12.0), None, None, Some(p.bottom(stage_h)))
                .max_w((stage_w - 24.0).max(0.0))
                .px(10.0)
                .py(6.0)
                .radius(8.0)
                .fill(fill)
                .child(text(step.summary.clone()).font(sans(12.0, 400.0, 1.4)).color(t.agent_caption_ink()).wrap_anywhere()),
        );
    }
    if let (Some(p), true) = (place, tag_shown(stage_w)) {
        stage = stage.child(
            el().key("agent-tab-tag")
                .pin(None, None, Some(12.0), Some(p.bottom(stage_h)))
                .px(7.0)
                .py(3.0)
                .radius(6.0)
                .fill(t.panel_bg.alpha(0.85))
                .border(1.0, t.border)
                .border_layout()
                .child(text(format!("agent's tab · {WIDTH}×{HEIGHT}")).font(sans(11.0, 400.0, 1.4)).color(t.muted)),
        );
    }
    stage
}
