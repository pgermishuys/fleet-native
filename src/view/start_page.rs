//! The start pages, drawn over the whole window as Fleet's desktop app shows them (`desktop/static/splash.html` and
//! `error.html`; they are separate pages there, so there is no rail and no status bar). Sizes are the pages' own CSS pixels.
//! Their colours are the page's `--bg`, `--card`, `--text`, `--muted`, `--accent` and `--border`: the theme's `main_bg`,
//! `card_bg`, `text`, `muted`, `accent` and `border`, which carry the same values.
use crate::desktop::start::{Phase, RunningCard};
use crate::theme::{Theme, mono, sans};
use crate::{State, Tx};
use rune::{Align, Color, El, col, el, row, text};

/// `.bar::after`'s sweep: 1.2 s, `ease-in-out`.
pub const SWEEP_SECS: f64 = 1.2;
const BAR_W: f32 = 160.0;
const SLIDER_W: f32 = BAR_W * 0.4;

/// What covers the window, if anything.
pub fn page(s: &State, t: &Theme) -> Option<El> {
    match &s.start.phase {
        Phase::Starting(status) => Some(splash(s, t, status)),
        Phase::Failed { message, log } => Some(error(s, t, message, log)),
        Phase::RunningFleet(card) => Some(running_fleet(s, t, card)),
        Phase::Idle | Phase::Owned => None,
    }
}

/// The icon the pages show: Fleet's mark, centred in a square the way `icon.png` has it (the mark takes about 83 % of the width).
fn icon(s: &State, size: f32, key: &'static str) -> El {
    let w = size * 0.825;
    let mut mark = el().size(w, w * 234.0 / 511.0);
    if let Some(image) = &s.logo {
        mark = mark.pixels(image.pixels());
    }
    el().key(key).size(size, size).center().child(mark)
}

/// `cubic-bezier(.42, 0, .58, 1)` at `x` in 0..1, as CSS solves it.
pub fn ease_in_out(x: f64) -> f64 {
    let (x1, y1, x2, y2) = (0.42, 0.0, 0.58, 1.0);
    let at = |a: f64, b: f64, u: f64| 3.0 * a * u * (1.0 - u).powi(2) + 3.0 * b * u * u * (1.0 - u) + u.powi(3);
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..24 {
        let mid = (lo + hi) / 2.0;
        if at(x1, x2, mid) < x {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    at(y1, y2, (lo + hi) / 2.0)
}

/// Where the slider's left edge is at `clock`: from fully left of the bar (`translateX(-100%)`) to `250%` of its own width.
pub fn slider_x(clock: f64) -> f32 {
    let p = ease_in_out((clock % SWEEP_SECS) / SWEEP_SECS) as f32;
    -SLIDER_W + p * (SLIDER_W * 3.5)
}

fn splash(s: &State, t: &Theme, status: &str) -> El {
    let slider = el().key("start-slider").w(SLIDER_W).h(3.0).radius(3.0).fill(t.accent).offset(slider_x(s.clock), 0.0);
    let bar = row()
        .key("start-bar")
        .role("progressbar")
        .label("Starting Fleet")
        .w(BAR_W)
        .h(3.0)
        .radius(3.0)
        .clip_rounded(3.0)
        .fill(t.text.alpha(0.09))
        .child(slider);
    let bar = if t.dark { bar.fill(Color::WHITE.alpha(0.075)) } else { bar };
    col()
        .key("start-splash")
        .w_full()
        .h_full()
        .fill(t.main_bg)
        .center()
        .gap(18.0)
        .child(icon(s, 72.0, "start-icon"))
        .child(bar)
        .child(text(status.to_string()).key("start-status").font(sans(14.0, 400.0, 1.5)).color(t.muted))
}

fn error(s: &State, t: &Theme, message: &str, log: &str) -> El {
    let button = |key: &'static str, label: &str, primary: bool| {
        let (fill, border, ink) = if primary { (t.accent, t.accent, Color::WHITE) } else { (t.card_bg, t.border, t.text) };
        row()
            .key(key)
            .label(label.to_string())
            .px(14.0)
            .py(7.0)
            .center()
            .radius(8.0)
            .fill(fill)
            .border(1.0, border)
            .border_layout()
            .cursor(rune::Cursor::Pointer)
            .focus_ring(2.0, 2.0, t.accent)
            .child(text(label.to_string()).font(sans(13.0, 500.0, 1.5)).color(ink))
    };
    let log_box = row()
        .key("start-log")
        .w_full()
        .px(10.0)
        .py(8.0)
        .radius(6.0)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.card_bg)
        .child(text(log.to_string()).font(mono(12.0, 400.0, 1.5)).color(t.text).wrap().w_full().selectable(true));
    let body = col()
        .max_w(460.0)
        .gap(14.0)
        .child(icon(s, 40.0, "start-icon"))
        .child(text("Fleet didn't start").key("start-title").font(sans(18.0, 600.0, 1.5)).color(t.text))
        .child(text(message.to_string()).key("start-message").font(sans(14.0, 400.0, 1.5)).color(t.muted).wrap().w_full())
        .child(text("Its log is here:").font(sans(14.0, 400.0, 1.5)).color(t.muted))
        .child(log_box)
        .child(
            row()
                .gap(8.0)
                .mt(4.0)
                .child(button("start-retry", "Try again", true).on_press(|s: &mut State| s.start.retry()))
                .child(button("start-open-logs", "Open logs", false).on_press(|s: &mut State| s.start.open_logs())),
        );
    // `body { display: grid; place-items: center; padding: 0 16px }`, the column `main` as wide as its text asks (max 460).
    col().key("start-error").w_full().h_full().fill(t.main_bg).center().px(16.0).child(body.align(Align::Start))
}

