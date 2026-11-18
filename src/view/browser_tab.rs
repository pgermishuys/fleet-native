//! The browser tab (component 14, piece H): `components/canvas/BrowserCanvas.vue`. The bar (Back, Forward, Reload, the address,
//! Open in a new tab) and the page area: the preview's web view laid over the panel, or what stands in for it (the panel when
//! there is no page yet, the error, another machine's note, the fallback where no web view can be made).
//!
//! The app's panel, strip and output drawer, and the port select, are `browser_app` (piece I). Not here yet: Your view / Agent's
//! view (piece J).
use crate::State;
use crate::fleet::browser_tab::BrowserPage;
use crate::icons::*;
use crate::theme::{RADIUS_BTN, Theme, mono, sans};
use crate::ui::button::{Size, Variant, button};
use crate::web::browser::ADDRESS_KEY;
use crate::web::browser_app::Panel;
use crate::web::preview_bridge::NavAction;
use rune::{El, Key, KeyEvent, Tx, col, row, text, text_input};

use super::browser_app;
use super::page_tab::{fallback, icon_button};

/// The whole tab (`.browser-canvas`).
pub fn browser_tab(s: &State, t: &Theme, tab: &str, page: &BrowserPage) -> El {
    let width_tab = tab.to_string();
    let mut all = col()
        .key("browser-tab")
        .w_full()
        .h_full()
        .min_h(0.0)
        // Fleet's container query reads this width (the strip's narrow forms).
        .on_layout(move |s: &mut State, r| {
            let ui = s.web.browsers.entry(width_tab.clone()).or_default();
            if ui.app.width != Some(r[2]) {
                ui.app.width = Some(r[2]);
            }
        })
        .child(bar(s, t, tab, page))
        .child(body(s, t, tab));
    if let Some(app) = browser_app::app_section(s, t, tab) {
        all = all.child(app);
    }
    all
}

/// `.browser-canvas__bar`: 40 px, padding 0 8, gap 2.
fn bar(s: &State, t: &Theme, tab: &str, page: &BrowserPage) -> El {
    let nav = |key: &str, tip: &str, glyph: &'static rune::Icon, action: NavAction| {
        let tab = tab.to_string();
        icon_button(t, key, tip, glyph).on_press(move |s: &mut State| s.browser_navigate(&tab, action))
    };
    let open_tab = tab.to_string();
    let open = icon_button(t, "browser-open-new", "Open in a new tab", &EXTERNAL_LINK).on_press_tx(move |s: &mut State, tx: &mut Tx| {
        if let Some(url) = s.browser_outside_url(&open_tab) {
            let _ = tx.open_url(&url);
        }
    });
    row()
        .key("browser-bar")
        .w_full()
        .h(40.0)
        .gap(2.0)
        .px(8.0)
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], t.border)
        .border_layout()
        .child(nav("browser-back", "Back", &ARROW_LEFT, NavAction::Back))
        .child(nav("browser-forward", "Forward", &ARROW_RIGHT, NavAction::Forward))
        .child(nav("browser-reload", "Reload", &ROTATE_CW, NavAction::Reload))
        .child(address(s, t, tab, page))
        .children(browser_app::port_select(s, t, tab))
        .child(open)
}

/// `.browser-canvas__address input`: 28 px, mono 12, the bar's own text; Enter shows it.
fn address(s: &State, t: &Theme, tab: &str, page: &BrowserPage) -> El {
    let text_now = s.browser_ui(tab).map_or_else(|| rune::TextEdit::new(&page.url), |u| u.address.clone());
    let (edit_tab, key_tab) = (tab.to_string(), tab.to_string());
    let placeholder = if page.app_id.is_some() { "Waiting for the app\u{2026}" } else { "localhost:5173" };
    let field = text_input(&text_now, mono(12.0, 400.0, 1.3))
        .key(ADDRESS_KEY)
        .label("Address")
        .placeholder(placeholder, t.muted)
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .single_line()
        .w_full()
        .h(28.0)
        .px(10.0)
        // The 28 px field less its border, less the line: the text sits in the middle.
        .py(5.2)
        .radius(RADIUS_BTN)
        .fill(t.text.alpha(0.04))
        .border(1.0, t.border)
        .border_layout()
        .focus_within_ring(1.0, -1.0, t.accent)
        .on_edit(move |s: &mut State| &mut s.web.browsers.entry(edit_tab.clone()).or_default().address)
        .on_input_key(move |s: &mut State, ev: &KeyEvent, _: &mut Tx| match ev.key {
            Key::Enter => {
                s.browser_submit(&key_tab);
                true
            }
            _ => false,
        });
    row().grow(1.0).shrink().min_w(0.0).mx(4.0).child(field)
}

/// `.browser-canvas__page`: `--panel-bg`, holding the web view's frame, a message, or the fallback.
fn body(s: &State, t: &Theme, tab: &str) -> El {
    let area = |inner: El| col().key("browser-tab-page").w_full().grow(1.0).min_h(0.0).fill(t.panel_bg).child(inner);
    if s.browser_elsewhere() {
        return area(elsewhere(s, t));
    }
    let ui = s.browser_ui(tab);
    if let Some(error) = ui.and_then(|u| u.error.as_ref()) {
        return area(
            text(error.clone()).key("browser-error").role("alert").font(sans(12.5, 400.0, 1.5)).color(t.error_text).wrap().padding(16.0),
        );
    }
    if let Some(p) = s.browser_panel(tab) {
        return area(browser_app::panel(s, t, tab, p, panel(t)));
    }
    let Some(frame_src) = ui.and_then(|u| u.frame_src.clone()) else {
        return area(browser_app::panel(s, t, tab, Panel::Waiting, panel(t)));
    };
    if s.web.can_show() {
        return area(
            col()
                .key("browser-tab-frame")
                .w_full()
                .grow(1.0)
                .min_h(0.0)
                .fill(t.page_frame())
                .on_layout(|s: &mut State, r| s.web.rect = Some(r))
                .on_screen_layout(|s: &mut State, r| s.web.screen = Some(r)),
        );
    }
    let outside = s.browser_outside_url(tab).or(Some(frame_src));
    area(fallback(t, outside))
}

/// `.browser-canvas__panel`: padding 28 20, gap 10, at most 560 wide, children at the start.
fn panel(_t: &Theme) -> El {
    col().key("browser-panel").max_w(560.0).items_start().gap(10.0).px(20.0).py(28.0)
}

/// This app runs on another machine: where its preview opens, or why it can't.
fn elsewhere(s: &State, t: &Theme) -> El {
    let (name, there) = s.browser_machine();
    let mut all = panel(t).key("browser-elsewhere").child(
        text(format!("This app runs on {name}. Its preview opens there."))
            .key("browser-elsewhere-title")
            .font(sans(13.0, 500.0, 1.5))
            .color(t.text)
            .wrap(),
    );
    all = match there {
        Some(url) => all.child(
            button(t, Variant::Default, Size::Sm, Some(&EXTERNAL_LINK), &format!("Open on {name}")).key("browser-open-there").on_press_tx(
                move |_: &mut State, tx: &mut Tx| {
                    let _ = tx.open_url(&url);
                },
            ),
        ),
        None => all.child(
            text(format!("{name} has no web app of its own, so its previews can't open from here yet."))
                .key("browser-elsewhere-note")
                .font(sans(12.5, 400.0, 1.5))
                .color(t.muted)
                .wrap(),
        ),
    };
    all
}
