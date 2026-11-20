//! The window's place and the one-instance rule (component 24, piece G; weave-fleet `desktop/src/main.ts` `createWindow`
//! and `saveBounds`, `settings.ts` `restorableBounds`). Plain Rust with no Rune: the app hands in what `placement()` and the
//! displays say, and gets back what to save and how to open.
//!
//! Electron tells the app when the window moves or resizes and saves 400 ms later. Rune has no such event, so the app reads
//! `placement()` on its poll and `Keeper` does the 400 ms wait. Everything is in physical pixels, Rune's own space for a
//! window's position and size.
use serde_json::{Value, json};

/// The id the app claims its one instance with (decision 10). Not Electron's `com.pgermishuys.fleet`, so Fleet's own app and
/// this one can run together.
pub const INSTANCE_ID: &str = "com.pgermishuys.fleet-native";

/// Electron's wait between the last move or resize and the write (`saveBounds`).
pub const SAVE_AFTER_MS: f64 = 400.0;

/// What Fleet's `restorableBounds` asks of a saved window: at least 400 x 300 points, and at least 100 x 100 of it on a screen.
const MIN_WIDTH: f64 = 400.0;
const MIN_HEIGHT: f64 = 300.0;
const MIN_ON_SCREEN: i64 = 100;

/// A window as it was left: the frame's top-left, the content's size, and whether it was maximised (then the frame is the one
/// restoring it returns to).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub position: (i32, i32),
    pub size: (u32, u32),
    /// Physical pixels per point where it was saved, to read "at least 400 x 300" in points.
    pub scale: f64,
    pub maximized: bool,
}

/// A display's position and size, the pair `WindowPlacement::display` gives.
pub type Screen = ((i32, i32), (u32, u32));

impl Frame {
    pub fn to_json(self) -> Value {
        json!({ "x": self.position.0, "y": self.position.1, "width": self.size.0, "height": self.size.1, "scale": self.scale, "maximized": self.maximized })
    }

    /// Reads what `to_json` wrote; anything missing or of the wrong kind is no saved window.
    pub fn from_json(v: &Value) -> Option<Frame> {
        let int = |k: &str| v.get(k)?.as_i64();
        let (x, y) = (i32::try_from(int("x")?).ok()?, i32::try_from(int("y")?).ok()?);
        let (w, h) = (u32::try_from(int("width")?).ok()?, u32::try_from(int("height")?).ok()?);
        let scale = v.get("scale").and_then(Value::as_f64).filter(|s| *s >= 0.5 && *s <= 8.0).unwrap_or(1.0);
        Some(Frame { position: (x, y), size: (w, h), scale, maximized: v.get("maximized") == Some(&Value::Bool(true)) })
    }
}

/// The saved window if it still fits a screen, else `None` so the window opens centred at its default size
/// (`restorableBounds`: DT "window bounds").
pub fn restorable(saved: Option<Frame>, screens: &[Screen]) -> Option<Frame> {
    let f = saved?;
    if f64::from(f.size.0) / f.scale < MIN_WIDTH || f64::from(f.size.1) / f.scale < MIN_HEIGHT {
        return None;
    }
    let (x, y, w, h) = (i64::from(f.position.0), i64::from(f.position.1), i64::from(f.size.0), i64::from(f.size.1));
    let visible = screens.iter().any(|&((sx, sy), (sw, sh))| {
        let (sx, sy) = (i64::from(sx), i64::from(sy));
        let over_x = (x + w).min(sx + i64::from(sw)) - x.max(sx);
        let over_y = (y + h).min(sy + i64::from(sh)) - y.max(sy);
        over_x >= MIN_ON_SCREEN && over_y >= MIN_ON_SCREEN
    });
    visible.then_some(f)
}

/// How the first window opens.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Opening {
    pub position: Option<(i32, i32)>,
    pub size: Option<(u32, u32)>,
    pub maximized: bool,
}

/// The saved window checked against the screens now. A window saved maximised opens maximised; the frame stays what restoring
/// it returns to.
pub fn opening(saved: Option<Frame>, screens: &[Screen]) -> Opening {
    match restorable(saved, screens) {
        Some(f) => Opening { position: Some(f.position), size: Some(f.size), maximized: f.maximized },
        None => Opening::default(),
    }
}

/// What the window reports about itself, from `WindowPlacement`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Seen {
    pub position: (i32, i32),
    pub size: (u32, u32),
    pub scale: f64,
    pub maximized: bool,
    pub minimized: bool,
    pub full_screen: bool,
    /// The system's own record of the frame restoring returns to while maximised (Windows only).
    pub restore: Option<((i32, i32), (u32, u32))>,
}

