//! A page the agent showed in the conversation (component 14, piece F): `components/session/ConversationPage.vue`. A native web
//! view can't scroll or clip inside a Rune list, so the page is a card there (decision 2): its title, its file's name and the two
//! buttons Fleet's bar has. **Open full size** opens Fleet's dialog, here a Rune dialog whose body the web view covers.
use crate::State;
use crate::conv::message_list::ToolPage;
use crate::icons::*;
use crate::theme::{RADIUS_CARD, RADIUS_PANEL, Theme, mono, sans};
use crate::ui::dialog::lift;
use crate::ui::overlay::centred;
use crate::view::page_tab::{FALLBACK_TEXT, icon_button};
use crate::web::full::{CLOSE_KEY, Full};
use rune::{Color, El, Spring, Transition, Vec2, col, el, icon, row, text};

/// The card, `margin: 12px 0 4px`, named for the call that showed the page.
/// `contained` (a mod's page, component 22) has no new tab: it must stay inside the app.
pub fn card(s: &State, t: &Theme, call: &str, title: &str, page: &ToolPage, contained: bool) -> El {
    let key = format!("page-card-{call}");
    let name = page.path.rsplit('/').next().unwrap_or(&page.path).to_string();
    let file = if page.source.is_empty() { name } else { page.source.rsplit(['/', '\\']).next().unwrap_or(&page.source).to_string() };
    let words = col()
        .grow(1.0)
        .shrink()
        .min_w(0.0)
        .gap(2.0)
        .child(
            text(title.to_string())
                .key(format!("{key}-title"))
                .font(sans(13.0, 600.0, 1.4))
                .color(t.text)
                .truncate()
                .min_w(0.0)
                .help_when_truncated(title.to_string()),
        )
        .child(text(file).key(format!("{key}-file")).font(mono(11.5, 400.0, 1.4)).color(t.muted).truncate().min_w(0.0));
    let (call_id, shown, path) = (call.to_string(), title.to_string(), page.path.clone());
    let expand = icon_button(t, &format!("{key}-expand"), &format!("Open full size: {title}"), &MAXIMIZE_2)
        .on_press_tx(move |s: &mut State, tx| s.open_page_full(&call_id, &shown, &path, tx));
    let url = s.page_url_of(&page.path);
    let open = icon_button(t, &format!("{key}-open"), "Open in a new tab", &EXTERNAL_LINK);
    let open = match url {
        Some(url) => open.on_press_tx(move |_: &mut State, tx| {
            let _ = tx.open_url(&url);
        }),
        None => open,
    };
    row()
        .key(key)
        .w_full()
        .h(72.0)
        .mt(12.0)
        .mb(4.0)
        .px(16.0)
        .gap(12.0)
        .center_y()
        .fill(t.card_bg)
        .border(1.0, t.border)
        .border_layout()
        .radius(RADIUS_CARD)
        .child(icon(&PANELS_TOP_LEFT, 16.0).color(t.muted))
        .child(words)
        .child(row().gap(2.0).child(expand).children((!contained).then_some(open)))
}

/// The dialog and its scrim (`.conversation-page__overlay`: `rgba(0,0,0,.6)`, padding 24; the sheet `min(1200, 100%)` by
/// `min(900, 100%)`); `None` while it is closed.
pub fn dialog(s: &State, t: &Theme) -> Option<Vec<El>> {
    let full = s.page_full()?;
    let (w, h) = s.ui.sel.window;
    let (w, h) = if w > 0.0 && h > 0.0 { (w, h) } else { (1280.0, 760.0) };
    let sheet = col()
        .key("page-full")
        .role("dialog")
        .label(full.title.clone())
        .w(1200.0f32.min(w - 48.0))
        .h(900.0f32.min(h - 48.0))
        .fill(t.panel_bg)
        .border(1.0, t.border)
        .border_layout()
        .radius(RADIUS_PANEL)
        .clip_rounded(RADIUS_PANEL)
        .child(head(t, full))
        .child(body(s, t, full));
    let sheet = lift(t, sheet)
        .transition(Transition::scale_anchor(0.97, Vec2::new(0.5, 0.5)))
        .token_motion(Spring::new(0.15, 0.0))
        .modal_tx(true, |s: &mut State, tx| s.close_page_full(tx));
    let scrim = el().key("page-full-overlay").role("none").absolute(0.0, 0.0).w_full().h_full().fill(Color::BLACK.alpha(0.6)).a11y_hidden();
    Some(vec![scrim, centred("page-full-frame", sheet)])
}

/// `.conversation-page__head`: padding 8 10 8 16, the title in 600 weight, Close.
fn head(t: &Theme, full: &Full) -> El {
    row()
        .key("page-full-head")
        .w_full()
        .pl(16.0)
        .pr(10.0)
        .py(8.0)
        .gap(12.0)
        .center_y()
        .justify_between()
        .border_sides([0.0, 0.0, 1.0, 0.0], t.border)
        .border_layout()
        .child(text(full.title.clone()).key("page-full-title").font(sans(14.0, 600.0, 1.4)).color(t.text).truncate().shrink().min_w(0.0))
        .child(icon_button(t, CLOSE_KEY, "Close", &X).on_press_tx(|s: &mut State, tx| s.close_page_full(tx)))
}

/// The page's area: the frame the web view covers, or the fallback where none can be made.
fn body(s: &State, t: &Theme, full: &Full) -> El {
    let url = s.page_full_url(full);
    if s.web.can_show() && url.is_some() {
        return col().key("page-full-body").w_full().grow(1.0).min_h(0.0).on_layout(|s: &mut State, r| s.web.full_rect = Some(r));
    }
    let open = crate::ui::button::button(
        t,
        crate::ui::button::Variant::Outline,
        crate::ui::button::Size::Sm,
        Some(&EXTERNAL_LINK),
        "Open in browser",
    )
    .key("page-full-open-browser");
    let open = match url {
        Some(url) => open.on_press_tx(move |_: &mut State, tx| {
            let _ = tx.open_url(&url);
        }),
        None => crate::ui::button::disabled(open),
    };
    col()
        .key("page-full-fallback")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .center()
        .gap(12.0)
        .padding(24.0)
        .child(icon(&APP_WINDOW, 24.0).color(t.muted))
        .child(text(FALLBACK_TEXT).font(sans(13.0, 400.0, 1.5)).color(t.text).wrap().align(rune::Align::Center))
        .child(open)
}
