//! `HarnessSetupWizard.vue` (`components/harness-setup`): the dialog that opens on a first launch with no harness ready, with
//! three steps: welcome, harnesses, ready. The rows of the harnesses step are `HarnessSetupRows.vue`, piece O; until it is
//! built the step lists each harness with its state so Continue has something to follow. Sizes are the component's own
//! scoped CSS (literal px).
use crate::State;
use crate::fleet::harness_setup::{Step, ready_harnesses, ready_line};
use crate::icons::{CIRCLE_CHECK, PLUS};
use crate::theme::{Theme, sans};
use crate::ui::dialog::{lift, overlay};
use crate::ui::overlay::centred;
use rune::{Cursor, El, Span, col, el, icon, rich, row, text};

/// The dialog, or `None` while it is closed. Click outside and Escape do nothing (C12.5): only the buttons leave.
pub fn dialog(s: &State, t: &Theme) -> Option<Vec<El>> {
    if !s.harness_setup.is_open || s.harness_setup.cloud {
        return None;
    }
    let step = s.harness_setup.step;
    let body = match step {
        Step::Welcome => welcome(s, t),
        Step::Harnesses => harnesses(s, t),
        Step::Ready => ready(s, t),
    };
    // `sm:max-w-md` (28rem = 392) for welcome and ready, `sm:max-w-2xl` (42rem = 588) for the harnesses; the window's height
    // less 2rem is the most it takes.
    let width = if step == Step::Harnesses { 588.0 } else { 392.0 };
    let panel = col()
        .key("harness-setup-wizard")
        .role("dialog")
        .label("Set up a harness")
        .w(width)
        .max_w(width)
        .gap(20.0)
        .padding(24.0)
        .fill(t.card_bg)
        .border(1.0, t.border)
        .child(body)
        .child(dots(t, step));
    let panel = lift(t, panel).modal(false, |_: &mut State| {});
    Some(vec![overlay("harness-setup"), centred("harness-setup-frame", panel)])
}

/// `.harness-setup__dots`: one 24×6 pill per step, the current one in the accent.
fn dots(t: &Theme, step: Step) -> El {
    let now = [Step::Welcome, Step::Harnesses, Step::Ready].iter().position(|p| *p == step).unwrap_or(0);
    row()
        .key("harness-setup-dots")
        .a11y_hidden()
        .justify_center()
        .gap(6.0)
        .children((0..3).map(|i| el().size(24.0, 6.0).radius(999.0).fill(if i == now { t.accent } else { t.border })))
}

/// `.harness-setup__primary`: the accent button; `wide` is 100% up to 320.
fn primary(t: &Theme, key: &str, label: &str, lead: Option<El>, wide: bool, on: bool, press: impl Fn(&mut State) + 'static) -> El {
    let mut b = row()
        .key(key.to_string())
        .role("button")
        .label(label.to_string())
        .center()
        .gap(8.0)
        .py(if wide { 10.0 } else { 9.0 })
        .px(16.0)
        .radius(8.0)
        .fill(t.accent)
        .hover(t.accent.alpha(0.9))
        .cursor(Cursor::Pointer)
        .focus_ring(2.0, 2.0, t.accent);
    if wide {
        b = b.w_full().max_w(320.0);
    }
    if let Some(lead) = lead {
        b = b.child(lead);
    }
    b = b.child(text(label.to_string()).font(sans(14.0, 500.0, 1.2)).color(t.primary_fg));
    if on { b.on_press(press) } else { b.opacity(0.5).disabled(true).cursor(Cursor::Default) }
}

/// `.harness-setup__text-btn`: muted, underlined, 12.5px, `--text` on hover.
fn text_button(t: &Theme, key: &str, label: &str, press: impl Fn(&mut State) + 'static) -> El {
    let spans = vec![Span::new(0, label.len()).underline(t.muted)];
    row()
        .key(key.to_string())
        .role("button")
        .label(label.to_string())
        .padding(4.0)
        .cursor(Cursor::Pointer)
        .focus_ring(2.0, 0.0, t.accent)
        .hover_group()
        .child(rich(label.to_string(), spans).font(sans(12.5, 400.0, 1.2)).color(t.muted).group_color(t.text))
        .on_press(press)
}

