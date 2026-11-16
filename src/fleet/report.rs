//! Help → Report a problem, the data and the requests (`lib/problem-report.ts`, `stores/problem-report.ts`, and the request
//! half of `ProblemReportDialog.vue`). Nothing here draws. A report is gathered when it is opened: what the app knows goes
//! to `POST /api/reports/prepare`, the server adds versions and its log and replaces private details, and only what the
//! person reviewed goes to `/send` or `/file`. The requests run on threads; the views (pieces Q and R) read `ReportState`.
use crate::{State, Tx};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn value_str(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or_default().to_string()
}

/// `ReportKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Kind {
    #[default]
    Bug,
    LooksWrong,
    Idea,
}

/// `REPORT_KINDS`, in the order the pills show.
pub const KINDS: [Kind; 3] = [Kind::Bug, Kind::LooksWrong, Kind::Idea];

impl Kind {
    pub fn id(self) -> &'static str {
        match self {
            Kind::Bug => "bug",
            Kind::LooksWrong => "looks-wrong",
            Kind::Idea => "idea",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Kind::Bug => "Something broke",
            Kind::LooksWrong => "Looks wrong",
            Kind::Idea => "Idea",
        }
    }
}

/// `ReportSource`: where the report was opened from. It only changes the wording of the footer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Help,
    Palette,
    Failure,
}

/// `ReportOpening`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opening {
    pub from: Source,
    /// The session the report is about; the open session when not given.
    pub session: Option<String>,
    /// Text to start the description with, such as the error a failure card showed.
    pub description: String,
}

/// The dialog's two stages, then the Sent screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Stage {
    #[default]
    Describe,
    Review,
    Sent,
}

/// What a request is running (`busy`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Busy {
    Prepare,
    Send,
    Save,
}

/// `ReportFact`: a name and a value shown to the person and sent along.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fact {
    pub name: String,
    pub value: String,
}

impl Fact {
    pub fn new(name: &str, value: impl Into<String>) -> Fact {
        Fact { name: name.into(), value: value.into() }
    }
    fn json(&self) -> Value {
        json!({"name": self.name, "value": self.value})
    }
}

/// `ReportIncludes`: the parts that are ticked, as the server sees them. The screenshot isn't one of them (it is sent
/// by the client, in `/send`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Includes {
    pub environment: bool,
    pub place: bool,
    pub connection: bool,
    pub log: bool,
}

impl Default for Includes {
    fn default() -> Includes {
        Includes { environment: true, place: true, connection: true, log: true }
    }
}

/// A name or an address that must not leave (`clientPrivateValues`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivateKind {
    Machine,
    Url,
}

/// `ReportClientContext`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Client {
    pub app: Option<String>,
    pub screen: Option<String>,
    pub shown_status: Option<String>,
    pub place: Vec<Fact>,
    pub connection: Vec<Fact>,
    pub private_values: Vec<(PrivateKind, String)>,
}

/// `PrepareReportRequest`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrepareRequest {
    pub kind: Kind,
    pub description: String,
    pub expected: Option<String>,
    pub session: Option<String>,
    pub include: Includes,
    pub client: Client,
}

impl PrepareRequest {
    /// The body of `POST /api/reports/prepare`. An empty "expected" is `null`; the server's own field is `where`.
    pub fn body(&self) -> Value {
        let c = &self.client;
        json!({
            "kind": self.kind.id(),
            "description": self.description,
            "expected": self.expected.as_deref().map(str::trim).filter(|e| !e.is_empty()),
            "sessionId": self.session,
            "include": {
                "environment": self.include.environment,
                "where": self.include.place,
                "connection": self.include.connection,
                "log": self.include.log,
            },
            "client": {
                "app": c.app,
                "screen": c.screen,
                "shownStatus": c.shown_status,
                "where": c.place.iter().map(Fact::json).collect::<Vec<_>>(),
                "connection": c.connection.iter().map(Fact::json).collect::<Vec<_>>(),
                "privateValues": c.private_values.iter().map(|(k, v)| json!({
                    "kind": match k { PrivateKind::Machine => "machine", PrivateKind::Url => "url" },
                    "value": v,
                })).collect::<Vec<_>>(),
            },
        })
    }
}

/// `ReportReplacement`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replacement {
    pub label: String,
    pub kind: String,
    pub shown: String,
}

/// `PreparedReport`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Prepared {
    pub title: String,
    pub body: String,
    pub log: Option<String>,
    pub log_entries: u64,
    pub labels: Vec<String>,
    pub replacements: Vec<Replacement>,
    /// What was left out: the item and why.
    pub problems: Vec<(String, String)>,
    pub can_send: bool,
}

