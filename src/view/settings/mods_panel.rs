//! What stands under the Mods switch (`ModsRuntimePanel.vue`, `ModsConfirm.vue`, `ModsProgress.vue`, `ModsFailed.vue`): the
//! question before Bun is installed, the install's progress, why it failed, and the quiet line once Mods are on. The state and
//! its words are `fleet::settings::mods_runtime`; this only draws them.
use super::kit;
use super::updates::bar;
use crate::State;
use crate::fleet::settings::mods_runtime::{self as rt, Action, Job, Panel, Release, View};
use crate::icons::CHECK;
use crate::theme::{RADIUS_BTN, Theme, mono, sans};
use crate::ui::rem;
use rune::{Cursor, El, Span, col, el, icon, rich, row, text};

/// `PRIMARY_BUTTON` and `SECONDARY_BUTTON` of `mods/classes.ts`: `px-3 py-1.5 text-sm font-medium`, filled with the accent or
/// the card colour and a border.
fn button(t: &Theme, key: &str, label: &str, primary: bool) -> El {
    let b = row()
        .key(key.to_string())
        .role("button")
        .label(label.to_string())
        .center()
        .px(rem(0.75))
        .py(rem(0.375))
        .radius(RADIUS_BTN)
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 2.0, t.accent);
    let font = sans(rem(0.875), 500.0, 1.43);
    if primary {
        b.fill(t.accent).hover(t.accent.alpha(0.9)).child(text(label.to_string()).font(font).color(t.primary_fg))
    } else {
        b.fill(t.card_bg)
            .border(1.0, t.border)
            .border_layout()
            .hover(t.accent.alpha(0.08))
            .child(text(label.to_string()).font(font).color(t.text))
    }
}

/// A paragraph (`max-w-xl text-sm leading-6 text-muted`) with some words in the mono face (`CODE`).
fn words(t: &Theme, key: &str, parts: &[(&str, bool)]) -> El {
    let (mut all, mut spans) = (String::new(), Vec::new());
    for (part, code) in parts {
        if *code {
            spans.push(Span::new(all.len(), all.len() + part.len()).mono().size(rem(0.8)).color(t.text));
        }
        all.push_str(part);
    }
    rich(all, spans).key(key.to_string()).font(sans(rem(0.875), 400.0, 1.71)).color(t.muted).wrap().max_w(504.0)
}

fn heading(key: &str, title: &str, color: rune::Color) -> El {
    text(title.to_string()).key(key.to_string()).font(sans(rem(0.875), 500.0, 1.43)).color(color)
}

fn size_words(release: &Release) -> String {
    release.size.filter(|s| *s > 0).map_or(String::new(), |s| format!(" (about {} MB)", rt::megabytes(Some(s))))
}

/// `ModsConfirm`: what Fleet will do, said before it does anything.
fn confirm(t: &Theme, view: &View) -> El {
    let release = &view.release;
    let size = size_words(release);
    let mine = view.found.first();
    let body = if let Some(m) = mine {
        let mut parts: Vec<(String, bool)> = vec![
            ("Fleet found Bun ".into(), false),
            (format!("{} at ", m.version), false),
            (m.display_path.clone(), true),
            (". Mods can run on it, and you keep it up to date with ".into(), false),
            ("bun upgrade".into(), true),
            (".".into(), false),
        ];
        if release.has_build {
            parts.extend([
                (format!(" Or Fleet can download its own Bun {}{size} from ", release.version), false),
                (release.source.clone(), true),
                (", check it, install it in ".into(), false),
                (release.install_folder.clone(), true),
                (" and keep it up to date.".into(), false),
            ]);
        }
        let refs: Vec<(&str, bool)> = parts.iter().map(|(a, b)| (a.as_str(), *b)).collect();
        words(t, "mods-confirm-text", &refs)
    } else if !release.has_build {
        words(t, "mods-confirm-text", &[("Fleet has no Bun build for this machine. Install Bun yourself and point Fleet at it.", false)])
    } else {
        let intro =
            format!("Mods run in a small Bun program beside Fleet. To turn them on, Fleet downloads Bun {}{size} from ", release.version);
        words(
            t,
            "mods-confirm-text",
            &[
                (&intro, false),
                (&release.source, true),
                (", checks it against a checksum built into Fleet, and installs it in ", false),
                (&release.install_folder, true),
                (". Nothing else on this machine changes.", false),
            ],
        )
    };
    let mut buttons = row().w_full().flex_wrap().items_center().gap(7.0);
    if let Some(m) = mine {
        let path = m.path.clone();
        buttons = buttons.child(
            button(t, "mods-use", &format!("Use my Bun {}", m.version), true)
                .on_press(move |s: &mut State| s.mods_runtime_action(Action::Use(path.clone()))),
        );
    }
    if release.has_build {
        let label = if mine.is_some() { "Download Fleet's own" } else { "Turn on and install" };
        buttons = buttons
            .child(button(t, "mods-install", label, mine.is_none()).on_press(|s: &mut State| s.mods_runtime_action(Action::Install)));
    }
    buttons = buttons
        .child(button(t, "mods-cancel-confirm", "Cancel", false).on_press(|s: &mut State| s.settings.mods_runtime.confirming = false));
    col().key("mods-confirm").w_full().gap(10.5).child(heading("mods-confirm-heading", "Mods need Bun", t.text)).child(body).child(buttons)
}

