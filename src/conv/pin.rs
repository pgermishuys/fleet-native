//! When the conversation follows its end, when it offers "Jump to latest", and when it asks for older messages: the
//! decisions of weave-fleet's `composables/use-stream-scroll.ts`, as plain logic with no Rune in it. The view feeds it
//! where the scroll box is (`Edges`) each time it moves and does what it says.
//!
//! Not ported, on purpose: the incremental mount (`use-incremental-mount.ts`) and the "laid out" class that
//! `showMessage` adds, both of which exist because a browser lays out every message. `virtual_rows` builds only what is
//! on screen and Rune reveals a row it hasn't built, so neither has a job here.

/// Within this far of the bottom (points) still counts as at the bottom, for the pill and for following.
pub const SCROLL_BOTTOM_THRESHOLD: f32 = 80.0;
/// Scrolled to within this far of the top, the next page of older messages is asked for.
pub const SCROLL_TOP_THRESHOLD: f32 = 100.0;
/// Exactly at the bottom: the offset is fractional at some zoom levels.
pub const AT_BOTTOM_EPSILON: f32 = 2.0;
/// How long after wheel, touch or key input a move up is taken as the user's (milliseconds).
pub const USER_INTENT_MS: f64 = 700.0;
/// How long a glide to the end may take before being far from the end counts as leaving it again (milliseconds).
pub const GLIDE_MS: f64 = 1500.0;
/// How long a jumped-to message keeps its ring (milliseconds).
pub const SHOWN_MS: f64 = 1600.0;

/// Where the scroll box is: its offset from the top, how tall its content is and how tall its window is.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Edges {
    pub top: f32,
    pub height: f32,
    pub viewport: f32,
}

impl Edges {
    /// How far the window is above the end of the content.
    pub fn from_bottom(&self) -> f32 {
        self.height - self.top - self.viewport
    }
}

/// What `updatePinnedState` asks the caller to do.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Update {
    /// Scrolled to the top: older messages are due (the caller checks `hasMore` and `isLoadingOlder`).
    pub load_older: bool,
    /// The content grew under a view that follows the end, and left it short of the end: put the box back there.
    pub follow: bool,
}

/// `keepPinnedToBottom` and its neighbours.
#[derive(Clone, Debug, PartialEq)]
pub struct Pin {
    keep: bool,
    last_top: f32,
    /// The content's and the window's height at the last update, to tell a box that grew from one that was scrolled.
    last_size: [f32; 2],
    intent_until: f64,
    /// Our own glide to the end is on its way: until then, being far from the end is not leaving it.
    glide_until: f64,
}

impl Default for Pin {
    fn default() -> Self {
        Pin { keep: true, last_top: 0.0, last_size: [0.0; 2], intent_until: f64::NEG_INFINITY, glide_until: f64::NEG_INFINITY }
    }
}

impl Pin {
    /// Whether the view follows the end (`keepPinnedToBottom`).
    pub fn pinned(&self) -> bool {
        self.keep
    }

    /// Whether the "Jump to latest" pill shows (`showJumpToLatest`).
    pub fn show_jump(&self) -> bool {
        !self.keep
    }

    /// Wheel up, touch, a key that moves up, or a press on the scroll box (`noteUserIntent`).
    pub fn note_user_intent(&mut self, now_ms: f64) {
        self.intent_until = now_ms + USER_INTENT_MS;
    }

    /// The box moved or its content changed (`updatePinnedState`).
    pub fn update(&mut self, at: Edges, now_ms: f64) -> Update {
        // Only the user's own input turns a small move up into leaving the bottom. Our scrolls, layout shifts and
        // rounding move the offset too, and must not.
        let load_older = at.top <= SCROLL_TOP_THRESHOLD;
        // Content that grows, or a window that resizes, moves the end away without a scroll: Fleet's observers put a
        // following view back at the end before any scroll event is read, so it isn't taken as leaving.
        let size = [at.height, at.viewport];
        let resized = size != std::mem::replace(&mut self.last_size, size);
        if resized && (at.top - self.last_top).abs() <= 1.0 {
            return Update { load_older, follow: self.keep && now_ms >= self.glide_until && at.from_bottom() > AT_BOTTOM_EPSILON };
        }
        let moved_up = at.top < self.last_top - 1.0;
        self.last_top = at.top;
        if at.from_bottom() <= AT_BOTTOM_EPSILON {
            self.keep = true;
        } else if (at.from_bottom() > SCROLL_BOTTOM_THRESHOLD && now_ms >= self.glide_until) || (moved_up && now_ms < self.intent_until) {
            self.keep = false;
        }
        Update { load_older, follow: false }
    }

    /// A prompt was sent, or another session opened: follow the end again (`pinToBottom`, the session watcher).
    pub fn pin_to_bottom(&mut self) {
        self.keep = true;
    }

    /// `pin_to_bottom`, and Rune is gliding the box there: the offset passes through places that are far from the end,
    /// and Fleet's instant scroll never does.
    pub fn follow(&mut self, now_ms: f64) {
        self.keep = true;
        self.glide_until = now_ms + GLIDE_MS;
        self.intent_until = f64::NEG_INFINITY;
    }

    /// The view was put at the end (`scrollToBottom`).
    pub fn scrolled_to_bottom(&mut self, at: Edges) {
        self.last_top = at.top;
        self.last_size = [at.height, at.viewport];
        self.keep = true;
    }

    /// A message was jumped to: stop following the end (`showMessage`).
    pub fn show_message(&mut self) {
        self.keep = false;
    }
}

/// How long the ring on a jumped-to message shows, and what is in its way: the id and the moment it ends.
#[derive(Clone, Debug, PartialEq)]
pub struct Shown {
    pub id: String,
    pub until_ms: f64,
}

impl Shown {
    pub fn new(id: &str, now_ms: f64) -> Shown {
        Shown { id: id.to_string(), until_ms: now_ms + SHOWN_MS }
    }

    pub fn active(&self, now_ms: f64) -> bool {
        now_ms < self.until_ms
    }
}
