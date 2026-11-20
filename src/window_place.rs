//! The window's place, kept between runs (component 24, piece G; `desktop::window` has the rules). Rune tells the app nothing
//! when its window moves or resizes, so the poll reads `WindowHandle::placement()` and `desktop::window::Keeper` waits the
//! 400 ms Electron waits. The handle belongs to the UI thread, so it is kept in a thread-local, as `web::platform` does.
use crate::State;
use crate::desktop::window::{self, Seen};
use rune::WindowHandle;
use std::cell::RefCell;

thread_local! {
    static WINDOW: RefCell<Option<WindowHandle>> = const { RefCell::new(None) };
}

/// `App::on_window`: the native window exists (again, after a Wayland close or a GPU release).
pub fn on_window(window: &WindowHandle) {
    WINDOW.with(|w| *w.borrow_mut() = Some(window.clone()));
}

/// What the window says about itself now; `None` before it exists, after it closed, and on Wayland, which hides window
/// positions (row 24.58: nothing else in Rune reports the size, so nothing is kept there).
fn seen() -> Option<Seen> {
    let p = WINDOW.with(|w| w.borrow().as_ref().and_then(WindowHandle::placement))?;
    Some(Seen {
        position: p.position,
        size: p.content_size,
        scale: p.scale,
        maximized: p.maximized,
        minimized: p.minimized,
        full_screen: p.full_screen,
        restore: p.restore,
    })
}

impl State {
    /// Poll: saves the window's place once it has stayed put for 400 ms. True when a save was asked for (nothing on screen changes).
    pub fn keep_window(&mut self, now: f64, tx: &mut crate::Tx) -> bool {
        let now_ms = now * 1000.0;
        if !self.window_place.due(now_ms) {
            return false;
        }
        // An idle app is not polled, and a window moved by the person doesn't wake it: ask for the next look. Headless runs have
        // no window, so a scene or a scripted run asks for nothing and keeps its clock.
        let Some(seen) = seen() else { return false };
        tx.wake_after(std::time::Duration::from_millis(window::WAKE_EVERY_MS));
        self.keep_window_seen(&seen, now_ms)
    }

    pub(crate) fn keep_window_seen(&mut self, seen: &Seen, now_ms: f64) -> bool {
        let Some(frame) = self.window_place.observe(seen, now_ms) else { return false };
        self.sidebar.save_window(frame);
        self.save_settings();
        true
    }

    /// The window is closing or the app quitting: whatever was seen and not yet written goes now, on this thread, because the
    /// process may end before another one runs.
    pub fn flush_window(&mut self) {
        let Some(frame) = self.window_place.flush(seen().as_ref()) else { return };
        self.sidebar.save_window(frame);
        if let Some((dir, json)) = self.sidebar.take_save() {
            crate::fleet::settings::save_now(&dir, &json);
        }
    }
}
