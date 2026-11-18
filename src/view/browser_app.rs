//! The browser tab's app (component 14, piece I): the panel that stands where the page would be while the app starts, has
//! exited or is stopped, the 36 px strip under the page, the output drawer over it and the port select in the bar. Ports the
//! app half of `components/canvas/BrowserCanvas.vue`; what it does is `web::browser_app`.
use crate::State;
use crate::fleet::app_runs::{AppRun, AppStatus};
use crate::icons::*;
use crate::theme::{RADIUS_BTN, Theme, mono, sans};
use crate::ui::Open;
use crate::web::browser_app::{Panel, panel_title, status_label};
use rune::{Cursor, Cycle, El, ScrollSpec, col, el, icon, row, text};

/// Fleet's container query: the buttons lose their labels under this width, and the status its text under the next.
const NARROW: f32 = 440.0;
const NARROWER: f32 = 300.0;

/// `.browser-canvas__dot`: 7 px; running green, starting amber and pulsing, failed and exited red, else muted.
fn dot(t: &Theme, status: Option<AppStatus>) -> El {
    // `--transition-pulse` is 2 s.
    static STOPS: [(f32, f32); 3] = [(0.0, 1.0), (0.5, 0.35), (1.0, 1.0)];
    let color = match status {
        Some(AppStatus::Running) => t.running,
        None | Some(AppStatus::Starting) => t.idle,
        Some(AppStatus::BuildFailed | AppStatus::Exited) => t.error,
        Some(AppStatus::Stopped) => t.muted,
    };
    let d = el().size(7.0, 7.0).radius(3.5).fill(color).min_w(7.0);
    if matches!(status, None | Some(AppStatus::Starting)) {
        d.cycle(Cycle { period: 2.0, phase: 0.0, stops: &STOPS, still: 1.0, fps: 30.0 })
    } else {
        d
    }
}

/// `.browser-canvas__start`: 28 px, padding 0 12, gap 6, the accent fill; half there while Fleet answers.
fn start_button(t: &Theme, key: &str, busy: bool) -> El {
    let b = row()
        .key(key.to_string())
        .role("button")
        .label("Start")
        .h(28.0)
        .px(12.0)
        .gap(6.0)
        .center_y()
        .radius(RADIUS_BTN)
        .fill(t.accent)
        .hover(t.panel_bg.alpha(0.1))
        .focus_ring(2.0, 1.0, t.accent.alpha(0.5))
        .child(icon(&PLAY, 13.0).color(t.primary_fg))
        .child(text("Start").font(sans(12.5, 500.0, 1.3)).color(t.primary_fg));
    if busy { b.opacity(0.5).disabled(true).cursor(Cursor::Default) } else { b.cursor(Cursor::Pointer).focusable(true) }
}

/// `.browser-canvas__panel` for an app: its state, the command, Start, and the current run's last lines.
pub fn panel(s: &State, t: &Theme, tab: &str, panel: Panel, shell: El) -> El {
    let app = s.browser_app(tab);
    let ui = s.browser_ui(tab);
    let status = app.as_ref().map(|a| a.status);
    let title = panel_title(panel, app.as_ref(), ui.is_some_and(|u| u.app_id.is_some()));
    let mut all = shell.key("browser-panel").child(
        row()
            .gap(9.0)
            .center_y()
            .child(dot(t, status))
            .child(text(title).key("browser-panel-title").font(sans(13.0, 500.0, 1.5)).color(t.text).wrap()),
    );
    if let Some(app) = &app {
        all = all.child(
            text(app.command.clone()).key("browser-panel-command").font(mono(12.0, 400.0, 1.3)).color(t.muted).truncate().max_w(520.0),
        );
    }
    if matches!(panel, Panel::Exited | Panel::Stopped) {
        let tab = tab.to_string();
        all = all.child(
            start_button(t, "browser-panel-start", ui.is_some_and(|u| u.app.busy))
                .on_press(move |s: &mut State| s.browser_app_act(&tab, "restart")),
        );
    }
    let tail = s.browser_tail(tab);
    if matches!(panel, Panel::Starting | Panel::Exited) && !tail.is_empty() {
        all = all.child(
            col()
                .key("browser-panel-tail")
                .label("Recent output")
                .w_full()
                .mt(4.0)
                .px(10.0)
                .py(8.0)
                .radius(RADIUS_BTN)
                .border(1.0, t.border)
                .border_layout()
                .fill(t.text.alpha(0.03))
                .child(text(tail.join("\n")).font(mono(11.5, 400.0, 1.5)).color(t.muted).wrap().w_full().selectable(true)),
        );
    }
    all
}

/// `.browser-canvas__app`: the output drawer, a refusal, and the strip; nothing without an app.
pub fn app_section(s: &State, t: &Theme, tab: &str) -> Option<El> {
    let app = s.browser_app(tab)?;
    let ui = s.browser_ui(tab)?;
    let mut all = col().key("browser-app").w_full().border_sides([1.0, 0.0, 0.0, 0.0], t.border).border_layout();
    if ui.app.show_logs {
        let lines = s.apps.output(&app.id).map(|o| o.lines.clone()).unwrap_or_default();
        let shown = if lines.is_empty() { "No output yet.".to_string() } else { lines.join("\n") };
        all = all.child(
            col()
                .key("browser-output")
                .label("Output")
                .w_full()
                .max_h(220.0)
                .scroll(ScrollSpec { stick_bottom: true })
                .px(12.0)
                .py(8.0)
                .border_sides([0.0, 0.0, 1.0, 0.0], t.border)
                .border_layout()
                .fill(t.text.alpha(0.03))
                .child(text(shown).font(mono(11.5, 400.0, 1.5)).color(t.text).wrap().w_full().selectable(true)),
        );
    }
    if let Some((why, _)) = &ui.app.action_error {
        all =
            all.child(row().w_full().px(12.0).py(6.0).border_sides([0.0, 0.0, 1.0, 0.0], t.border).border_layout().child(
                text(why.clone()).key("browser-action-error").role("alert").font(sans(12.0, 400.0, 1.5)).color(t.error_text).wrap(),
            ));
    }
    Some(all.child(strip(t, tab, &app, ui.app.busy, ui.app.show_logs, ui.app.width.unwrap_or(f32::MAX))))
}