/// ` (started 20 Nov)` for the Fleet's start time in this computer's zone, or nothing when the time can't be read.
pub fn started_words(since: &str, offset_secs: i64) -> String {
    let Some(ms) = crate::fleet::work::parse_iso_ms(since) else { return String::new() };
    let (_, month, day, ..) = crate::conv::time::civil(ms, offset_secs);
    format!(" (started {day} {})", crate::conv::time::MONTHS[month])
}

/// `running-fleet.html`: a card, 470 wide, that asks once whether the app may use the Fleet already running here.
fn running_fleet(s: &State, t: &Theme, card: &RunningCard) -> El {
    let since = started_words(&card.since, crate::conv::time::local_offset_secs());
    let message = format!(
        "{} runs Fleet in the background{since}. The desktop app will use it: the same sessions, harnesses and machines, and it keeps running when you close the window.",
        card.host
    );
    let detail = format!("{} · Fleet {}", card.host, card.version).trim().to_string();
    let detail_box = row()
        .key("start-running-detail")
        .w_full()
        .px(12.0)
        .py(10.0)
        .radius(10.0)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.main_bg)
        .child(text(detail).font(sans(12.5, 400.0, 1.5)).color(t.muted).wrap().w_full());
    let button = row()
        .key("start-use-it")
        .label("Use it")
        .px(14.0)
        .py(7.0)
        .center()
        .radius(8.0)
        .fill(t.accent)
        .border(1.0, t.accent)
        .border_layout()
        .focusable(true)
        // Return presses it, as it does a focused HTML button; Rune sends Return to the default button.
        .default_button()
        .focus_ring(2.0, 2.0, t.accent)
        .child(text("Use it").font(sans(13.0, 500.0, 1.5)).color(Color::WHITE));
    let button = if card.pressed {
        button.opacity(0.6).disabled(true)
    } else {
        button.cursor(rune::Cursor::Pointer).on_press(|s: &mut State| s.use_running_fleet())
    };
    let body = col()
        .key("start-running")
        .w(470.0)
        .gap(12.0)
        .px(24.0)
        .py(22.0)
        .radius(12.0)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.card_bg)
        .child(icon(s, 40.0, "start-icon"))
        .child(text("Fleet already runs on this computer").key("start-title").font(sans(18.0, 600.0, 1.5)).color(t.text))
        .child(text(message).key("start-message").font(sans(14.0, 400.0, 1.5)).color(t.muted).wrap().w_full())
        .child(detail_box)
        .child(row().w_full().justify_end().mt(4.0).child(button));
    col().key("start-running-page").w_full().h_full().fill(t.main_bg).center().px(16.0).child(body)
}

/// Keeps the splash's sweep drawing: a frame every 16 ms while it shows. The card's button gets the keyboard once
/// (`autofocus`: Rune's own is for modals).
pub fn keep(s: &mut State, tx: &mut Tx) -> bool {
    if let Phase::RunningFleet(card) = &mut s.start.phase
        && card.focused < 2
    {
        // The key has to be in the drawn tree when the focus is asked for: not in the poll right after the card was posted.
        card.focused += 1;
        if card.focused == 2 {
            tx.focus("start-use-it", false);
        }
        return true;
    }
    if matches!(s.start.phase, Phase::Starting(_)) {
        tx.wake_after(std::time::Duration::from_millis(16));
        return true;
    }
    false
}
