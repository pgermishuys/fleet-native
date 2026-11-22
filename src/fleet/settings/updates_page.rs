//! The words and rules of Settings → Updates & about's Fleet and Harnesses groups (`SystemSection.vue`, `DesktopAppUpdates.vue`,
//! `HarnessUpdatesGroup.vue`, `SettingsNavPanel.vue`). Plain Rust: no Rune, no I/O. The page's data lives in
//! `fleet::updates` (the status store) and `update` (the app's own updater); this decides what they say.
use crate::composer::format::format_bytes;
use crate::fleet::new_session::harness_choice::HarnessEntry;
use crate::fleet::updates::strip::format_relative_words;
use crate::fleet::updates::{Status, UpdateStatus};
use crate::update::{Phase, Update};

/// How a status line is inked: `--running-text`, `--accent-text`, `--idle-text`, `--error-text`, or the plain text colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Plain,
    Ok,
    Info,
    Warn,
    Error,
}

/// What this Fleet's server row says (`statusLabel`, `statusVariant`). `status` is `None` until the first answer.
pub fn server_label(status: Option<&UpdateStatus>) -> String {
    let latest = |s: &UpdateStatus| s.latest_version.clone().unwrap_or_default();
    match status {
        Some(s) => match s.status {
            Status::UpToDate => "Up to date".into(),
            Status::Available => format!("Update available: v{}", latest(s)),
            Status::Downloading => format!("Downloading v{}\u{2026}", latest(s)),
            Status::Staged => format!("v{} is ready", latest(s)),
            Status::Error => "Update check failed".into(),
            Status::Managed => "Updates come from the Fleet app".into(),
            Status::Unknown => "Checking for updates\u{2026}".into(),
        },
        None => "Checking for updates\u{2026}".into(),
    }
}

pub fn server_tone(status: Option<&UpdateStatus>) -> Tone {
    match status.map(|s| s.status) {
        Some(Status::UpToDate) => Tone::Ok,
        Some(Status::Available | Status::Downloading) => Tone::Info,
        Some(Status::Staged) => Tone::Warn,
        Some(Status::Error) => Tone::Error,
        _ => Tone::Plain,
    }
}

/// A server update waits: the line under the version points at Machines (`updateWaiting`).
pub fn update_waiting(status: Option<&UpdateStatus>) -> bool {
    matches!(status.map(|s| s.status), Some(Status::Available | Status::Downloading | Status::Staged))
}

/// "Check for updates" is offered unless a download runs, one is ready, or the desktop app manages them (`canCheck`).
pub fn can_check(status: Option<&UpdateStatus>) -> bool {
    !matches!(status.map(|s| s.status), Some(Status::Downloading | Status::Staged | Status::Managed))
}

pub fn can_download(status: Option<&UpdateStatus>) -> bool {
    status.map(|s| s.status) == Some(Status::Available)
}

/// The version the What's new link opens at (`notesVersion`).
pub fn notes_version(status: Option<&UpdateStatus>) -> Option<String> {
    let s = status?;
    if !update_waiting(Some(s)) {
        return None;
    }
    s.latest_version.clone().filter(|v| !v.is_empty())
}

/// The bar under a downloading row: its percent (`None` when the size isn't known: the bar is full) and the words under it
/// (`downloadProgressPercent`, `downloadProgressLabel`).
pub fn download_progress(status: Option<&UpdateStatus>) -> Option<(Option<u32>, Option<String>)> {
    let s = status.filter(|s| s.status == Status::Downloading)?;
    let received = s.download_bytes_received.unwrap_or(0);
    let total = s.download_bytes_total.filter(|t| *t > 0);
    let percent = total.map(|t| ((received as f64 / t as f64 * 100.0).round() as u32).min(100));
    let words = match total {
        Some(t) => format!("{} / {}", format_bytes(Some(received as f64)), format_bytes(Some(t as f64))),
        None => format_bytes(Some(received as f64)),
    };
    Some((percent, Some(words)))
}

/// The line under "Installed version": "v0.72.0", with ", update waiting ·" and a link while an update waits.
pub fn version_text(status: Option<&UpdateStatus>) -> Option<String> {
    let s = status?;
    Some(format!("v{}{}", s.current_version, if update_waiting(Some(s)) { ", update waiting \u{b7}" } else { "" }))
}

/// The menu's dot beside Updates & about: accent while an update is out or coming down, the idle colour once it is staged
/// (`showUpdateDot`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dot {
    Accent,
    Idle,
}

pub fn menu_dot(status: Option<&UpdateStatus>) -> Option<Dot> {
    match status.map(|s| s.status) {
        Some(Status::Available | Status::Downloading) => Some(Dot::Accent),
        Some(Status::Staged) => Some(Dot::Idle),
        _ => None,
    }
}

