//! The browser tab's Your view / Agent's view (component 14, piece J; row 14.67): the toggle in the bar and the banner under
//! it, from `components/canvas/BrowserCanvas.vue` (`.browser-canvas__views`, `.browser-canvas__banner`). Both exist only while
//! the agent has its own tab on this page. The picture is component 10's `view::agent_tab`.
use crate::State;
use crate::fleet::browser_tab::BrowserPage;
use crate::icons::{BOT, INFO};
use crate::theme::{RADIUS_BTN, Theme, sans};
use rune::{Color, Cursor, El, Shadow, col, el, icon, row, text};

/// The container query's width (`@container browser-canvas (max-width: 520px)`): "You" and "Agent" under it.
pub const SHORT_UNDER: f32 = 520.0;

/// The short words are used up to and including 520 wide; before the tab has been measured the long ones stay.
pub fn short(width: Option<f32>) -> bool {
    width.is_some_and(|w| w <= SHORT_UNDER)
}

/// `.browser-canvas__views`: a pill of two 24 px buttons in a `--text` 7 % ground; the pressed one lifts onto the panel colour.
pub fn views(s: &State, t: &Theme, tab: &str, page: &BrowserPage) -> Option<El> {
    s.browser_agent_tab(tab, page)?;
    let agent = s.browser_ui(tab).is_some_and(|u| u.agent);
    let short = short(s.browser_ui(tab).and_then(|u| u.app.width));
    let button = |key: &str, long: &str, brief: &str, tip: &str, pip: bool, on: bool| {
        let ink = if on { t.text } else { t.muted };
        let mut b = row()
            .key(key.to_string())
            .role("button")
            .label(long.to_string())
            .help(tip.to_string())
            .h(24.0)
            .px(9.0)
            .gap(6.0)
            .center_y()
            .radius(6.0)
            .cursor(Cursor::Pointer)
            .focusable(true)
            .focus_ring(2.0, -1.0, t.accent);
        if pip {
            b = b.child(el().w(6.0).h(6.0).radius(3.0).fill(t.accent));
        }
        b = b.child(text(if short { brief } else { long }.to_string()).font(sans(12.0, 400.0, 1.3)).color(ink));
        if on {
            b.fill(t.panel_bg).box_shadow(Shadow { x: 0.0, y: 1.0, blur: 2.0, spread: 0.0, color: Color::BLACK.alpha(0.18), inset: false })
        } else {
            b
        }
    };
    let (you_tab, agent_tab) = (tab.to_string(), tab.to_string());
    Some(
        row()
            .key("browser-views")
            .role("group")
            .label("Whose view")
            .gap(2.0)
            .padding(2.0)
            .ml(2.0)
            .mr(4.0)
            .radius(8.0)
            .fill(t.text.alpha(0.07))
            .child(
                button("browser-view-you", "Your view", "You", "Your view: the page in your browser", false, !agent)
                    .on_press(move |s: &mut State| s.browser_set_view(&you_tab, false)),
            )
            .child(
                button("browser-view-agent", "Agent's view", "Agent", "Agent's view: a live picture of the agent's own tab", true, agent)
                    .on_press(move |s: &mut State| s.browser_set_view(&agent_tab, true)),
            ),
    )
}

/// `.browser-canvas__banner`: what the user is looking at, under the bar. In Your view it offers Agent's view.
pub fn banner(s: &State, t: &Theme, tab: &str, page: &BrowserPage) -> Option<El> {
    s.browser_agent_tab(tab, page)?;
    let watching = s.browser_ui(tab).is_some_and(|u| u.agent);
    let (glyph, words) = if watching {
        (&BOT, "Live picture of the agent's tab. You can watch, not click. Its sign-ins and cookies are its own.")
    } else {
        (&INFO, "The agent is trying this page in its own browser. What it types and clicks doesn't show here.")
    };
    let ink = if watching { t.muted } else { t.text };
    let mut r = row()
        .key("browser-agent-banner")
        .w_full()
        .gap(10.0)
        .px(12.0)
        .py(7.0)
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], if watching { t.border } else { t.accent.alpha(0.25) })
        .border_layout()
        .fill(if watching { t.text.alpha(0.05) } else { t.accent.alpha(0.12) })
        .child(icon(glyph, 14.0).color(ink))
        .child(
            col()
                .grow(1.0)
                .shrink()
                .min_w(0.0)
                .child(text(words).key("browser-agent-banner-text").font(sans(12.0, 400.0, 1.4)).color(ink).wrap()),
        );
    if !watching {
        let tab = tab.to_string();
        r = r.child(
            row()
                .key("browser-banner-agent")
                .role("button")
                .h(24.0)
                .px(9.0)
                .center()
                .radius(RADIUS_BTN)
                .border(1.0, t.border)
                .border_layout()
                .fill(t.panel_bg)
                .hover(t.text.alpha(0.06))
                .cursor(Cursor::Pointer)
                .focusable(true)
                .focus_ring(2.0, -1.0, t.accent)
                .child(text("Agent's view").font(sans(12.0, 400.0, 1.3)).color(t.text))
                .on_press(move |s: &mut State| s.browser_set_view(&tab, true)),
        );
    }
    Some(r)
}
