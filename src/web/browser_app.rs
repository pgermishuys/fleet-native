//! The browser tab's app (component 14, piece I; rows 14.59, 14.61 to 14.64 and 14.66): what the app's panel, strip and output
//! drawer show and do. Ports the app half of `components/canvas/BrowserCanvas.vue`. Plain Rust with no window; the view only
//! draws it. The app itself is `fleet::app_runs`.
use super::browser::BrowserUi;
use crate::State;
use crate::fleet::app_runs::{AppRun, AppStatus, address_for_port, current_run_lines};

/// How often output is fetched while it's on screen and the app runs (`OUTPUT_POLL_MS`). Output isn't pushed.
pub const OUTPUT_POLL: f64 = 1.0;
/// How many of the current run's last lines the panel shows (`TAIL_LINES`).
pub const TAIL_LINES: usize = 8;
/// How long a refused Start, Restart or Stop stays up.
pub const ACTION_ERROR_SECS: f64 = 10.0;

/// What a browser tab keeps of its app besides the app (which `AppRuns` holds).
#[derive(Clone, Debug, Default)]
pub struct AppUi {
    /// The output drawer is open.
    pub show_logs: bool,
    /// A Start, Restart or Stop is on its way: the buttons wait.
    pub busy: bool,
    /// Why Fleet refused one, and the time it goes.
    pub action_error: Option<(String, f64)>,
    /// The status the last look saw, so a change from one to another can be told from one that was always there.
    pub last_status: Option<AppStatus>,
    /// The tab was on screen at the last look: it opens, or comes back (`onActivated`).
    pub visible: bool,
    /// Output is wanted, and whether the app is live (every second), else once; none while nothing needs it.
    pub out_key: Option<bool>,
    pub polled_at: f64,
    /// The tab's width, for the strip's narrow forms (Fleet's container query).
    pub width: Option<f32>,
}

/// What the page area shows instead of the page (`panel`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Panel {
    Starting,
    Waiting,
    Exited,
    Stopped,
}

/// The panel for an app's status, and whether the tab has a page to frame.
pub fn panel_of(status: Option<AppStatus>, has_target: bool) -> Option<Panel> {
    match status {
        Some(AppStatus::Starting) => Some(Panel::Starting),
        Some(AppStatus::Exited) => Some(Panel::Exited),
        Some(AppStatus::Stopped) => Some(Panel::Stopped),
        _ if !has_target => Some(Panel::Waiting),
        _ => None,
    }
}

/// Fleet's `isLive`: the app is running, or on its way, or its build failed.
pub fn is_live(status: Option<AppStatus>) -> bool {
    matches!(status, Some(AppStatus::Starting | AppStatus::Running | AppStatus::BuildFailed))
}

/// The panel's title (`panelTitle`).
pub fn panel_title(panel: Panel, app: Option<&AppRun>, has_app_id: bool) -> String {
    match panel {
        Panel::Starting => "Starting\u{2026}".to_string(),
        Panel::Exited => match app.and_then(|a| a.exit_code) {
            Some(code) => format!("The app exited with code {code}"),
            None => "The app exited".to_string(),
        },
        Panel::Stopped => "The app is stopped".to_string(),
        Panel::Waiting if has_app_id => "Waiting for the app to serve a page\u{2026}".to_string(),
        Panel::Waiting => "Loading\u{2026}".to_string(),
    }
}

/// The strip's status word (`statusLabel`).
pub fn status_label(app: &AppRun) -> String {
    match app.status {
        AppStatus::Exited => app.exit_code.map_or_else(|| "exited".to_string(), |c| format!("exited ({c})")),
        AppStatus::BuildFailed => "build failed".to_string(),
        AppStatus::Starting => "starting".to_string(),
        AppStatus::Running => "running".to_string(),
        AppStatus::Stopped => "stopped".to_string(),
    }
}

impl BrowserUi {
    /// The app this tab shows, if the app is one Fleet runs.
    pub fn app_id(&self) -> Option<&str> {
        self.app_id.as_deref()
    }
}

impl State {
    /// The app a browser tab shows, as the store has it.
    pub fn browser_app(&self, tab: &str) -> Option<AppRun> {
        self.apps.app(self.browser_ui(tab)?.app_id()?).cloned()
    }

    /// What the tab's page area shows instead of the page, if anything.
    pub fn browser_panel(&self, tab: &str) -> Option<Panel> {
        let ui = self.browser_ui(tab);
        let status = self.browser_app(tab).map(|a| a.status);
        panel_of(status, ui.is_some_and(|u| u.target.is_some()))
    }

    /// The panel's last lines: this run's, not an earlier run's crash under "Starting…" (`tail`).
    pub fn browser_tail(&self, tab: &str) -> Vec<String> {
        let Some(id) = self.browser_ui(tab).and_then(|u| u.app_id()) else { return Vec::new() };
        let lines = self.apps.output(id).map(|o| o.lines.as_slice()).unwrap_or_default();
        let run = current_run_lines(lines);
        run[run.len().saturating_sub(TAIL_LINES)..].to_vec()
    }

