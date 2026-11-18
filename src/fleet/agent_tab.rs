//! Agent's view (`AgentTabView.vue`): a picture of the agent's own browser tab, asked of Fleet again every 700 ms while it
//! is watched, with a ring on the element the latest step acted on and a caption saying what it did, for six seconds.
//! The view is `view::agent_tab`; this is what it reads and the clock that keeps it fresh. The browser canvas (component
//! 14) hosts it with `State::watch_agent_tab`; until it does, a scene does.
use super::canvases::CanvasKind;
use super::images::Source;
use crate::State;
use crate::conv::image_sniff::encode_component;
use rune::Tx;
use std::time::Duration;

/// How often the picture is asked for again while it's watched (`FRAME_MS`).
pub const FRAME_MS: u64 = 700;
/// How long a step's ring and caption stay (`STEP_MS`).
pub const STEP_MS: i64 = 6000;
/// The agent's tab is this size (Fleet's headless browser window).
pub const WIDTH: f32 = 1280.0;
pub const HEIGHT: f32 = 800.0;
/// The ring stands this far outside the element's box.
pub const RING_PAD: f32 = 4.0;
/// The size tag is hidden when the canvas is this wide or narrower (`@container browser-canvas (max-width: 420px)`).
pub const TAG_HIDDEN_AT: f32 = 420.0;

/// Whether the "agent's tab · 1280×800" tag shows in a canvas this wide.
pub fn tag_shown(canvas_w: f32) -> bool {
    canvas_w > TAG_HIDDEN_AT
}

const PREFIX: &str = "agent-frame:";

/// Where the picture sits in the stage (`frame`): it keeps its 1280×800 shape and is pinned to the top, like a page; the
/// room a tall panel leaves is below it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub scale: f32,
    pub left: f32,
}

pub fn placed(stage: [f32; 2]) -> Option<Placed> {
    let [w, h] = stage;
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let scale = (w / WIDTH).min(h / HEIGHT);
    Some(Placed { scale, left: (w - WIDTH * scale) / 2.0 })
}

impl Placed {
    /// The caption and the tag sit on the picture's bottom edge, not the panel's (`onPicture`).
    pub fn bottom(&self, stage_h: f32) -> f32 {
        (stage_h - HEIGHT * self.scale).max(0.0) + 12.0
    }

    /// The ring around a step's box: [left, top, width, height] in the stage.
    pub fn ring(&self, b: &crate::conv::browser::StepBox) -> [f32; 4] {
        [
            self.left + b.x * self.scale - RING_PAD,
            b.y * self.scale - RING_PAD,
            b.width * self.scale + RING_PAD * 2.0,
            b.height * self.scale + RING_PAD * 2.0,
        ]
    }
}

/// Whether a step at `at` (ISO text) is still within its six seconds at `now` (unix ms). An unreadable time is never fresh.
pub fn fresh(at: Option<&str>, now: i64) -> bool {
    at.and_then(super::work::parse_iso_ms).is_some_and(|t| now - t < STEP_MS)
}

/// The picture's path on Fleet (`agentFrameUrl`); `nonce` makes each request a fresh one.
pub fn frame_path(session: &str, tab: &str, nonce: u64) -> String {
    format!("/api/sessions/{}/agent-browser/tabs/{}/frame?n={nonce}", encode_component(session), encode_component(tab))
}

/// What the image cache keeps a frame under.
pub fn frame_key(session: &str, tab: &str, nonce: u64) -> String {
    format!("{PREFIX}{session}:{tab}:{nonce}")
}

#[derive(Default)]
pub struct AgentView {
    /// The session and tab being watched; none when no browser canvas shows one.
    pub target: Option<(String, String)>,
    nonce: u64,
    /// The last picture that loaded stays up while the next one comes (no flicker).
    pub shown: Option<String>,
    /// A load failed, and no picture has been shown yet: the tab is closed.
    pub failed: bool,
    last_tick: Option<f64>,
    /// The clock the ring and the caption end by, moved on every tick whether or not the view is active.
    pub now: i64,
    /// The stage as the view last measured it.
    pub stage: [f32; 2],
}