/// The badge's words for a screen reader: "2 machines can update".
pub fn waiting_label(waiting: usize) -> String {
    format!("{waiting} {} can update", if waiting == 1 { "machine" } else { "machines" })
}

/// A harness row's line: "Installed" (found, no version), "Not installed", else the version, with the CLI's for Copilot.
pub fn harness_version_line(h: &HarnessEntry) -> String {
    let Some(version) = h.version.as_deref().filter(|v| !v.is_empty()) else {
        return if h.available { "Installed".into() } else { "Not installed".into() };
    };
    match h.card.cli_version.as_deref().filter(|c| h.kind == "copilot" && !c.is_empty()) {
        Some(cli) => format!("{version} \u{b7} Copilot CLI {cli}"),
        None => version.to_string(),
    }
}

/// What the app's own row says. Native's updater is build numbers from the rolling release with one dialog, so the states are
/// fewer than Electron's (decision 4 of docs/parity/16-settings.md): no "ready, restart later", and "Update and restart"
/// opens the dialog.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppRow {
    /// Under the title: "build 214", or "dev build" for one that can't update.
    pub build: String,
    pub label: String,
    pub tone: Tone,
    /// A spinner, not an icon.
    pub busy: bool,
    pub percent: Option<u32>,
    /// A muted line under the status (the way the download goes on, when it was last looked).
    pub note: Option<String>,
    /// An error note under the status.
    pub error: Option<String>,
    pub can_check: bool,
    /// The check button shows a spinner and waits.
    pub checking: bool,
    /// "Update and restart": opens the dialog.
    pub can_update: bool,
    /// The button's words: "Update and restart", or "Download the new .deb" where the install can only link.
    pub update_label: &'static str,
}

pub fn app_row(u: &Update, now_ms: i64, offset_secs: i64) -> AppRow {
    let mut row = AppRow {
        build: u.running.map_or("dev build".to_string(), |b| format!("build {b}")),
        label: String::new(),
        tone: Tone::Plain,
        busy: false,
        percent: None,
        note: None,
        error: None,
        can_check: false,
        checking: false,
        can_update: false,
        update_label: if u.notify_only() { "Download the new .deb" } else { "Update and restart" },
    };
    let Some(_) = u.running.filter(|_| !u.off) else {
        row.label = "This build doesn't update itself".into();
        return row;
    };
    let offered = u.available.as_ref().map(|a| a.build);
    let build = offered.map_or(String::new(), |b| b.to_string());
    match &u.phase {
        Phase::Downloading(done, total) => {
            row.label = format!("Downloading build {build}\u{2026}");
            row.busy = true;
            row.tone = Tone::Info;
            row.percent = Some((done * 100 / (*total).max(1)).min(100) as u32);
            return row;
        }
        Phase::Installing => {
            row.label = format!("Installing build {build}\u{2026}");
            row.busy = true;
            row.tone = Tone::Info;
            return row;
        }
        Phase::Failed(why) => {
            row.label = "Update failed".into();
            row.tone = Tone::Error;
            row.error = Some(why.clone());
            row.can_update = offered.is_some();
            return row;
        }
        Phase::Manual { why } => {
            row.label = "Finish the update by hand".into();
            row.tone = Tone::Info;
            row.note = Some(why.clone());
            row.can_update = offered.is_some();
            return row;
        }
        Phase::Idle => {}
    }
    if let Some(a) = &u.available {
        row.label = format!("Build {} is available", a.build);
        row.tone = Tone::Info;
        row.note = Some(if a.link.is_some() {
            format!(
                "This copy came from a .deb, which can't replace itself. Download the new package ({}) and install it.",
                about_size(a.asset.size)
            )
        } else {
            format!(
                "Fleet Native downloads it ({}), checks it, replaces itself and its server, and starts again. Your drafts are kept.",
                about_size(a.asset.size)
            )
        });
        row.can_update = true;
        return row;
    }
    row.can_check = true;
    row.checking = u.checking;
    if u.checking {
        row.label = "Checking for updates\u{2026}".into();
        row.busy = true;
    } else if let Some(why) = &u.check_error {
        row.label = "Update check failed".into();
        row.tone = Tone::Error;
        row.error = Some(why.clone());
    } else if let Some(at) = u.checked_ms {
        row.label = "Up to date".into();
        row.tone = Tone::Ok;
        row.note = Some(format!("Checked {}.", format_relative_words(at, now_ms, offset_secs)));
    } else {
        // The checker's first look hasn't come back.
        row.label = "Checking for updates\u{2026}".into();
        row.busy = true;
    }
    row
}

/// "about 130 MB": the size a download will be, in whole megabytes, said before it starts.
pub fn about_size(bytes: u64) -> String {
    match (bytes as f64 / 1_000_000.0).round() as u64 {
        0 => "under 1 MB".to_string(),
        mb => format!("about {mb} MB"),
    }
}
