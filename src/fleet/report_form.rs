//! Report a problem, the Describe step's own state (`ProblemReportDialog.vue`'s script, and `stores/problem-report.ts`'s
//! screenshot): what was typed, which parts are ticked, the screenshot, and the facts the window gives the server when
//! the report is reviewed. The requests and the dialog's stages are `report.rs`; the drawing is `view/report.rs`.
use super::link::Connection;
use super::report::{
    Client, Includes, Kind, Opening, PrepareRequest, SendRequest, Source, describe_app, describe_screen, private_values, where_facts,
};
use crate::{Rail, State, Tx};
use rune::TextEdit;

/// `maxlength` of the three fields.
pub const DESCRIPTION_MAX: usize = 10_000;
pub const EXPECTED_MAX: usize = 2_000;
pub const CONTACT_MAX: usize = 200;
/// `maxlength` of the Review step's title.
pub const TITLE_MAX: usize = 200;

/// `ReviewFile`: which of the report's files Review shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReviewFile {
    #[default]
    Report,
    Log,
    Screenshot,
}

/// What a part of the report is called and says (`ITEMS`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Item {
    Screenshot,
    Environment,
    Place,
    Connection,
    Log,
}

pub const ITEMS: [Item; 5] = [Item::Screenshot, Item::Environment, Item::Place, Item::Connection, Item::Log];

impl Item {
    pub fn id(self) -> &'static str {
        match self {
            Item::Screenshot => "screenshot",
            Item::Environment => "environment",
            Item::Place => "where",
            Item::Connection => "connection",
            Item::Log => "log",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Item::Screenshot => "Screenshot",
            Item::Environment => "Fleet and system",
            Item::Place => "Where you were",
            Item::Connection => "Connection",
            Item::Log => "Fleet log",
        }
    }
    pub fn detail(self) -> &'static str {
        match self {
            Item::Screenshot => "Your screen when you clicked",
            Item::Environment => "Versions, app, operating system, harnesses",
            Item::Place => "Screen, session status as shown and as the server has it",
            Item::Connection => "Live updates, right now",
            Item::Log => "Last 15 minutes: this session's lines, plus warnings and errors",
        }
    }
}

/// `Screenshot`: the app's own frame as PNG, when it was taken and how big it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shot {
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// "09:14:33 PM", local time (`formatTime` with seconds, in the 12-hour style Fleet's English shows).
    pub taken: String,
}

impl Shot {
    /// The `data:` URL `/api/reports/send` takes.
    pub fn data_url(&self) -> String {
        format!("data:image/png;base64,{}", base64(&self.png))
    }
}

fn base64(bytes: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, b)| n | (u32::from(*b) << (16 - 8 * i)));
        for i in 0..4 {
            out.push(if i <= chunk.len() { char::from(ABC[(n >> (18 - 6 * i) & 63) as usize]) } else { '=' });
        }
    }
    out
}

/// The app's own frame as a picture. Rune draws a window's frame on the GPU and gives the app no way to read it back
/// (`docs/RUNE.md`, "A running app can't take its own picture"), so there is none: Describe says it couldn't, and the
/// report works without one. The seam is here, so a Rune that can is one function.
pub fn grab_frame() -> Option<Shot> {
    None
}

/// The key the screenshot's picture is held under in `State::images`.
pub const SHOT_KEY: &str = "report-screenshot";

/// What the person has typed and ticked.
pub struct ReportForm {
    pub kind: Kind,
    pub description: TextEdit,
    pub expected: TextEdit,
    pub contact: TextEdit,
    /// The parts that are ticked; the screenshot's is `shot_on`.
    pub include: Includes,
    pub shot_on: bool,
    pub screenshot: Option<Shot>,
    /// A screenshot is being taken (`capturing`).
    pub capturing: bool,
    /// Review: the title (the server's, then the person's), the file shown, "Show what was removed", and "Copied" on Sent.
    pub title: TextEdit,
    pub file: ReviewFile,
    pub show_removed: bool,
    pub copied: bool,
}

impl Default for ReportForm {
    fn default() -> ReportForm {
        ReportForm::new("")
    }
}

impl ReportForm {
    /// A new opening starts a new report: the description may start with text (a failure card's), everything ticked.
    pub fn new(description: &str) -> ReportForm {
        let mut d = TextEdit::new(description);
        d.set_caret(description.len());
        ReportForm {
            kind: Kind::Bug,
            description: d,
            expected: TextEdit::new(""),
            contact: TextEdit::new(""),
            include: Includes::default(),
            shot_on: true,
            screenshot: None,
            capturing: false,
            title: TextEdit::new(""),
            file: ReviewFile::Report,
            show_removed: false,
            copied: false,
        }
    }