    /// The tab's app: loaded when its tab opens or comes back, the status it changed to acted on, the output fetched while it's
    /// seen. Called from `keep_browser` for the tab on screen, with its page. True when something to draw changed.
    pub(super) fn keep_browser_app(&mut self, tab: &str, page_url: &str, app_id: Option<&str>, tx: &mut rune::Tx) -> bool {
        let session = self.canvas_session();
        let now = self.clock;
        let mut changed = false;
        let ui = self.web.browsers.entry(tab.to_string()).or_default();
        ui.app_id = app_id.map(str::to_string);
        let came = !ui.app.visible;
        ui.app.visible = true;
        if came {
            // The frame loads again when the tab comes back (`onActivated`), which isn't the page reloading itself.
            ui.expecting_load = true;
        }
        let Some(app_id) = app_id else { return changed };
        if came {
            self.load_app(&session, app_id, |_, _| {});
        }
        changed |= self.browser_app_status(tab, page_url);
        changed |= self.browser_output(tab, &session, app_id, tx);
        if let Some((_, until)) = self.browser_ui(tab).and_then(|u| u.app.action_error.clone()) {
            if now >= until {
                self.web.browsers.entry(tab.to_string()).or_default().app.action_error = None;
                changed = true;
            } else {
                tx.wake_after(std::time::Duration::from_secs_f64((until - now).clamp(0.05, ACTION_ERROR_SECS)));
            }
        }
        changed
    }

    /// The watch on `[status, url]`: back from a restart shows the page again and pulses; a failed build opens the output once.
    fn browser_app_status(&mut self, tab: &str, page_url: &str) -> bool {
        let Some(app) = self.browser_app(tab) else { return false };
        let now = self.clock;
        let ui = self.web.browsers.entry(tab.to_string()).or_default();
        let before = ui.app.last_status.replace(app.status);
        if before == Some(app.status) {
            return false;
        }
        let mut show = None;
        let mut pulse = false;
        if app.status == AppStatus::Running && matches!(before, Some(b) if !matches!(b, AppStatus::Running | AppStatus::BuildFailed)) {
            ui.expecting_load = true;
            // A tab with a page of its own follows the app when it moved to another port.
            if let (false, Some(url), Some(target)) = (page_url.is_empty(), app.url.as_deref(), ui.target.as_ref())
                && url::Url::parse(url).is_ok_and(|u| u.origin() != target.origin())
            {
                show = Some(url.to_string());
            }
            pulse = ui.last_href.is_some();
        }
        if app.status == AppStatus::BuildFailed {
            ui.app.show_logs = true;
        }
        if let Some(url) = show {
            self.browser_show(tab, &url);
        }
        if pulse {
            self.canvases.mark_updated(tab, now);
        }
        true
    }

    /// Output only while someone can see it: every second while the app runs, once when it doesn't (the watch on
    /// `[active, outputOnScreen, isLive, appId]`).
    fn browser_output(&mut self, tab: &str, session: &str, app_id: &str, tx: &mut rune::Tx) -> bool {
        let now = self.clock;
        let panel = self.browser_panel(tab);
        let live = is_live(self.browser_app(tab).map(|a| a.status));
        let ui = self.web.browsers.entry(tab.to_string()).or_default();
        let wanted = ui.app.show_logs || matches!(panel, Some(Panel::Starting | Panel::Exited));
        let key = wanted.then_some(live);
        let fetch = if key != ui.app.out_key {
            ui.app.out_key = key;
            key.is_some()
        } else {
            key == Some(true) && now - ui.app.polled_at >= OUTPUT_POLL
        };
        if fetch {
            ui.app.polled_at = now;
            self.fetch_app_output(session, app_id);
        }
        if key == Some(true) {
            let due = (self.browser_ui(tab).map_or(now, |u| u.app.polled_at) + OUTPUT_POLL - now).max(0.05);
            tx.wake_after(std::time::Duration::from_secs_f64(due));
        }
        false
    }

    /// Start, Restart and Stop (`appAction`): the buttons wait while Fleet answers; a refusal shows for ten seconds.
    pub fn browser_app_act(&mut self, tab: &str, action: &'static str) {
        let session = self.canvas_session();
        let Some(app_id) = self.browser_ui(tab).and_then(|u| u.app_id.clone()) else { return };
        if self.browser_ui(tab).is_none_or(|u| u.app.busy) || self.link_for(&session).is_none() {
            return;
        }
        let ui = self.web.browsers.entry(tab.to_string()).or_default();
        ui.app.busy = true;
        ui.app.action_error = None;
        let tab = tab.to_string();
        self.act_on_app(&session, &app_id, action, move |s, why| {
            let until = s.clock + ACTION_ERROR_SECS;
            if let Some(ui) = s.web.browsers.get_mut(&tab) {
                ui.app.busy = false;
                ui.app.action_error = why.map(|w| (w, until));
            }
        });
    }

    /// The output drawer's button.
    pub fn browser_toggle_output(&mut self, tab: &str) {
        let ui = self.web.browsers.entry(tab.to_string()).or_default();
        ui.app.show_logs = !ui.app.show_logs;
    }

    /// The port select: show the app on another of its ports (`pickPort`).
    pub fn browser_pick_port(&mut self, tab: &str, port: u16) {
        let printed = self.browser_app(tab).map(|a| a.printed_urls).unwrap_or_default();
        self.browser_show(tab, &address_for_port(port, &printed));
    }

    /// The port of the page shown (`currentPort`).
    pub fn browser_port(&self, tab: &str) -> Option<u16> {
        self.browser_ui(tab)?.target.as_ref()?.port_or_known_default()
    }
}
