//! The right panel's width rules (`AppShell.vue`: `rightPanelWidth`, `inlineRoom`, `sessionsRightPanelWidth`,
//! `onGutterPointerDown`). Plain numbers in, plain numbers out, so the rules can be tested without a window.

/// The panel's width before anyone drags it.
pub const DEFAULT_W: f32 = 360.0;
pub const MIN_W: f32 = 280.0;
/// What Widen asks for (`WIDENED_RIGHT_PANEL_WIDTH`).
pub const WIDENED_W: f32 = 760.0;
/// The conversation never gets narrower than this beside an inline panel (`MIN_CONVERSATION_WIDTH`).
pub const MIN_CONVERSATION: f32 = 480.0;
/// Review gives the panel this share of the sheet (`REVIEW_PANEL_SHARE`).
pub const REVIEW_SHARE: f32 = 0.6;
/// How long the width eases when Widen is toggled (`right-panel--widening`).
pub const WIDEN_SECS: f32 = 0.2;
/// The collapsed rail's width.
pub const RAIL_W: f32 = 48.0;

#[derive(Debug, Clone, PartialEq)]
pub struct RightPanel {
    /// The width the user dragged to; not saved (`rightPanelWidth = ref(360)`).
    pub width: f32,
    /// The active tab asked for more room (`canvasesStore.widened`).
    pub widened: bool,
    /// Changes is open to be reviewed (`state.review && activeId === "changes"`); component 10 sets it.
    pub review: bool,
    /// The gutter was dragged once: the width is the user's own from then on (`userSizedPanel`).
    pub user_sized: bool,
}

impl Default for RightPanel {
    fn default() -> Self {
        Self { width: DEFAULT_W, widened: false, review: false, user_sized: false }
    }
}

/// Room for the panel beside a readable conversation; unlimited until the sheet is measured.
pub fn inline_room(sheet_w: f32) -> f32 {
    if sheet_w > 0.0 { (sheet_w - MIN_CONVERSATION).round() } else { f32::INFINITY }
}

/// The panel is a column only while the conversation keeps its room beside it at the panel's narrowest.
pub fn fits(sheet_w: f32) -> bool {
    inline_room(sheet_w) >= MIN_W
}

impl RightPanel {
    pub fn reviewing(&self) -> bool {
        self.review && !self.user_sized
    }

    /// The width on screen (`sessionsRightPanelWidth`).
    pub fn shown(&self, sheet_w: f32) -> f32 {
        let room = inline_room(sheet_w);
        if self.reviewing() && sheet_w > 0.0 {
            return room.min(self.width.max((sheet_w * REVIEW_SHARE).round()));
        }
        room.min(if self.widened { self.width.max(WIDENED_W.min(room)) } else { self.width })
    }

    /// The gutter was pressed: it takes over from Widen and from the review width, starting at the width on screen.
    /// Returns the width the drag starts from.
    pub fn begin_drag(&mut self, sheet_w: f32) -> f32 {
        if self.widened || self.reviewing() {
            self.width = self.shown(sheet_w);
            self.widened = false;
        }
        self.user_sized = true;
        self.width.min(inline_room(sheet_w))
    }

    /// The gutter moved `dx` to the right of where it was pressed: the panel grows as the pointer goes left, never below
    /// 280, never past half the app (or where it started, if that was wider) or the room beside the conversation.
    pub fn drag(&mut self, start: f32, dx: f32, app_w: f32, sheet_w: f32) {
        let max = (app_w * 0.5).max(start).min(inline_room(sheet_w));
        self.width = MIN_W.max(max.min(start - dx));
    }
}

impl crate::State {
    /// The width the open panel is drawn at, in the sheet as measured.
    pub fn right_shown_w(&self) -> f32 {
        self.right.shown(self.sheet_w)
    }

    /// Widen was toggled by the active tab. The width eases over 200 ms on Fleet's curve, and only for this; a drag lands
    /// at once (`isResizingForWiden`). Reduce Motion lands it at once too, in Rune.
    pub fn set_widened(&mut self, widened: bool, tx: &mut rune::Tx) {
        if self.right.widened != widened {
            tx.motion_curve(rune::Curve::tween(WIDEN_SECS, rune::Easing::cubic(0.2, 0.8, 0.2, 1.0)));
        }
        self.right.widened = widened;
        self.sidebar.save_canvas_widened(widened);
    }

    /// The open session has something to show in the panel, or nothing yet (`noteSessionContent`). A session that was empty
    /// opens its panel by itself the first time it gets something, unless the user chose for it.
    pub fn note_panel_content(&mut self, has_content: bool) {
        let Some(id) = self.open_id() else { return };
        self.sidebar.note_session_content(&id, has_content);
        if let Some(collapsed) = self.sidebar.remembered_right(&id) {
            self.content_open = !collapsed;
        }
    }
}
