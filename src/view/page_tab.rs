//! The page tab (component 14, pieces A and C): `components/canvas/PageCanvas.vue`. The bar (Reload, the source, Open in a new
//! tab), the warnings, the page area where the web view is laid over the panel (`.page-canvas__page` and its frame), and the
//! choice bar (`page_choice.rs`).
//!
//! In a real window on a system that hosts a web view, Rune draws the frame's white and reports where it is; the web view
//! covers it (`web::platform`). Anywhere else (headless runs, scenes, a Wayland window, a build without the `web` feature) the
//! body is the fallback panel (decision 9).
use crate::State;
use crate::fleet::web_page::{ShownPage, page_url};
use crate::icons::*;
use crate::theme::{RADIUS_BTN, Theme, mono, sans};
use crate::ui::button::{Size, Variant, button, disabled};
use crate::web::wanted::page_base;
use rune::{Cursor, El, col, el, icon, row, text};

pub const FALLBACK_TEXT: &str = "This page can't be shown inside the app on this system.";

/// The whole tab: bar, warnings, the page, the choice bar (`.page-canvas`).
pub fn page_tab(s: &State, t: &Theme, tab: &str, page: &ShownPage) -> El {
    let url = page_base(s).and_then(|base| page_url(&base, page));
    let mut all = col().key("page-tab").w_full().h_full().min_h(0.0).child(bar(t, tab, page, url.clone()));
    if !page.warnings.is_empty() {
        all = all.child(warnings(t, &page.warnings));
    }
    all = all.child(body(s, t, url));
    match super::page_choice::choice_bar(s, t, tab, page) {
        Some(bar) => all.child(bar),
        None => all,
    }
}

/// `.page-canvas__icon`: 28 square, `--muted`, `--accent-dim` and `--text` on hover.
pub(super) fn icon_button(t: &Theme, key: &str, tip: &str, glyph: &'static rune::Icon) -> El {
    el().key(key.to_string())
        .role("button")
        .label(tip.to_string())
        .help(tip.to_string())
        .size(28.0, 28.0)
        .min_w(28.0)
        .center()
        .radius(RADIUS_BTN)
        .hover(t.accent_dim)
        .hover_group()
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, -2.0, t.accent)
        .child(icon(glyph, 14.0).color(t.muted).group_color(t.text))
}

/// `.page-canvas__bar`: Reload, where the page came from, and Open in a new tab.
fn bar(t: &Theme, tab: &str, page: &ShownPage, url: Option<String>) -> El {
    let (reload_tab, open_tab) = (tab.to_string(), tab.to_string());
    let reload = icon_button(t, "page-tab-reload", "Reload", &ROTATE_CW).on_press(move |s: &mut State| s.reload_page(&reload_tab));
    let open = icon_button(t, "page-tab-open-new", "Open in a new tab", &EXTERNAL_LINK);
    let open = match url {
        Some(_) => open.on_press_tx(move |s: &mut State, tx| {
            if let Some(url) = s.page_outside_url(&open_tab) {
                let _ = tx.open_url(&url);
            }
        }),
        None => open,
    };
    row()
        .key("page-tab-bar")
        .w_full()
        .h(40.0)
        .gap(2.0)
        .px(8.0)
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], t.border)
        .border_layout()
        .child(reload)
        .child(source(t, page))
        .child(open)
}

/// `.page-canvas__source`: the label alone, else the file name and the whole source after it; the tooltip is what's shown.
fn source(t: &Theme, page: &ShownPage) -> El {
    let file = |words: &str| text(words.to_string()).key("page-tab-file").font(sans(12.0, 500.0, 1.4)).color(t.text);
    let tip = page.label.clone().unwrap_or_else(|| page.source.clone());
    let mut r = row().key("page-tab-source").grow(1.0).shrink().min_w(0.0).mx(4.0).gap(8.0).items_center().help(tip);
    r = match &page.label {
        Some(label) => r.child(file(label)),
        None => r.child(file(page.file_name())).child(
            text(page.source.clone()).key("page-tab-path").font(mono(11.5, 400.0, 1.4)).color(t.muted).truncate().shrink().min_w(0.0),
        ),
    };
    r
}

/// `.page-canvas__warnings`: links Fleet couldn't copy, over an amber wash.
fn warnings(t: &Theme, all: &[String]) -> El {
    let mut list = col()
        .key("page-tab-warnings")
        .role("list")
        .label("Links that won't load")
        .w_full()
        .gap(4.0)
        .px(12.0)
        .py(8.0)
        .fill(t.status_waiting().alpha(0.08))
        .border_sides([0.0, 0.0, 1.0, 0.0], t.border)
        .border_layout();
    for (i, warning) in all.iter().enumerate() {
        list = list.child(
            row()
                .key(format!("page-tab-warning-{i}"))
                .gap(6.0)
                .child(icon(&TRIANGLE_ALERT, 13.0).color(t.status_waiting()).mt(2.0))
                .child(text(warning.clone()).font(sans(12.0, 400.0, 1.4)).color(t.text).wrap().grow(1.0).shrink().min_w(0.0)),
        );
    }
    list
}

/// The page area: `--panel-bg` holding the frame the web view covers, or the fallback panel.
fn body(s: &State, t: &Theme, url: Option<String>) -> El {
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
pub(super) fn fallback(t: &Theme, url: Option<String>) -> El {
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