/// Decides when to write. `saveBounds` doesn't save while minimised or full screen, and saves the normal bounds with the
/// maximised flag when maximised: where the system keeps no record of the normal frame (Linux, macOS), this remembers the last
/// one it saw before the window grew.
#[derive(Debug, Default)]
pub struct Keeper {
    /// The frame that is on disk, or was asked to be.
    kept: Option<Frame>,
    /// A frame seen but not yet 400 ms old, and when it last changed.
    pending: Option<(Frame, f64)>,
    /// The last frame seen while the window was normal.
    normal: Option<Frame>,
    /// When the window was last read, so the poll asks `placement()` a few times a second, not every frame.
    looked: Option<f64>,
}

/// How often the poll reads the window (Electron fires per event; this is a fifth of a second late at most).
const LOOK_EVERY_MS: f64 = 200.0;
/// The poll asks to be woken this often while a window exists, because nothing else wakes an idle app when its window moves.
pub const WAKE_EVERY_MS: u64 = 250;

impl Keeper {
    /// Starts from the frame that was saved, so a window that opens where it was saved writes nothing.
    pub fn new(saved: Option<Frame>) -> Keeper {
        Keeper { kept: saved, pending: None, looked: None, normal: saved.map(|f| Frame { maximized: false, ..f }) }
    }

    /// True when it is time to read the window again.
    pub fn due(&mut self, now_ms: f64) -> bool {
        if self.looked.is_some_and(|at| now_ms - at < LOOK_EVERY_MS) {
            return false;
        }
        self.looked = Some(now_ms);
        true
    }

    fn frame_of(&self, seen: &Seen) -> Option<Frame> {
        if seen.minimized || seen.full_screen {
            return None;
        }
        if !seen.maximized {
            return Some(Frame { position: seen.position, size: seen.size, scale: seen.scale, maximized: false });
        }
        let (position, size) = seen.restore.or_else(|| self.normal.map(|f| (f.position, f.size)))?;
        Some(Frame { position, size, scale: seen.scale, maximized: true })
    }

    /// Looks at the window at `now_ms`; returns the frame to write once it has stayed the same for `SAVE_AFTER_MS`.
    pub fn observe(&mut self, seen: &Seen, now_ms: f64) -> Option<Frame> {
        if !seen.maximized && !seen.minimized && !seen.full_screen {
            self.normal = Some(Frame { position: seen.position, size: seen.size, scale: seen.scale, maximized: false });
        }
        let frame = self.frame_of(seen)?;
        if self.pending.as_ref().map(|(f, _)| *f) != Some(frame) {
            self.pending = (self.kept != Some(frame)).then_some((frame, now_ms));
            return None;
        }
        let (frame, since) = self.pending?;
        if now_ms - since < SAVE_AFTER_MS {
            return None;
        }
        self.pending = None;
        self.kept = Some(frame);
        Some(frame)
    }

    /// The window is closing: whatever was seen and not yet written is written now, because nothing will look again.
    pub fn flush(&mut self, seen: Option<&Seen>) -> Option<Frame> {
        let latest = seen.and_then(|s| self.frame_of(s)).or(self.pending.map(|(f, _)| f))?;
        self.pending = None;
        (self.kept != Some(latest)).then(|| {
            self.kept = Some(latest);
            latest
        })
    }
}