impl Prepared {
    /// Reads the server's answer. A body with no title or text isn't a report.
    pub fn parse(v: &Value) -> Option<Prepared> {
        let text = |key: &str| v.get(key).and_then(Value::as_str).map(str::to_string);
        let list = |key: &str| v.get(key).and_then(Value::as_array).cloned().unwrap_or_default();
        Some(Prepared {
            title: text("title")?,
            body: text("body")?,
            log: text("log"),
            log_entries: v.get("logEntries").and_then(Value::as_u64).unwrap_or(0),
            labels: list("labels").iter().filter_map(|l| l.as_str().map(str::to_string)).collect(),
            replacements: list("replacements")
                .iter()
                .map(|r| Replacement { label: value_str(r, "label"), kind: value_str(r, "kind"), shown: value_str(r, "shown") })
                .collect(),
            problems: list("problems").iter().map(|p| (value_str(p, "item"), value_str(p, "reason"))).collect(),
            can_send: v.get("canSend").and_then(Value::as_bool).unwrap_or(false),
        })
    }
}

/// `SendReportRequest`: the reviewed report. The screenshot is a `data:` URL, left out once removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendRequest {
    pub kind: Kind,
    pub title: String,
    pub body: String,
    pub log: Option<String>,
    pub screenshot: Option<String>,
    pub contact: Option<String>,
    pub labels: Vec<String>,
}

impl SendRequest {
    /// What the person reviewed: the edited title (the server's own when blank), the prepared text as it came, the
    /// screenshot only when it is still attached, a contact only when one was typed.
    pub fn reviewed(kind: Kind, prepared: &Prepared, title: &str, screenshot: Option<&str>, contact: &str) -> SendRequest {
        let (title, contact) = (title.trim(), contact.trim());
        SendRequest {
            kind,
            title: if title.is_empty() { prepared.title.clone() } else { title.to_string() },
            body: prepared.body.clone(),
            log: prepared.log.clone(),
            screenshot: screenshot.map(str::to_string),
            contact: (!contact.is_empty()).then(|| contact.to_string()),
            labels: prepared.labels.clone(),
        }
    }

    /// The body of `/api/reports/send` and `/api/reports/file`.
    pub fn body(&self) -> Value {
        json!({
            "kind": self.kind.id(),
            "title": self.title,
            "body": self.body,
            "log": self.log,
            "screenshot": self.screenshot,
            "contact": self.contact,
            "labels": self.labels,
        })
    }
}

/// The footer's words when a request fails and the server said nothing.
pub const PREPARE_FAILED: &str = "Couldn't gather the report. Try again.";
pub const SEND_FAILED: &str = "Couldn't send the report. Try again, or use Save as file.";
pub const SAVE_FAILED: &str = "Couldn't save the report.";

/// The server's own words win (`extractApiError`); otherwise `fallback`. `error` may be text or an object with a message.
pub fn error_words(status: u16, body: &str, fallback: &str) -> String {
    let _ = status;
    let Ok(parsed) = serde_json::from_str::<Value>(body) else { return fallback.to_string() };
    let said = |v: &Value| -> Option<String> {
        match v {
            Value::String(t) => Some(t.clone()),
            Value::Object(o) => ["message", "detail", "title", "error"].iter().find_map(|k| o.get(*k)?.as_str().map(str::to_string)),
            _ => None,
        }
    };
    parsed.get("error").and_then(said).or_else(|| said(&parsed)).filter(|t| !t.trim().is_empty()).unwrap_or_else(|| fallback.to_string())
}

/// "Desktop app 0.38.3 · linux".
pub fn describe_app(version: &str, os: &str) -> String {
    format!("Desktop app {version} · {os}")
}

/// The part of Fleet the person was in, from the address: "Sessions", "Settings", "Automations"…
pub fn describe_screen(path: &str) -> String {
    let first = path.split('/').find(|p| !p.is_empty());
    match first {
        None | Some("sessions") => "Sessions".to_string(),
        Some(first) => {
            let mut chars = first.chars();
            let head = chars.next().map(|c| c.to_uppercase().to_string()).unwrap_or_default();
            format!("{head}{}", chars.as_str().replace('-', " "))
        }
    }
}

/// `clientPrivateValues`: other machines' names and addresses, then the address Fleet is open at.
pub fn private_values<'a>(machines: impl IntoIterator<Item = (&'a str, &'a str)>, own: &str) -> Vec<(PrivateKind, String)> {
    let mut values = Vec::new();
    for (name, address) in machines {
        if !name.is_empty() {
            values.push((PrivateKind::Machine, name.to_string()));
        }
        if !address.is_empty() {
            values.push((PrivateKind::Url, address.to_string()));
        }
    }
    values.push((PrivateKind::Url, own.to_string()));
    values
}