    pub fn is_on(&self, item: Item) -> bool {
        match item {
            Item::Screenshot => self.shot_on,
            Item::Environment => self.include.environment,
            Item::Place => self.include.place,
            Item::Connection => self.include.connection,
            Item::Log => self.include.log,
        }
    }

    /// Whether the box is ticked and means something: a screenshot that couldn't be taken is off and can't be ticked.
    pub fn counts(&self, item: Item) -> bool {
        self.is_on(item) && (item != Item::Screenshot || self.screenshot.is_some())
    }

    pub fn toggle(&mut self, item: Item) {
        if item == Item::Screenshot && self.screenshot.is_none() {
            return;
        }
        let flag = match item {
            Item::Screenshot => &mut self.shot_on,
            Item::Environment => &mut self.include.environment,
            Item::Place => &mut self.include.place,
            Item::Connection => &mut self.include.connection,
            Item::Log => &mut self.include.log,
        };
        *flag = !*flag;
    }

    /// "n of 5".
    pub fn count(&self) -> usize {
        ITEMS.iter().filter(|i| self.counts(**i)).count()
    }

    /// `hasScreenshot`: taken and still ticked.
    pub fn has_screenshot(&self) -> bool {
        self.shot_on && self.screenshot.is_some()
    }

    /// `removeScreenshot`: unticked, so it isn't sent, and the report file shows again.
    pub fn remove_screenshot(&mut self) {
        self.shot_on = false;
        self.file = ReviewFile::Report;
    }

    /// The description may be reviewed (`!description.trim()`).
    pub fn can_review(&self) -> bool {
        !self.description.text.trim().is_empty()
    }
}

/// "09:14:33 PM" at `ms` in local time.
fn clock(ms: i64, offset_secs: i64) -> String {
    let day = (ms / 1000 + offset_secs).rem_euclid(86_400);
    let hour = day / 3600;
    format!(
        "{:02}:{:02}:{:02} {}",
        if hour % 12 == 0 { 12 } else { hour % 12 },
        day % 3600 / 60,
        day % 60,
        if hour < 12 { "AM" } else { "PM" }
    )
}

impl State {
    /// `store.show`: opens the report and takes the screenshot first, so it shows what the person was looking at. A menu or
    /// the palette the report was chosen from is put away at once. Nothing while a report is open.
    pub fn report_open(&mut self, from: Source, session: Option<String>, description: &str) -> bool {
        let opening = Opening { from, session, description: description.to_string() };
        if !self.report.show(opening) {
            return false;
        }
        self.ui.open = None;
        self.report_form = ReportForm::new(description);
        self.report_grab();
        true
    }

    /// Takes the screenshot, or notes there is none.
    fn report_grab(&mut self) {
        let shot = grab_frame().map(|mut shot| {
            shot.taken = clock(self.now_ms.unwrap_or_else(super::link::now_ms), crate::conv::time::local_offset_secs());
            shot
        });
        self.report_set_screenshot(shot);
    }

    /// Holds a screenshot (or none) and its picture for Describe's thumbnail.
    pub fn report_set_screenshot(&mut self, shot: Option<Shot>) {
        match &shot {
            Some(s) => self.supply_image(SHOT_KEY, &s.png),
            None => self.images.forget(SHOT_KEY),
        }
        self.report_form.screenshot = shot;
        self.report_form.capturing = false;
    }

    /// `retake`: the screenshot again (the dialog is out of the frame in Fleet; here there is no frame to take).
    pub fn report_retake(&mut self) {
        if !self.report.open || self.report_form.capturing {
            return;
        }
        self.report_grab();
    }

    /// `hide` and the screenshot with it.
    pub fn report_close(&mut self) {
        if self.report.hide() {
            self.report_form = ReportForm::default();
            self.images.forget(SHOT_KEY);
        }
    }

    /// The session the report is about: the one it was opened for, else the open one (`sessionId`), and its title.
    pub fn report_about(&self) -> Option<(String, String)> {
        let id = match self.report.opening.as_ref().and_then(|o| o.session.clone()) {
            Some(id) => id,
            None => self.open_id()?,
        };
        let title = self.title_of(&id)?;
        let title = if title.trim().is_empty() { "this session".to_string() } else { title };
        Some((id, title))
    }

