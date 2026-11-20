//! Closing the window, quitting, and what stays running (component 24, piece E): `onWindowClose`, `before-quit` and
//! `confirmStoppingSessions` of `desktop/src/main.ts`. The rules are pure functions of the system, whether the app started
//! the Fleet, and how many sessions it has working; the flow around them asks the Fleet on a thread and acts when it answers.
//! The window and the tray icon are behind [`Surface`], so a test runs the whole flow with no window and no D-Bus.
use crate::State;
use crate::fleet::link::Post;
use rune::Tx;
use std::sync::Arc;

/// The two families of close rules: macOS keeps the app in the Dock, the others quit or go to the tray.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum System {
    Mac,
    Other,
}

impl System {
    pub fn current() -> System {
        if cfg!(target_os = "macos") { System::Mac } else { System::Other }
    }
}

/// What closing the window does first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Close {
    /// The app ends (a Fleet it only connected to is left running).
    Quit,
    /// The window goes and the app stays (macOS: the Dock icon brings it back).
    Hide,
    /// Its own Fleet: ask how many sessions are working before deciding.
    Ask,
}

/// `onWindowClose`, before the Fleet is asked: macOS hides; elsewhere a Fleet this app didn't start is left alone and
/// closing quits; its own Fleet is asked.
pub fn on_close(system: System, owned: bool) -> Close {
    match (system, owned) {
        (System::Mac, _) => Close::Hide,
        (System::Other, false) => Close::Quit,
        (System::Other, true) => Close::Ask,
    }
}

/// What a close comes to once the working count is known.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Quit,
    Hide,
    /// The window goes; the app and its Fleet keep running behind a tray icon.
    Tray(usize),
}

/// Sessions working keep the app running in the tray; none, and closing quits.
pub fn after_status(working: usize) -> Outcome {
    if working > 0 { Outcome::Tray(working) } else { Outcome::Quit }
}

/// The whole rule for one case: `working` is what `/api/desktop/status` said (0 when it couldn't be asked).
pub fn decide(system: System, owned: bool, working: usize) -> Outcome {
    match on_close(system, owned) {
        Close::Quit => Outcome::Quit,
        Close::Hide => Outcome::Hide,
        Close::Ask => after_status(working),
    }
}

/// The tray's tooltip, `hideToTray`.
pub fn tooltip(working: usize) -> String {
    if working == 1 { "Fleet: a session is working".into() } else { format!("Fleet: {working} sessions are working") }
}

/// The note Fleet posts as it goes to the tray: (title, body). Posted by `State::post_still_running` (24 F).
pub fn still_running(working: usize) -> (String, String) {
    let who = if working == 1 { "A session is".to_string() } else { format!("{working} sessions are") };
    ("Fleet is still running".into(), format!("{who} still working. Quit from the tray icon when you're done."))
}

/// The quit dialog's message, `confirmStoppingSessions`.
pub fn quit_message(working: usize) -> String {
    if working == 1 { "A session is still working".into() } else { format!("{working} sessions are still working") }
}

pub const QUIT_DETAIL: &str = "Quitting Fleet stops them.";
pub const QUIT_BUTTON: &str = "Quit Fleet";
pub const CANCEL_BUTTON: &str = "Cancel";

/// What a tray's menu or icon asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayAction {
    Show,
    Quit,
}

pub const MENU_SHOW: &str = "show";
pub const MENU_QUIT: &str = "quit";

/// A click on the icon and "Show Fleet" show the window; "Quit Fleet" quits.
pub fn tray_action(event: &rune::tray::TrayEvent) -> Option<TrayAction> {
    use rune::tray::TrayEvent;
    match event {
        TrayEvent::Activate { .. } => Some(TrayAction::Show),
        TrayEvent::Menu(id) if id == MENU_SHOW => Some(TrayAction::Show),
        TrayEvent::Menu(id) if id == MENU_QUIT => Some(TrayAction::Quit),
        _ => None,
    }
}

/// The window and the tray icon, as the flow needs them. Calls come from the UI thread and must not block it.
pub trait Surface: Send + Sync {
    /// Shows the window and takes focus (`AppHandle::open`).
    fn show_window(&self);
    /// Puts the icon on the tray with this many working sessions in its tooltip; if it can't be had the app says so with
    /// `State::tray_failed` and the window comes back.
    fn show_tray(&self, working: usize);
    fn hide_tray(&self);
}

/// Asks the Fleet at an address how many sessions are working.
pub type Status = Arc<dyn Fn(&str) -> usize + Send + Sync>;

