//! The web view host (component 14): the system's own web view (WebView2, WKWebView, WebKitGTK, through `wry`) as a child
//! of the window, laid over a page tab's body (`docs/PORT.md` decision 1, `docs/research/web-content.md`).
//!
//! A native view is on top of everything Rune draws, so the state decides where it is (`wanted`) and the host only follows:
//! every frame `State::keep_web` compares what is wanted with what was last done, and moves, loads or hides the views when
//! they differ. Headless runs, scenes and builds without the `web` feature never get a window handle, so they draw the
//! fallback panel instead (decision 9).
pub mod wanted;

#[cfg(all(feature = "web", not(target_arch = "wasm32")))]
pub mod demo;
#[cfg(all(feature = "web", not(target_arch = "wasm32")))]
pub mod platform;

pub use wanted::{Placement, Wanted, wanted};

/// What the state knows of the web views. The views themselves are native and live on the UI thread (`platform`).
#[derive(Debug, Default)]
pub struct Host {
    /// The page area's rectangle in window points, from its `on_layout`.
    pub rect: Option<[f32; 4]>,
    /// The same in screen points (the CI walk clicks at it).
    pub screen: Option<[f32; 4]>,
    /// The window handed over its native handle and the system's side started (GTK, COM).
    pub available: bool,
    /// Why no web view could be made (a Wayland window, no WebView2 runtime); the fallback shows from then on.
    pub failed: Option<String>,
    /// With no server: where the invented pages are served (the CI walk's local server).
    pub mock_base: Option<String>,
    /// A click in the page gave it the keyboard (it told us over IPC); Rune has none until one of its elements takes it.
    pub page_has_keys: bool,
    /// What `keep_web` last did, so an unchanged frame touches nothing native.
    pub applied: Option<Wanted>,
    /// The CI walk, while it runs (`demo`).
    #[cfg(all(feature = "web", not(target_arch = "wasm32")))]
    pub walk: Option<demo::Walk>,
}

impl Host {
    /// A web view can be shown here: a real window on a system that hosts one.
    pub fn can_show(&self) -> bool {
        self.available && self.failed.is_none()
    }
}

/// The script every page gets at document start: it tells the app when the page takes the keyboard. Piece B adds the
/// page bridge (decision 3) beside it.
pub const HOST_SCRIPT: &str = r#"(function () {
  if (window.__fleetHost) return;
  window.__fleetHost = true;
  var send = function (kind) { try { window.ipc.postMessage(JSON.stringify({ host: kind })); } catch (e) {} };
  window.addEventListener("focus", function () { send("focus"); }, true);
  window.addEventListener("DOMContentLoaded", function () { send("loaded"); });
})();"#;

impl crate::State {
    /// From `poll`, every frame: the native views follow `wanted`. True when the view must draw again (a web view
    /// couldn't be made, so the fallback shows).
    pub fn keep_web(&mut self, tx: &mut rune::Tx) -> bool {
        #[cfg(all(feature = "web", not(target_arch = "wasm32")))]
        return platform::keep(self, tx);
        #[cfg(not(all(feature = "web", not(target_arch = "wasm32"))))]
        {
            let _ = tx;
            false
        }
    }
}
