//! The page tab's body (component 14, piece A): the page area of `components/canvas/PageCanvas.vue` (`.page-canvas__page`
//! and its frame), where the web view is laid over the panel. The bar above it, the warnings and the choice bar are piece C's.
//!
//! In a real window on a system that hosts a web view, Rune draws the frame's white and reports where it is; the web view
//! covers it (`web::platform`). Anywhere else (headless runs, scenes, a Wayland window, a build without the `web` feature) the
//! body is the fallback panel (decision 9).
use crate::State;
use crate::fleet::web_page::{ShownPage, page_url};
use crate::icons::*;
use crate::theme::{Theme, sans};
use crate::ui::button::{Size, Variant, button, disabled};
use crate::web::wanted::page_base;
use rune::{El, col, icon, text};

pub const FALLBACK_TEXT: &str = "This page can't be shown inside the app on this system.";

pub fn body(s: &State, t: &Theme, page: &ShownPage) -> El {
    let url = page_base(s).and_then(|base| page_url(&base, page));
    if s.web.can_show() && url.is_some() {
        // `.page-canvas__page { background: var(--panel-bg) }` holding the frame, white until the page draws (row 14.10).
        return col().key("page-tab-page").w_full().grow(1.0).min_h(0.0).fill(t.panel_bg).child(
            col()
                .key("page-tab-frame")
                .w_full()
                .grow(1.0)
                .min_h(0.0)
                .fill(t.page_frame())
                .on_layout(|s: &mut State, r| s.web.rect = Some(r))
                .on_screen_layout(|s: &mut State, r| s.web.screen = Some(r)),
        );
    }
    fallback(t, url)
}

/// The page can't be shown here: say so, and offer the page in the system's browser.
fn fallback(t: &Theme, url: Option<String>) -> El {
    let open = button(t, Variant::Outline, Size::Sm, Some(&EXTERNAL_LINK), "Open in browser").key("page-tab-open-browser");
    let open = match url {
        Some(url) => open.on_press_tx(move |_: &mut State, tx| {
            let _ = tx.open_url(&url);
        }),
        None => disabled(open),
    };
    col()
        .key("page-tab-fallback")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .fill(t.panel_bg)
        .center()
        .gap(12.0)
        .padding(24.0)
        .child(icon(&APP_WINDOW, 24.0).color(t.muted))
        .child(text(FALLBACK_TEXT).font(sans(13.0, 400.0, 1.5)).color(t.text).wrap().align(rune::Align::Center))
        .child(open)
}