/// The window's size and look: "1360×880 at 2×" and the theme (`whereFacts`).
pub fn where_facts(width: f32, height: f32, scale: f32, dark: bool) -> Vec<Fact> {
    let scale = if scale > 0.0 { scale } else { 1.0 };
    vec![
        Fact::new("Window", format!("{}×{} at {scale}×", width.round(), height.round())),
        Fact::new("Theme", if dark { "dark" } else { "light" }),
    ]
}

// ─── labels ──────────────────────────────────────────────────────────────────────────────────────────────────────────

/// A text run of a prepared report, and whether it is a `‹kind-n›` label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub text: String,
    pub label: bool,
}

/// The end of a label that starts at `from` (a `‹`): `‹`, lowercase letters, an optional `-` and digits, `›`.
fn label_end(text: &str, from: usize) -> Option<usize> {
    let rest = text[from..].strip_prefix('‹')?;
    let letters = rest.bytes().take_while(u8::is_ascii_lowercase).count();
    if letters == 0 {
        return None;
    }
    let mut at = letters;
    if rest[at..].starts_with('-') {
        let digits = rest[at + 1..].bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 {
            return None;
        }
        at += 1 + digits;
    }
    rest[at..].starts_with('›').then(|| from + '‹'.len_utf8() + at + '›'.len_utf8())
}

/// `splitLabels`: the text in plain runs and labels, for marking what was replaced.
pub fn split_labels(text: &str) -> Vec<Run> {
    let mut runs = Vec::new();
    let (mut last, mut at) = (0, 0);
    while let Some(found) = text[at..].find('‹') {
        let start = at + found;
        match label_end(text, start) {
            Some(end) => {
                if start > last {
                    runs.push(Run { text: text[last..start].into(), label: false });
                }
                runs.push(Run { text: text[start..end].into(), label: true });
                (last, at) = (end, end);
            }
            None => at = start + '‹'.len_utf8(),
        }
    }
    if last < text.len() {
        runs.push(Run { text: text[last..].into(), label: false });
    }
    runs
}

/// `countLabels`.
pub fn count_labels(text: Option<&str>) -> usize {
    text.map_or(0, |t| split_labels(t).iter().filter(|r| r.label).count())
}

/// `summarizeReplacements`: "3 folders, 1 session name, 1 token", in the order each kind first appears.
pub fn summarize_replacements(replacements: &[Replacement]) -> String {
    let mut counts: Vec<(&str, usize)> = Vec::new();
    for r in replacements {
        match counts.iter_mut().find(|(k, _)| *k == r.kind) {
            Some((_, n)) => *n += 1,
            None => counts.push((&r.kind, 1)),
        }
    }
    counts
        .iter()
        .map(|(kind, n)| {
            let (one, many) = match *kind {
                "folder" => ("folder".to_string(), "folders".to_string()),
                "session" => ("session name".into(), "session names".into()),
                "branch" => ("branch".into(), "branches".into()),
                "user" => ("user name".into(), "user names".into()),
                "machine" => ("machine or address".into(), "machines or addresses".into()),
                "url" => ("address".into(), "addresses".into()),
                "secret" => ("token".into(), "tokens".into()),
                "email" => ("email address".into(), "email addresses".into()),
                other => (other.to_string(), format!("{other}s")),
            };
            format!("{n} {}", if *n == 1 { one } else { many })
        })
        .collect::<Vec<_>>()
        .join(", ")
}

// ─── the file ────────────────────────────────────────────────────────────────────────────────────────────────────────

/// The name from `content-disposition: attachment; filename="…"`, else Fleet's own default. Only the last path part is
/// kept, so a server can't name a file outside the downloads folder.
pub fn file_name(disposition: &str) -> String {
    let named = disposition.split_once("filename=").map(|(_, rest)| {
        let rest = rest.trim_start_matches('"');
        rest.split(['"', ';']).next().unwrap_or("").trim().to_string()
    });
    let last = named.as_deref().and_then(|n| n.rsplit(['/', '\\']).next()).unwrap_or("");
    if last.is_empty() || last == "." || last == ".." { "fleet-report.zip".to_string() } else { last.to_string() }
}

/// A path in `dir` for `name` that doesn't exist yet: `name`, then `name (1)`… with the extension kept.
pub fn free_path(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s, format!(".{e}")),
        _ => (name, String::new()),
    };
    (1..).map(|n| dir.join(format!("{stem} ({n}){ext}"))).find(|p| !p.exists()).expect("a free name")
}

/// The system's downloads folder: `Downloads` in the home folder, which is where all three systems keep it by default.
pub fn downloads_dir() -> Option<PathBuf> {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(var).filter(|h| !h.is_empty()).map(|h| PathBuf::from(h).join("Downloads"))
}

/// Writes the zip the server made to `dir` (made if missing) and says where.
pub fn write_zip(dir: &Path, disposition: &str, bytes: &[u8]) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let path = free_path(dir, &file_name(disposition));
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(path)
}