/// `ModsProgress`: Download, Check and Unpack with the megabytes and a bar, and Cancel.
fn progress(s: &State, t: &Theme, job: &Job, release: &Release, came_back: bool) -> El {
    let (size, _received, _total, share) = rt::size_words(job, release);
    let title = rt::progress_title(job);
    let mut steps = row().key("mods-steps").flex_wrap().items_center().gap(14.0);
    for (id, label, status) in rt::steps(job) {
        let color = match status {
            "done" => t.running_text,
            "current" => t.text,
            _ => t.muted,
        };
        let mark = if status == "done" {
            icon(&CHECK, 14.0).color(color)
        } else if status == "current" {
            el().size(10.0, 10.0).radius(5.0).fill(color)
        } else {
            el().size(10.0, 10.0).radius(5.0).border(1.5, color).border_layout()
        };
        steps = steps.child(
            row()
                .key(format!("mods-step-{id}"))
                .label(format!("{label} {status}"))
                .items_center()
                .gap(5.25)
                .child(mark)
                .child(text(label).font(sans(rem(0.875), 400.0, 1.43)).color(color)),
        );
    }
    let footer = if came_back {
        format!("Started {}. It kept going while you were away.", s.mods_started_words(job))
    } else {
        "This keeps going if you leave Settings. Mods start as soon as it's done.".to_string()
    };
    col()
        .key("mods-progress")
        .w_full()
        .gap(10.5)
        .child(
            row()
                .w_full()
                .flex_wrap()
                .items_center()
                .justify_between()
                .gap(14.0)
                .child(heading("mods-progress-heading", &title, t.text))
                .child(text(size.clone()).key("mods-progress-size").font(sans(rem(0.875), 400.0, 1.43)).color(t.muted)),
        )
        .child(bar(t, "mods-progress-bar", Some((share * 100.0).round() as u32)).label(format!("{title}: {size}")))
        .child(
            row()
                .w_full()
                .flex_wrap()
                .items_center()
                .justify_between()
                .gap(10.5)
                .child(steps)
                .child(button(t, "mods-cancel", "Cancel", false).on_press(|s: &mut State| s.mods_runtime_action(Action::Cancel))),
        )
        .child(words(t, "mods-progress-footer", &[(&footer, false)]))
}

/// `ModsFailed`: the install failed and the switch went back off: the reason in plain words and Try again, hidden when there is
/// no build for this computer, since trying again can't help.
fn failed(t: &Theme, job: &Job, release: &Release) -> El {
    let message = format!("{} {}", job.message.as_deref().unwrap_or_default(), rt::failed_hint(job, release));
    let mut c = col()
        .key("mods-failed")
        .w_full()
        .gap(10.5)
        .child(heading("mods-failed-heading", "\u{26a0} Couldn't download Bun", t.error_text))
        .child(words(t, "mods-failed-text", &[(message.trim(), false)]));
    if job.reason.as_deref() != Some("no-build") {
        c = c.child(
            row().child(button(t, "mods-retry", "Try again", true).on_press(|s: &mut State| s.mods_runtime_action(Action::Install))),
        );
    }
    c
}

/// The quiet line once Mods are on: a green dot, the Bun in use, its path, and where to add a mod.
fn ready(t: &Theme, bun: &rt::Bun) -> El {
    let whose = if bun.configured { "your Bun" } else { "Bun" };
    let link = row()
        .key("mods-showcase-link")
        .role("button")
        .label("Showcase in Mods")
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 0.0, t.accent)
        .child(text("Showcase in Mods").font(sans(rem(0.75), 400.0, 1.33)).color(t.accent_text))
        .on_press(|s: &mut State| s.open_settings("skills", Some("mods")));
    let small = sans(rem(0.75), 400.0, 1.33);
    col()
        .key("mods-ready")
        .w_full()
        .gap(5.25)
        .child(
            row().items_center().gap(7.0).child(el().size(10.5, 10.5).radius(6.0).fill(t.running)).child(
                text(format!("Mods on \u{b7} {whose} {}", bun.version))
                    .key("mods-ready-text")
                    .font(sans(rem(0.875), 400.0, 1.43))
                    .color(t.text),
            ),
        )
        .child(text(bun.display_path.clone()).key("mods-ready-path").font(mono(rem(0.8), 400.0, 1.5)).color(t.muted).wrap())
        .child(
            row()
                .flex_wrap()
                .items_center()
                .child(text("To see what mods can do, add one from the ").font(small).color(t.muted))
                .child(link)
                .child(text(".").font(small).color(t.muted)),
        )
}

/// The panel in the slot under the Mods row and at the top of its page, or nothing: no state and no refusal to show.
pub fn panel(s: &State, t: &Theme, key: String) -> Option<El> {
    let run = &s.settings.mods_runtime;
    let state = rt::panel_state(run.view.as_ref(), run.confirming, s.plugin_enabled("mods"));
    if state.is_none() && run.action_error.is_none() {
        return None;
    }
    let mut c = col().key(key).w_full().mt(10.5).pt(14.0).border_sides([1.0, 0.0, 0.0, 0.0], t.border).border_layout();
    if let (Some(state), Some(view)) = (state, run.view.as_ref()) {
        c = c.child(match (state, &view.job, &view.bun) {
            (Panel::Confirm, ..) => confirm(t, view),
            (Panel::Progress, Some(job), _) => progress(s, t, job, &view.release, run.came_back),
            (Panel::Failed, Some(job), _) => failed(t, job, &view.release),
            (Panel::Ready, _, Some(bun)) => ready(t, bun),
            _ => col(),
        });
    }
    if let Some(message) = &run.action_error {
        c = c.child(kit::error_note(t, "mods-error", message).mt(10.5));
    }
    Some(c)
}
