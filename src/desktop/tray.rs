//! The tray icon (component 24, piece E): `hideToTray` in `desktop/src/main.ts`. Rune's tray runs on a thread of its own
//! (StatusNotifierItem over D-Bus on Linux, `Shell_NotifyIcon` on Windows; macOS has none and never needs one, closing
//! hides to the Dock). Making it talks to the bus, so it happens on a thread, and a failure comes back as a job.
use super::close::{MENU_QUIT, MENU_SHOW, Surface, tooltip};
use crate::State;
use crate::fleet::link::Post;
use rune::tray::{Tray, TrayIcon, TrayItem, TraySpec};
use rune::{AppHandle, Tx};
use std::sync::{Arc, Mutex};

/// `desktop/static/tray.png`, 32 x 32.
const ICON: &[u8] = include_bytes!("../../assets/tray.png");

/// The picture's pixels as straight RGBA rows, which is what the tray takes.
pub fn decode_icon(bytes: &[u8]) -> Result<TrayIcon, String> {
    let mut reader = png::Decoder::new(std::io::Cursor::new(bytes)).read_info().map_err(|e| e.to_string())?;
    let mut pixels = vec![0; reader.output_buffer_size().ok_or("the icon is too large")?];
    let info = reader.next_frame(&mut pixels).map_err(|e| e.to_string())?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return Err("the tray icon must be 8-bit RGBA".into());
    }
    pixels.truncate(info.buffer_size());
    Ok(TrayIcon::rgba(info.width, info.height, pixels))
}

pub fn menu() -> Vec<TrayItem> {
    vec![TrayItem::action(MENU_SHOW, "Show Fleet"), TrayItem::Separator, TrayItem::action(MENU_QUIT, "Quit Fleet")]
}

fn spec(working: usize) -> Result<TraySpec, String> {
    Ok(TraySpec::new("com.pgermishuys.fleet-native", "Fleet", decode_icon(ICON)?).tooltip(tooltip(working)).menu(menu()))
}

#[derive(Default)]
struct Cell {
    /// Whether the icon is wanted now; a tray that was being made when the window came back is dropped as it is finished.
    wanted: bool,
    tray: Option<Tray>,
}

/// The app's window and tray, through its handle.
pub struct Real {
    handle: AppHandle<State>,
    post: Post,
    cell: Arc<Mutex<Cell>>,
}

impl Real {
    pub fn new(handle: AppHandle<State>, post: Post) -> Real {
        Real { handle, post, cell: Arc::default() }
    }
}

impl Surface for Real {
    fn show_window(&self) {
        self.handle.open();
    }

    fn show_tray(&self, working: usize) {
        self.cell.lock().unwrap_or_else(|e| e.into_inner()).wanted = true;
        let (cell, post) = (self.cell.clone(), self.post.clone());
        let started = std::thread::Builder::new().name("fleet-tray".into()).spawn(move || {
            let mut cell = cell.lock().unwrap_or_else(|e| e.into_inner());
            if !cell.wanted {
                return;
            }
            if let Some(tray) = &cell.tray {
                return tray.set_tooltip(tooltip(working));
            }
            let made = spec(working).and_then(|spec| {
                let post = post.clone();
                Tray::new(spec, move |event| {
                    // On the tray's thread: only hand the click to the app.
                    post.update(move |s: &mut State, tx: &mut Tx| s.tray_event(&event, tx));
                })
                .map_err(|e| e.to_string())
            });
            match made {
                Ok(tray) => cell.tray = Some(tray),
                Err(why) => {
                    cell.wanted = false;
                    post.update(move |s, _| s.tray_failed(&why));
                }
            }
        });
        if let Err(e) = started {
            let why = e.to_string();
            self.post.update(move |s, _| s.tray_failed(&why));
        }
    }

    fn hide_tray(&self) {
        let mut cell = self.cell.lock().unwrap_or_else(|e| e.into_inner());
        cell.wanted = false;
        let gone = cell.tray.take();
        drop(cell);
        // Dropping closes the bus connection; not on the UI thread.
        if gone.is_some() {
            let _ = std::thread::Builder::new().name("fleet-tray-drop".into()).spawn(move || drop(gone));
        }
    }
}