fn title(t: &Theme, words: &str, size: f32) -> El {
    text(words.to_string()).font(sans(size, 600.0, 1.2)).color(t.text)
}

/// Step "welcome": the logo tile, the title and lede, Get started and Skip setup.
fn welcome(s: &State, t: &Theme) -> El {
    // The logo file is 511×234, drawn `object-fit: contain` in the tile's 42px of content.
    let mut tile = el().size(64.0, 64.0).padding(10.0).radius(10.0).fill(t.card_bg).border(1.0, t.border).center();
    if let Some(image) = &s.logo {
        tile = tile.child(el().size(42.0, 42.0 * 234.0 / 511.0).pixels(image.pixels()));
    }
    col()
        .key("harness-setup-welcome")
        .items_center()
        .gap(18.0)
        .py(12.0)
        .child(tile)
        .child(title(t, "Welcome to Weave Fleet", 20.0))
        .child(
            text("Fleet runs your sessions through a harness: a coding tool like OpenCode or Claude Code, installed on this machine. Let's get one ready. It takes a minute or two.")
                .font(sans(13.5, 400.0, 1.55))
                .color(t.muted)
                .wrap()
                .max_w(360.0)
                .align(rune::Align::Center),
        )
        .child(primary(t, "harness-setup-get-started", "Get started", None, true, true, |s: &mut State| {
            s.harness_setup.step = Step::Harnesses
        }))
        .child(text_button(t, "harness-setup-skip", "Skip setup", |s: &mut State| s.leave_harness_setup()))
}

/// Step "harnesses": the title, the lede, the rows, and the footer with Skip for now (Close) and Continue.
fn harnesses(s: &State, t: &Theme) -> El {
    let list = s.setup_harnesses();
    let any_ready = !ready_harnesses(&list).is_empty();
    let rows = col().key("harness-setup-rows").gap(8.0).children(list.all.iter().map(|h| {
        let label = if h.is_ready() { "Ready".to_string() } else { h.reason.clone().unwrap_or_else(|| "Not installed".into()) };
        row()
            .key(format!("harness-setup-row-{}", h.kind))
            .gap(10.0)
            .padding(12.0)
            .radius(10.0)
            .fill(t.card_bg)
            .border(1.0, t.border)
            .center_y()
            .child(text(h.display_name.clone()).font(sans(14.0, 500.0, 1.3)).color(t.text))
            .child(text(label).font(sans(12.5, 400.0, 1.3)).color(if h.is_ready() { t.running_text } else { t.muted }))
    }));
    col()
        .key("harness-setup-harnesses")
        .gap(18.0)
        .child(
            col().gap(6.0).child(title(t, "Install a harness", 18.0)).child(
                text("You need at least one. You can add the other later in Settings → Harnesses.")
                    .font(sans(13.5, 400.0, 1.55))
                    .color(t.muted)
                    .wrap(),
            ),
        )
        .child(rows)
        .child(
            row()
                .key("harness-setup-foot")
                .justify_between()
                .center_y()
                .gap(10.0)
                .child(text_button(t, "harness-setup-skip", if any_ready { "Close" } else { "Skip for now" }, |s: &mut State| {
                    s.leave_harness_setup()
                }))
                .child(primary(t, "harness-setup-continue", "Continue", None, false, any_ready, |s: &mut State| {
                    s.harness_setup_continue()
                })),
        )
}

/// Step "ready": the green badge, "You're all set", which harnesses are ready, and Start a session.
fn ready(s: &State, t: &Theme) -> El {
    let badge =
        el().size(64.0, 64.0).radius(32.0).fill(t.running.alpha(0.12)).center().child(icon(&CIRCLE_CHECK, 30.0).color(t.running_text));
    col()
        .key("harness-setup-ready")
        .items_center()
        .gap(18.0)
        .py(12.0)
        .child(badge)
        .child(title(t, "You're all set", 20.0))
        .child(
            text(ready_line(&s.setup_harnesses()))
                .font(sans(13.5, 400.0, 1.55))
                .color(t.muted)
                .wrap()
                .max_w(360.0)
                .align(rune::Align::Center),
        )
        .child(primary(
            t,
            "harness-setup-start-session",
            "Start a session",
            Some(icon(&PLUS, 16.0).color(t.primary_fg)),
            true,
            true,
            |s: &mut State| s.harness_setup_start_session(),
        ))
}