// ─── the state ───────────────────────────────────────────────────────────────────────────────────────────────────────

/// Whether the report is open and what its requests last said. Answers that arrive after the report was closed, or after
/// a newer request began, are dropped (`generation`).
#[derive(Debug, Default)]
pub struct ReportState {
    pub open: bool,
    pub opening: Option<Opening>,
    pub stage: Stage,
    pub busy: Option<Busy>,
    pub prepared: Option<Prepared>,
    /// In the footer, `role=alert`.
    pub error: Option<String>,
    pub sent_id: Option<String>,
    /// Where Save as file put the zip.
    pub saved: Option<PathBuf>,
    generation: u64,
    /// Where the zip goes; the system's downloads folder unless a test says otherwise.
    pub downloads: Option<PathBuf>,
}

impl ReportState {
    /// `show`: opens the report; nothing while one is open. (The screenshot and its wait are piece Q's.)
    pub fn show(&mut self, opening: Opening) -> bool {
        if self.open {
            return false;
        }
        *self = ReportState {
            open: true,
            opening: Some(opening),
            downloads: self.downloads.take(),
            generation: self.generation + 1,
            ..Default::default()
        };
        true
    }

    /// `hide`. Esc and the overlay close the report, except while it is sending (`close`).
    pub fn hide(&mut self) -> bool {
        if self.busy == Some(Busy::Send) {
            return false;
        }
        *self = ReportState { downloads: self.downloads.take(), generation: self.generation + 1, ..Default::default() };
        true
    }

    /// Back from Review to Describe: what was prepared is dropped, so Review asks again.
    pub fn back(&mut self) {
        if self.busy.is_none() && self.stage == Stage::Review {
            self.stage = Stage::Describe;
            self.prepared = None;
            self.error = None;
        }
    }

    fn begin(&mut self, what: Busy) -> Option<u64> {
        if !self.open || self.busy.is_some() {
            return None;
        }
        self.busy = Some(what);
        self.error = None;
        self.generation += 1;
        Some(self.generation)
    }

    fn finished(&mut self, generation: u64) -> bool {
        let current = self.open && self.generation == generation;
        if current {
            self.busy = None;
        }
        current
    }
}

impl State {
    /// `review`: asks the server to prepare the report. Nothing while a request runs or the description is blank.
    pub fn report_prepare(&mut self, request: PrepareRequest) {
        let Some(link) = self.link.clone() else { return };
        if request.description.trim().is_empty() {
            return;
        }
        let Some(generation) = self.report.begin(Busy::Prepare) else { return };
        let body = request.body();
        link.off_thread(
            move |base| super::http::report_prepare(base, &body),
            move |s, _, result| {
                if !s.report.finished(generation) {
                    return;
                }
                match result.and_then(|v| Prepared::parse(&v).ok_or_else(|| PREPARE_FAILED.to_string())) {
                    Ok(prepared) => {
                        s.report.prepared = Some(prepared);
                        s.report.stage = Stage::Review;
                    }
                    Err(why) => s.report.error = Some(why),
                }
            },
        );
    }

    /// `send`: the reviewed report goes to the inbox through this Fleet; its id shows on the Sent screen.
    pub fn report_send(&mut self, request: SendRequest) {
        let Some(link) = self.link.clone() else { return };
        let Some(generation) = self.report.begin(Busy::Send) else { return };
        let body = request.body();
        link.off_thread(
            move |base| super::http::report_send(base, &body),
            move |s, _, result| {
                if !s.report.finished(generation) {
                    return;
                }
                match result {
                    Ok(id) => {
                        s.report.sent_id = Some(id);
                        s.report.stage = Stage::Sent;
                    }
                    Err(why) => s.report.error = Some(why),
                }
            },
        );
    }

    /// `saveFile`: the zip is fetched and written to the downloads folder, both off the UI thread.
    pub fn report_save(&mut self, request: SendRequest, tx: &mut Tx) {
        let Some(link) = self.link.clone() else { return };
        let Some(dir) = self.report.downloads.clone().or_else(downloads_dir) else {
            self.report.error = Some(SAVE_FAILED.to_string());
            return;
        };
        let Some(generation) = self.report.begin(Busy::Save) else { return };
        let body = request.body();
        link.off_thread(
            move |base| {
                let (disposition, bytes) = super::http::report_file(base, &body)?;
                write_zip(&dir, &disposition, &bytes).map_err(|_| SAVE_FAILED.to_string())
            },
            move |s, tx, result| {
                if !s.report.finished(generation) {
                    return;
                }
                match result {
                    Ok(path) => {
                        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                        s.report.saved = Some(path);
                        s.toast(format!("Saved {name}"), None, 3.0, tx);
                    }
                    Err(why) => s.report.error = Some(why),
                }
            },
        );
        tx.wake_after(std::time::Duration::from_millis(50));
    }
}