    /// What the window knows, for `Client` (`review`).
    fn report_client(&self) -> Client {
        let path = match (&self.route, self.rail) {
            (super::store::Route::Session(id), _) => format!("/sessions/{id}"),
            (super::store::Route::NewSession { .. }, _) => "/sessions/new".to_string(),
            (_, Rail::Fleet) => "/fleet".into(),
            (_, Rail::Board) => "/board".into(),
            (_, Rail::Workflows) => "/workflows".into(),
            (_, Rail::Jobs) => "/jobs".into(),
            (_, Rail::Automations) => "/automations".into(),
            (_, Rail::Analytics) => "/analytics".into(),
            (_, Rail::Plugin(id)) => self.places.rail_item(id).map_or_else(|| "/".into(), |r| r.path.into()),
            (_, Rail::Settings) => "/settings".into(),
            (_, Rail::Sessions) => "/".into(),
        };
        let now = self.now_ms.unwrap_or_else(super::link::now_ms);
        let shown_status = self.report_about().and_then(|(id, _)| {
            let session = self.store.session_by_id(&self.projects, &id)?;
            let offset = crate::conv::time::local_offset_secs();
            let status = super::row_status::row_status(&super::row_status::RowItem::of(session, now), now, offset, true);
            Some(if status.description.is_empty() { status.label } else { status.description }).filter(|w| !w.is_empty())
        });
        let (w, h) = self.ui.sel.window;
        let live = match &self.connection {
            Connection::Online => "connected".to_string(),
            Connection::Connecting => "connecting".to_string(),
            Connection::Down(_) => "reconnecting".to_string(),
            Connection::Refused => "refused".to_string(),
        };
        let machines = self.machines.list.iter().map(|m| (m.name.as_str(), m.base_url.as_str()));
        let own = self.link.as_ref().map(|l| l.base.clone()).unwrap_or_default();
        Client {
            app: Some(describe_app(env!("CARGO_PKG_VERSION"), std::env::consts::OS)),
            screen: Some(describe_screen(&path)),
            shown_status,
            place: where_facts(w, h, 1.0, self.drawn_dark.get()),
            connection: vec![super::report::Fact::new("Live updates", live)],
            private_values: private_values(machines, &own),
        }
    }

    /// `review`: asks the server to prepare what the form holds.
    pub fn report_review(&mut self) {
        if !self.report_form.can_review() {
            return;
        }
        let form = &self.report_form;
        let trim = |e: &rune::TextEdit, max: usize| e.text.chars().take(max).collect::<String>();
        let request = PrepareRequest {
            kind: form.kind,
            description: trim(&form.description, DESCRIPTION_MAX),
            expected: Some(trim(&form.expected, EXPECTED_MAX)),
            session: self.report_about().map(|(id, _)| id),
            include: form.include,
            client: self.report_client(),
        };
        self.report_prepare(request);
    }

    /// A prepared report arrived: its title is the one to edit, and the report file shows first.
    pub fn report_reviewing(&mut self, title: &str) {
        let f = &mut self.report_form;
        f.title = TextEdit::new(title);
        f.title.set_caret(title.len());
        f.file = ReviewFile::Report;
        f.show_removed = false;
    }

    /// `reviewedReport`: what the person reviewed, as the requests take it.
    pub fn report_reviewed(&self) -> Option<SendRequest> {
        let prepared = self.report.prepared.as_ref()?;
        let form = &self.report_form;
        let screenshot = form.screenshot.as_ref().filter(|_| form.has_screenshot()).map(|s| s.data_url());
        let title: String = form.title.text.chars().take(TITLE_MAX).collect();
        Some(SendRequest::reviewed(form.kind, prepared, &title, screenshot.as_deref(), &form.contact.text))
    }

    /// `send`.
    pub fn report_send_reviewed(&mut self) {
        // Send is off when this Fleet has no inbox (`:disabled="!prepared?.canSend"`).
        if !self.report.prepared.as_ref().is_some_and(|p| p.can_send) {
            return;
        }
        if let Some(request) = self.report_reviewed() {
            self.report_send(request);
        }
    }

    /// `saveFile`.
    pub fn report_save_reviewed(&mut self, tx: &mut Tx) {
        if let Some(request) = self.report_reviewed() {
            self.report_save(request, tx);
        }
    }

    /// Back to Describe: what was typed and ticked stays.
    pub fn report_back(&mut self) {
        self.report.back();
    }

    /// `copyId`: the report id on the clipboard, and "Copied".
    pub fn report_copy_id(&mut self, tx: &mut Tx) {
        if let Some(id) = self.report.sent_id.clone() {
            tx.copy(id);
            self.report_form.copied = true;
        }
    }
}