pub struct CloseState {
    /// Set by the app once it has a window handle; without it (scenes, tests that don't need it) nothing is shown or hidden.
    pub surface: Option<Arc<dyn Surface>>,
    pub post: Option<Post>,
    pub status: Status,
    /// The quit dialog is open, for this many working sessions.
    pub quit_dialog: Option<usize>,
    /// The window is closed to the tray, with this many working when it went.
    pub in_tray: Option<usize>,
    /// A question to the Fleet is out; a second close or quit waits for its answer.
    pub asking: bool,
}

impl Default for CloseState {
    fn default() -> CloseState {
        CloseState {
            surface: None,
            post: None,
            status: Arc::new(crate::fleet::http::desktop_working),
            quit_dialog: None,
            in_tray: None,
            asking: false,
        }
    }
}

#[derive(Clone, Copy)]
enum Why {
    Close,
    Quit,
}

impl State {
    /// `App::on_close`: the window has been closed (the app keeps running; this decides whether it should).
    pub fn window_closed(&mut self, tx: &mut Tx) {
        // The window going because the app is quitting: nothing to decide.
        if rune::quitting() {
            return;
        }
        match on_close(System::current(), self.start.owned.is_some()) {
            Close::Quit => tx.quit(),
            Close::Hide => {}
            Close::Ask => self.ask_working(Why::Close, tx),
        }
    }

    /// "Quit Fleet" of the tray, and the menu's and the palette's (piece H): a Fleet the app started and sessions working ask
    /// first, with the window shown; otherwise the app quits.
    pub fn quit_requested(&mut self, tx: &mut Tx) {
        if self.start.owned.is_some() {
            self.ask_working(Why::Quit, tx);
        } else {
            tx.quit();
        }
    }

    fn ask_working(&mut self, why: Why, tx: &mut Tx) {
        let (Some(base), Some(post)) = (self.link.as_ref().map(|l| l.base.clone()), self.close.post.clone()) else {
            // Nothing to ask (no link yet, no way to hand an answer back): as when the Fleet can't be asked, no sessions.
            return self.working_answer(why, 0, tx);
        };
        if self.close.asking {
            return;
        }
        self.close.asking = true;
        let status = self.close.status.clone();
        let spawned = std::thread::Builder::new().name("fleet-status".into()).spawn({
            let post = post.clone();
            move || {
                let working = status(&base);
                post.update(move |s, tx| {
                    s.close.asking = false;
                    s.working_answer(why, working, tx)
                });
            }
        });
        if spawned.is_err() {
            self.close.asking = false;
            self.working_answer(why, 0, tx);
        }
    }

    fn working_answer(&mut self, why: Why, working: usize, tx: &mut Tx) {
        match why {
            Why::Close => match after_status(working) {
                Outcome::Tray(n) => self.hide_to_tray(n),
                _ => tx.quit(),
            },
            Why::Quit if working == 0 => tx.quit(),
            Why::Quit => {
                // The question needs the window, which may be closed to the tray.
                self.close.quit_dialog = Some(working);
                if let Some(surface) = &self.close.surface {
                    surface.show_window();
                }
            }
        }
    }

    /// The window is gone and the Fleet keeps working: the tray icon is the way back.
    pub fn hide_to_tray(&mut self, working: usize) {
        self.close.in_tray = Some(working);
        self.post_still_running(working);
        if let Some(surface) = &self.close.surface {
            surface.show_tray(working);
        }
    }

    /// The tray couldn't be had (no D-Bus session): a window that can't come back must not go, so it comes back now.
    pub fn tray_failed(&mut self, why: &str) {
        eprintln!("fleet-native: no tray icon ({why}); keeping the window");
        self.close.in_tray = None;
        if let Some(surface) = &self.close.surface {
            surface.show_window();
        }
    }

    pub fn tray_event(&mut self, event: &rune::tray::TrayEvent, tx: &mut Tx) {
        match tray_action(event) {
            Some(TrayAction::Show) => self.show_from_tray(),
            Some(TrayAction::Quit) => self.quit_requested(tx),
            None => {}
        }
    }

    pub fn show_from_tray(&mut self) {
        if let Some(surface) = &self.close.surface {
            surface.show_window();
        }
        self.window_visible();
    }

    /// The window is on screen again, however it came back: the tray icon goes (`showWindow`).
    pub fn window_visible(&mut self) {
        if self.close.in_tray.take().is_some()
            && let Some(surface) = &self.close.surface
        {
            surface.hide_tray();
        }
    }

    /// The dialog's Quit Fleet: the app quits, and stops its Fleet as it goes (`App::on_quit`).
    pub fn quit_confirm(&mut self, tx: &mut Tx) {
        if self.close.quit_dialog.take().is_some() {
            tx.quit();
        }
    }

    /// The scenes' picture of the question.
    pub fn show_quit_dialog(&mut self, dark: bool, working: usize) {
        self.dark = Some(dark);
        self.close.quit_dialog = Some(working);
    }

    pub fn quit_cancel(&mut self) {
        self.close.quit_dialog = None;
    }
}
