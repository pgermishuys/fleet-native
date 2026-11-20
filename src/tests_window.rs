//! Component 24, piece G: the window's place goes through the app's state and its settings file.
use crate::State;
use crate::desktop::window::{Frame, Seen};
use crate::fleet::settings::Sidebar;

fn normal(x: i32, y: i32, w: u32, h: u32) -> Seen {
    Seen { position: (x, y), size: (w, h), scale: 1.0, maximized: false, minimized: false, full_screen: false, restore: None }
}

#[test]
fn a_window_that_stops_moving_is_kept_in_the_settings() {
    let mut s = State::default();
    assert!(s.sidebar.window().is_none());
    assert!(!s.keep_window_seen(&normal(40, 50, 1200, 800), 0.0));
    assert!(!s.keep_window_seen(&normal(40, 50, 1200, 800), 399.0));
    assert!(s.keep_window_seen(&normal(40, 50, 1200, 800), 400.0));
    let saved = s.sidebar.window().expect("kept");
    assert_eq!((saved.position, saved.size, saved.maximized), ((40, 50), (1200, 800), false));
}

#[test]
fn the_settings_file_brings_the_window_back() {
    let mut s = Sidebar::default();
    let frame = Frame { position: (-20, 30), size: (1300, 850), scale: 2.0, maximized: true };
    s.save_window(frame);
    let json = s.to_json();
    assert_eq!(Sidebar::parse(&json).window(), Some(frame));
    // Another run's file with nothing for the window, or garbage there, opens at the default.
    assert_eq!(Sidebar::parse("{}").window(), None);
    assert_eq!(Sidebar::parse(r#"{"window": {"x": "left"}}"#).window(), None);
}

#[test]
fn closing_writes_the_file_at_once() {
    let dir = std::env::temp_dir().join(format!("fleet-native-window-{}", std::process::id()));
    let mut s = State::default();
    s.sidebar.adopt(dir.clone(), Sidebar::default());
    // Seen a moment ago and not yet 400 ms old.
    assert!(!s.keep_window_seen(&normal(5, 6, 1000, 700), 0.0));
    s.flush_window();
    // Headless there is no window to read, so the flush wrote what was pending.
    let on_disk = crate::fleet::settings::load(&dir).window().expect("written");
    assert_eq!((on_disk.position, on_disk.size), ((5, 6), (1000, 700)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_window_is_titled_fleet() {
    assert_eq!(crate::spec().title, "Fleet");
}