/// `.browser-canvas__strip`: dot, command, status, Restart or Start, Stop while it's live, Output.
fn strip(t: &Theme, tab: &str, app: &AppRun, busy: bool, logs: bool, width: f32) -> El {
    let live = crate::web::browser_app::is_live(Some(app.status));
    let narrow = width <= NARROW;
    let failed = app.status == AppStatus::BuildFailed;
    let button = |key: &str, tip: &str, glyph: &'static rune::Icon, size: f32, on: bool, off: bool| {
        let mut b = row()
            .key(key.to_string())
            .role("button")
            .label(tip.to_string())
            .help(tip.to_string())
            .h(26.0)
            .gap(5.0)
            .center()
            .radius(RADIUS_BTN)
            .hover_group()
            .child(icon(glyph, size).color(if on { t.text } else { t.muted }).group_color(t.text));
        b = if narrow {
            b.w(26.0).min_w(26.0)
        } else {
            b.px(8.0).child(text(tip.to_string()).font(sans(12.0, 400.0, 1.3)).color(if on { t.text } else { t.muted }).group_color(t.text))
        };
        b = if on { b.fill(t.accent_dim) } else { b };
        if off {
            b.opacity(0.5).disabled(true).cursor(Cursor::Default)
        } else {
            b.hover(t.accent_dim).cursor(Cursor::Pointer).focusable(true).focus_ring(2.0, -2.0, t.accent)
        }
    };
    let (restart_tab, stop_tab, output_tab) = (tab.to_string(), tab.to_string(), tab.to_string());
    let mut r = row()
        .key("browser-strip")
        .w_full()
        .h(36.0)
        .pl(12.0)
        .pr(8.0)
        .gap(if narrow { 6.0 } else { 8.0 })
        .center_y()
        .child(dot(t, Some(app.status)))
        .child(
            text(app.command.clone())
                .key("browser-strip-command")
                .help(app.command.clone())
                .font(mono(11.5, 400.0, 1.3))
                .color(t.text)
                .truncate()
                .grow(1.0)
                .shrink()
                .min_w(0.0),
        );
    if width > NARROWER {
        r = r.child(
            text(status_label(app))
                .key("browser-strip-status")
                .font(sans(12.0, 400.0, 1.3))
                .color(if failed { t.error_text } else { t.muted })
                .min_w(0.0),
        );
    }
    let (word, glyph, size) = if live { ("Restart", &ROTATE_CCW, 13.0) } else { ("Start", &PLAY, 13.0) };
    r = r.child(
        button("browser-restart", word, glyph, size, false, busy).on_press(move |s: &mut State| s.browser_app_act(&restart_tab, "restart")),
    );
    if live {
        r = r.child(
            button("browser-stop", "Stop", &SQUARE, 12.0, false, busy).on_press(move |s: &mut State| s.browser_app_act(&stop_tab, "stop")),
        );
    }
    r.child(
        button("browser-output-toggle", "Output", &SCROLL_TEXT, 13.0, logs, false)
            .on_press(move |s: &mut State| s.browser_toggle_output(&output_tab)),
    )
}

/// `.browser-canvas__ports`: 28 px, mono 12, `:{port}`; only for an app with more than one port.
pub fn port_select(s: &State, t: &Theme, tab: &str) -> Option<El> {
    let app = s.browser_app(tab)?;
    if app.ports.len() < 2 {
        return None;
    }
    let shown = s.browser_port(tab).map_or_else(String::new, |p| format!(":{p}"));
    Some(
        row()
            .key(PORT_SELECT)
            .role("combobox")
            .label("Port")
            .h(28.0)
            .px(6.0)
            .gap(4.0)
            .mr(2.0)
            .center_y()
            .radius(RADIUS_BTN)
            .border(1.0, t.border)
            .border_layout()
            .cursor(Cursor::Pointer)
            .focusable(true)
            .focus_ring(2.0, -1.0, t.accent)
            .child(text(shown).font(mono(12.0, 400.0, 1.3)).color(t.text))
            .child(icon(&CHEVRON_DOWN, 12.0).color(t.muted))
            .on_press(|s: &mut State| s.ui.open = Some(Open::Select("browser-port"))),
    )
}

pub const PORT_SELECT: &str = "browser-port";

/// The open list of ports.
pub fn port_list(s: &State, t: &Theme) -> Vec<El> {
    let tab = s.active_canvas();
    let Some(app) = s.browser_app(&tab) else { return Vec::new() };
    let labels: Vec<(String, String)> = app.ports.iter().map(|p| (p.to_string(), format!(":{p}"))).collect();
    let options: Vec<(&str, &str)> = labels.iter().map(|(v, l)| (v.as_str(), l.as_str())).collect();
    let current = s.browser_port(&tab).map(|p| p.to_string());
    let choose: crate::ui::select::Choose = std::rc::Rc::new(move |s: &mut State, v: &str| {
        if let Ok(port) = v.parse::<u16>() {
            s.browser_pick_port(&tab, port);
        }
    });
    crate::ui::select::content(t, "browser-port-list", PORT_SELECT, 72.0, &options, current.as_deref(), choose)
}