impl AgentView {
    pub fn nonce(&self) -> u64 {
        self.nonce
    }

    fn current(&self) -> Option<String> {
        self.target.as_ref().map(|(s, t)| frame_key(s, t, self.nonce))
    }
}

impl State {
    /// Starts watching a tab: its first picture is asked for at once, not after the first 700 ms. The same tab again keeps
    /// what it has.
    pub fn watch_agent_tab(&mut self, session: &str, tab: &str) {
        if self.agent_view.target.as_ref().is_some_and(|(s, t)| s == session && t == tab) {
            return;
        }
        self.stop_watching_agent_tab();
        self.agent_view.target = Some((session.to_string(), tab.to_string()));
        self.agent_view.now = crate::view::message::now_ms(self);
        self.agent_view.last_tick = None;
        self.want_agent_frame();
    }

    pub fn stop_watching_agent_tab(&mut self) {
        let view = &mut self.agent_view;
        *view = AgentView { stage: view.stage, ..AgentView::default() };
        self.images.forget_where(|k| k.starts_with(PREFIX));
    }

    fn want_agent_frame(&self) {
        if let (Some((session, tab)), Some(key)) = (&self.agent_view.target, self.agent_view.current()) {
            self.images.want(&key, Source::Path(frame_path(session, tab, self.agent_view.nonce)));
        }
    }

    /// The newest step on the watched tab, while it is fresh (it has the ring and the caption).
    pub fn agent_step(&self) -> Option<&crate::conv::browser::BrowserStep> {
        let (session, tab) = self.agent_view.target.as_ref()?;
        let step = self.browser.last_step_on(session, tab)?;
        fresh(step.at.as_deref(), self.agent_view.now).then_some(step)
    }

    /// The panel shows the browser canvas, so the picture should be kept fresh (`active`).
    fn agent_view_active(&self) -> bool {
        self.panel_body_shown(CanvasKind::Browser) && self.quiet.visible
    }

    /// Settles the frame that has come, and ticks every 700 ms: the clock moves, and a new picture is asked for only while
    /// the canvas is on screen and the window is visible. True when the view must redraw.
    pub fn keep_agent_view(&mut self, now: f64, tx: &mut Tx) -> bool {
        let Some(key) = self.agent_view.current() else { return false };
        // The canvas is gone with its session.
        if self.agent_view.target.as_ref().is_some_and(|(session, _)| *session != self.canvas_session()) {
            self.stop_watching_agent_tab();
            return true;
        }
        let before = (self.agent_view.shown.clone(), self.agent_view.failed, self.agent_step().map(|s| s.seq));
        match self.images.slot(&key) {
            Some(super::images::Slot::Ready(_)) => {
                self.agent_view.shown = Some(key.clone());
                self.agent_view.failed = false;
            }
            Some(super::images::Slot::Missing) => self.agent_view.failed = true,
            _ => {}
        }
        let due = self.agent_view.last_tick.is_none_or(|at| now - at >= FRAME_MS as f64 / 1000.0);
        if due {
            self.agent_view.last_tick = Some(now);
            self.agent_view.now = crate::view::message::now_ms(self);
            // One at a time: a slow Fleet isn't asked for a pile of pictures.
            let waiting = matches!(self.images.slot(&key), Some(super::images::Slot::Loading(_)));
            if self.agent_view_active() && !waiting {
                self.agent_view.nonce += 1;
                self.want_agent_frame();
            }
        }
        tx.wake_after(Duration::from_millis(FRAME_MS));
        // Pictures other than the one up and the one coming are let go.
        let (shown, current) = (self.agent_view.shown.clone(), self.agent_view.current());
        self.images.forget_where(|k| k.starts_with(PREFIX) && Some(k) != shown.as_deref() && Some(k.to_string()) != current);
        before != (self.agent_view.shown.clone(), self.agent_view.failed, self.agent_step().map(|s| s.seq))
    }
}