/// Whether this run claims the one instance (decision 10): a normal start only. Headless runs, `--demo`, `--server` and a
/// scratch data folder (`FLEET_NATIVE_DATA_DIR`) run beside others, because scratch runs and CI must run side by side.
pub fn single_instance_wanted(headless: bool, demo: bool, explicit_server: bool, get: &dyn Fn(&str) -> Option<String>) -> bool {
    !headless && !demo && !explicit_server && get("FLEET_NATIVE_DATA_DIR").is_none_or(|v| v.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREENS: [Screen; 1] = [((0, 0), (1920, 1080))];

    fn frame(x: i32, y: i32, w: u32, h: u32) -> Frame {
        Frame { position: (x, y), size: (w, h), scale: 1.0, maximized: false }
    }

    fn seen(f: Frame) -> Seen {
        Seen {
            position: f.position,
            size: f.size,
            scale: f.scale,
            maximized: f.maximized,
            minimized: false,
            full_screen: false,
            restore: None,
        }
    }

    // DT "window bounds", three tests under their names.
    #[test]
    fn restores_bounds_that_are_still_on_a_screen() {
        let saved = frame(100, 100, 1200, 800);
        assert_eq!(restorable(Some(saved), &SCREENS), Some(saved));
    }

    #[test]
    fn drops_bounds_on_a_monitor_thats_gone() {
        assert_eq!(restorable(Some(frame(2500, 100, 1200, 800)), &SCREENS), None);
    }

    #[test]
    fn drops_bounds_too_small_to_use() {
        assert_eq!(restorable(Some(frame(0, 0, 200, 100)), &SCREENS), None);
    }

    #[test]
    fn nothing_saved_opens_at_the_default() {
        assert_eq!(opening(None, &SCREENS), Opening::default());
    }

    #[test]
    fn needs_a_hundred_by_a_hundred_on_a_screen() {
        // 99 across of a 1200-wide window is on the screen: not enough. 100 is.
        assert_eq!(restorable(Some(frame(1821, 100, 1200, 800)), &SCREENS), None);
        assert!(restorable(Some(frame(1820, 100, 1200, 800)), &SCREENS).is_some());
        assert_eq!(restorable(Some(frame(100, 981, 1200, 800)), &SCREENS), None);
        assert!(restorable(Some(frame(100, 980, 1200, 800)), &SCREENS).is_some());
    }

    #[test]
    fn a_second_screen_still_there_keeps_the_frame() {
        let two = [((0, 0), (1920, 1080)), ((1920, 0), (1920, 1080))];
        let on_second = frame(2500, 100, 1200, 800);
        assert_eq!(restorable(Some(on_second), &two), Some(on_second));
        // Left of the first screen, as a monitor placed on the left would be.
        let left = [((-1920, 0), (1920, 1080)), ((0, 0), (1920, 1080))];
        let on_left = frame(-1500, 100, 1200, 800);
        assert_eq!(restorable(Some(on_left), &left), Some(on_left));
        assert_eq!(restorable(Some(on_left), &SCREENS), None);
    }

    #[test]
    fn the_smallest_size_is_read_in_points() {
        // 600 x 500 physical on a 2x display is 300 x 250 points: too small. On a 1x display it is fine.
        let small = Frame { scale: 2.0, ..frame(0, 0, 600, 500) };
        assert_eq!(restorable(Some(small), &SCREENS), None);
        assert!(restorable(Some(Frame { scale: 1.0, ..small }), &SCREENS).is_some());
    }

    #[test]
    fn a_saved_maximized_window_opens_maximized() {
        let saved = Frame { maximized: true, ..frame(100, 100, 1200, 800) };
        let o = opening(Some(saved), &SCREENS);
        assert!(o.maximized);
        assert_eq!((o.position, o.size), (Some((100, 100)), Some((1200, 800))));
        assert!(!opening(Some(frame(100, 100, 1200, 800)), &SCREENS).maximized);
        // On a screen that is gone it opens at the default, not maximised somewhere unseen.
        assert_eq!(opening(Some(Frame { maximized: true, ..frame(5000, 0, 1200, 800) }), &SCREENS), Opening::default());
    }

    #[test]
    fn the_file_round_trips_and_ignores_what_is_not_a_frame() {
        let f = Frame { scale: 1.5, maximized: true, ..frame(-40, 12, 1360, 880) };
        assert_eq!(Frame::from_json(&f.to_json()), Some(f));
        assert_eq!(Frame::from_json(&json!({ "x": 1, "y": 2, "width": "wide", "height": 3 })), None);
        assert_eq!(Frame::from_json(&json!({ "x": 1, "y": 2, "width": -5, "height": 3 })), None);
        assert_eq!(Frame::from_json(&json!("window")), None);
        // A scale outside what a display has reads as 1.
        assert_eq!(Frame::from_json(&json!({ "x": 0, "y": 0, "width": 800, "height": 600, "scale": 99 })).map(|f| f.scale), Some(1.0));
    }

    #[test]
    fn a_move_is_saved_400_ms_after_it_stops() {
        let mut k = Keeper::new(Some(frame(0, 0, 1360, 880)));
        assert_eq!(k.observe(&seen(frame(0, 0, 1360, 880)), 0.0), None, "where it was saved, nothing to write");
        let moved = frame(50, 60, 1360, 880);
        assert_eq!(k.observe(&seen(moved), 1000.0), None);
        assert_eq!(k.observe(&seen(moved), 1399.0), None);
        // Still moving: the wait starts again.
        let more = frame(70, 60, 1360, 880);
        assert_eq!(k.observe(&seen(more), 1400.0), None);
        assert_eq!(k.observe(&seen(more), 1799.0), None);
        assert_eq!(k.observe(&seen(more), 1800.0), Some(more));
        assert_eq!(k.observe(&seen(more), 5000.0), None, "written once");
    }

    #[test]
    fn minimized_and_full_screen_windows_are_not_saved() {
        let mut k = Keeper::new(None);
        let f = frame(10, 10, 900, 700);
        for s in [Seen { minimized: true, ..seen(f) }, Seen { full_screen: true, ..seen(f) }] {
            assert_eq!(k.observe(&s, 0.0), None);
            assert_eq!(k.observe(&s, 9999.0), None);
        }
        assert_eq!(k.flush(Some(&Seen { minimized: true, ..seen(f) })), None);
    }

    #[test]
    fn maximizing_saves_the_normal_frame_with_the_flag() {
        let mut k = Keeper::new(None);
        let normal = frame(80, 90, 1100, 760);
        assert_eq!(k.observe(&seen(normal), 0.0), None);
        assert_eq!(k.observe(&seen(normal), 500.0), Some(normal));
        // Maximised: the window is now the whole screen, but the saved frame is the normal one.
        let big = Seen { position: (0, 0), size: (1920, 1080), maximized: true, ..seen(normal) };
        assert_eq!(k.observe(&big, 600.0), None);
        let saved = k.observe(&big, 1000.0).expect("the maximised state is written");
        assert!(saved.maximized);
        assert_eq!((saved.position, saved.size), (normal.position, normal.size));
        // Restoring it saves the unmaximised flag again.
        assert_eq!(k.observe(&seen(normal), 1100.0), None);
        assert_eq!(k.observe(&seen(normal), 1500.0), Some(normal));
    }

    #[test]
    fn windows_gives_the_restore_frame_and_it_wins() {
        let mut k = Keeper::new(None);
        let big =
            Seen { position: (0, 0), size: (1920, 1080), maximized: true, restore: Some(((5, 6), (1000, 700))), ..seen(frame(0, 0, 1, 1)) };
        assert_eq!(k.observe(&big, 0.0), None);
        let saved = k.observe(&big, 400.0).expect("saved");
        assert_eq!((saved.position, saved.size, saved.maximized), ((5, 6), (1000, 700), true));
    }

    #[test]
    fn a_window_launched_maximized_with_no_normal_frame_saves_nothing_yet() {
        // Nothing is known about the frame restoring would return to: no guess is written.
        let mut k = Keeper::new(None);
        let big = Seen { maximized: true, ..seen(frame(0, 0, 1920, 1080)) };
        assert_eq!(k.observe(&big, 0.0), None);
        assert_eq!(k.observe(&big, 9999.0), None);
    }

    #[test]
    fn closing_writes_what_was_seen_but_not_yet_old_enough() {
        let mut k = Keeper::new(Some(frame(0, 0, 1360, 880)));
        let moved = frame(30, 30, 1360, 880);
        assert_eq!(k.observe(&seen(moved), 100.0), None);
        assert_eq!(k.flush(None), Some(moved), "the pending frame goes even when the window can no longer be read");
        assert_eq!(k.flush(None), None);
        assert_eq!(k.flush(Some(&seen(moved))), None, "nothing new");
        let later = frame(31, 30, 1360, 880);
        assert_eq!(k.flush(Some(&seen(later))), Some(later));
    }

    #[test]
    fn the_window_is_read_a_few_times_a_second() {
        let mut k = Keeper::default();
        assert!(k.due(0.0));
        assert!(!k.due(199.0));
        assert!(k.due(200.0));
    }

    #[test]
    fn the_id_rule() {
        let none = |_: &str| None;
        assert!(single_instance_wanted(false, false, false, &none));
        assert!(!single_instance_wanted(true, false, false, &none), "headless");
        assert!(!single_instance_wanted(false, true, false, &none), "--demo");
        assert!(!single_instance_wanted(false, false, true, &none), "--server");
        let scratch = |k: &str| (k == "FLEET_NATIVE_DATA_DIR").then(|| "/scratch".to_string());
        assert!(!single_instance_wanted(false, false, false, &scratch), "a scratch data folder");
        let empty = |k: &str| (k == "FLEET_NATIVE_DATA_DIR").then(String::new);
        assert!(single_instance_wanted(false, false, false, &empty));
        assert_ne!(INSTANCE_ID, "com.pgermishuys.fleet", "Fleet's own app runs beside this one");
    }
}
