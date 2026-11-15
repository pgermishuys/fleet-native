//! The Export as Markdown dialog. Ports `components/session/ExportMarkdownDialog.vue`: the options on the left, the file
//! (as text, or drawn) on the right, and Save and Copy under it. Opened from the palette (and later the session menu); the
//! state is `conv::export_dialog`, the work is `fleet::export`. The window is `max-w-5xl` (896px) and 80% of the height.
use crate::State;
use crate::conv::export::{ExportToolMode, looks_like_key};
use crate::conv::export_dialog::{ExportDialog, Load, Pane};
use crate::icons::*;
use crate::theme::{RADIUS_CARD, Theme, mono, sans};
use crate::ui::button::{Size, Variant, button, disabled};
use crate::ui::dialog::{close_x, lift, overlay};
use crate::ui::overlay::centred;
use crate::ui::rem;
use rune::{Color, Cursor, Cycle, El, ScrollSpec, Span, Spring, Transition, Vec2, col, el, icon, rich, row, text};

const KEY: &str = "export";

/// `h-[80vh]` of the window, and `max-w-5xl` (or the window less 2rem).
fn size(s: &State) -> (f32, f32) {
    let (w, h) = s.ui.sel.window;
    let (w, h) = if w > 0.0 && h > 0.0 { (w, h) } else { (1280.0, 760.0) };
    ((rem(64.0)).min(w - rem(2.0)), (h * 0.8).round())
}

/// The dialog and its scrim, to mount over the window; `None` while it is closed.
pub fn dialog(s: &State, t: &Theme) -> Option<Vec<El>> {
    let d = s.export.dialog.as_ref()?;
    let (w, h) = size(s);
    let body = row().w_full().h_full().min_h(0.0).child(options(t, d)).child(preview(t, d));
    let panel = col()
        .key("export-markdown-dialog")
        .role("dialog")
        .label("Export as Markdown")
        .w(w)
        .h(h)
        .fill(t.panel_bg)
        .border(1.0, t.border)
        .radius(rem(0.5))
        .child(body)
        .child(close_x(t, KEY, |s: &mut State| s.export_close()));
    let panel = lift(t, panel)
        .transition(Transition::scale_anchor(0.95, Vec2::new(0.5, 0.5)))
        .token_motion(Spring::new(0.2, 0.0))
        .modal(true, |s: &mut State| s.export_close());
    Some(vec![overlay(KEY), centred("export-frame", panel)])
}

// ---- the options ----------------------------------------------------------------------------------------------------

/// A native checkbox or radio, 13px, in the accent colour.
fn mark(t: &Theme, on: bool, round: bool) -> El {
    let b = el().size(13.0, 13.0).mt(rem(0.25)).border(1.0, if on { t.accent } else { t.muted }).border_layout().radius(if round {
        6.5
    } else {
        2.0
    });
    match (on, round) {
        (false, _) => b,
        (true, true) => b.center().child(el().size(5.0, 5.0).radius(2.5).fill(t.accent)),
        (true, false) => b.fill(t.accent).center().child(icon(&CHECK, 10.0).color(t.primary_fg)),
    }
}

/// `<label class="flex items-start gap-2">` with the control, the words and a hint under them.
fn choice(t: &Theme, key: &str, on: bool, round: bool, label: &str, hint: Option<&str>) -> El {
    let mut words = col().child(text(label.to_string()).font(sans(rem(0.875), 400.0, 1.43)).color(t.text));
    if let Some(hint) = hint {
        words = words.child(text(hint.to_string()).font(sans(rem(0.75), 400.0, 1.33)).color(t.muted).wrap());
    }
    row()
        .key(key.to_string())
        .role(if round { "radio" } else { "checkbox" })
        .label(label.to_string())
        .checked(on)
        .gap(rem(0.5))
        .items_start()
        .cursor(Cursor::Pointer)
        .focus_ring(2.0, 2.0, t.accent)
        .child(mark(t, on, round))
        .child(words.shrink().min_w(0.0))
}

/// `role="note"` boxes under the options.
fn note(key: &str, body: String, edge: Color, ink: Color, fill: Option<Color>) -> El {
    let mut b = col()
        .key(key.to_string())
        .role("note")
        .w_full()
        .padding(rem(0.5))
        .border(1.0, edge)
        .border_layout()
        .radius(rem(0.375))
        .child(text(body).font(sans(rem(0.75), 400.0, 1.33)).color(ink).wrap());
    if let Some(fill) = fill {
        b = b.fill(fill);
    }
    b
}

fn options(t: &Theme, d: &ExportDialog) -> El {
    let o = d.options;
    let mut c = col()
        .w(rem(18.0))
        .h_full()
        .padding(rem(1.0))
        .gap(rem(0.75))
        .border_sides([0.0, 1.0, 0.0, 0.0], t.border)
        .scroll(ScrollSpec { stick_bottom: false })
        .key("export-options")
        .child(text("Export as Markdown").font(sans(rem(1.0), 600.0, 1.5)).color(t.text))
        .child(
            text("A readable copy for an issue, a PR or a doc. It cannot be imported back.")
                .font(sans(rem(0.75), 400.0, 1.33))
                .color(t.muted)
                .wrap(),
        );
    let tools = col()
        .gap(rem(0.375))
        .child(text("Tool calls").font(sans(rem(0.875), 500.0, 1.43)).color(t.text).mb(rem(0.25)))
        .child(
            choice(
                t,
                "export-tools-line",
                o.tools == ExportToolMode::Line,
                true,
                "One line each",
                Some("\u{201c}Run bun test upload\u{201d}"),
            )
            .on_press(|s: &mut State| s.export_set_tools(ExportToolMode::Line)),
        )
        .child(
            choice(t, "export-tools-output", o.tools == ExportToolMode::Output, true, "With output", Some("Commands, results, diffs"))
                .on_press(|s: &mut State| s.export_set_tools(ExportToolMode::Output)),
        )
        .child(
            choice(t, "export-tools-off", o.tools == ExportToolMode::Off, true, "Leave out", None)
                .on_press(|s: &mut State| s.export_set_tools(ExportToolMode::Off)),
        );
    c = c.child(tools);
    let available = d.thinking_available();
    let thinking = choice(
        t,
        "export-thinking",
        o.thinking && available,
        false,
        "Thinking",
        Some(if available { "The model's notes, as quotes" } else { "This session has no thinking to include." }),
    );
    c = c.child(if available {
        thinking.on_press(|s: &mut State| s.export_toggle_thinking())
    } else {
        thinking.opacity(0.6).disabled(true).cursor(Cursor::Default)
    });
    c = c
        .child(
            choice(t, "export-paths", o.shorten_paths, false, "Shorten folder paths", Some("Relative to the session's folder"))
                .on_press(|s: &mut State| s.export_toggle_paths()),
        )
        .child(choice(t, "export-meta", o.meta, false, "Times and model", None).on_press(|s: &mut State| s.export_toggle_meta()));
    if o.tools == ExportToolMode::Output {
        c = c.child(note(
            "export-output-warning",
            "Tool output can hold keys and tokens. Fleet does not remove them. Read the file before you share it.".into(),
            t.border,
            t.text,
            Some(t.main_bg),
        ));
    }
    if let Some(words) = d.key_lines_note() {
        c = c.child(note("export-key-lines", words, t.error.alpha(0.4), t.error_text, None));
    }
    c
}

// ---- the file -------------------------------------------------------------------------------------------------------

/// The Markdown as text, with each line that looks like a key washed in the error colour (`<mark class="bg-error/20
/// text-error-text">`). The text is unchanged.
fn source(t: &Theme, markdown: &str) -> El {
    let mut spans = Vec::new();
    let mut at = 0;
    for line in markdown.split('\n') {
        if looks_like_key(line) {
            spans.push(Span::new(at, at + line.len()).color(t.error_text).bg(t.error.alpha(0.2), 0.0));
        }
        at += line.len() + 1;
    }
    rich(markdown.to_string(), spans).key("export-source").font(mono(rem(0.75), 400.0, 1.33)).color(t.text).wrap().w_full()
}

fn spinner(t: &Theme) -> El {
    static FADE: [(f32, f32); 3] = [(0.0, 1.0), (0.5, 0.35), (1.0, 1.0)];
    icon(&LOADER_CIRCLE, rem(1.0)).color(t.muted).cycle(Cycle { period: 1.2, phase: 0.0, stops: &FADE, still: 1.0, fps: 30.0 })
}

/// `ErrorNote`: the red box with the way to try again.
fn error_note(t: &Theme, key: &str, message: &str) -> El {
    let retry =
        button(t, Variant::Outline, Size::Sm, None, "Try again").key("export-retry").on_press(|s: &mut State| s.export_load()).mt(rem(0.5));
    row()
        .key(key.to_string())
        .role("alert")
        .w_full()
        .gap(rem(0.5))
        .items_start()
        .px(rem(0.75))
        .py(rem(0.5))
        .radius(RADIUS_CARD)
        .border(1.0, t.error.alpha(0.3))
        .border_layout()
        .fill(t.error.alpha(0.1))
        .child(icon(&CIRCLE_ALERT, 16.0).color(t.error_text).mt(1.0))
        .child(
            col()
                .shrink()
                .min_w(0.0)
                .child(text(message.to_string()).font(sans(rem(0.875), 400.0, 1.43)).color(t.error_text).wrap())
                .child(row().child(retry)),
        )
}

fn pane(t: &Theme, d: &ExportDialog) -> El {
    let inner =
        match (&d.load, &d.built) {
            (Load::Failed(why), _) => error_note(t, "export-error", why),
            (_, Some(built)) if d.pane == Pane::Source => source(t, &built.markdown),
            (_, Some(built)) => {
                // The renderer in Fleet's dialog draws smaller than the conversation's: 13px text, 16px and 14.5px headings.
                let style = super::prose::style(t)
                    .body_font(sans(13.0, 400.0, 1.6))
                    .heading_font(1, sans(17.0, 600.0, 1.3))
                    .heading_font(2, sans(15.0, 600.0, 1.3))
                    .heading_font(3, sans(14.0, 600.0, 1.3))
                    .heading_font(4, sans(13.0, 600.0, 1.3));
                col().key("export-rendered").w_full().child(super::prose::prose(&style, "export-rendered-md", &built.markdown, false))
            }
            (load, None) => {
                let n = if let Load::Loading(n) = load { *n } else { 0 };
                row().key("export-loading").gap(rem(0.5)).center_y().child(spinner(t)).child(
                    text(format!("Loading the conversation\u{2026} {n} messages")).font(sans(rem(0.875), 400.0, 1.43)).color(t.muted),
                )
            }
        };
    col().grow(1.0).min_h(0.0).child(
        col().w_full().padding(rem(0.75)).child(inner).scroll(ScrollSpec { stick_bottom: false }).key("export-body").grow(1.0).min_h(0.0),
    )
}

fn toolbar(t: &Theme, d: &ExportDialog) -> El {
    let tab = |key: &'static str, label: &str, on: bool, to: Pane| {
        button(t, Variant::Filter { active: on }, Size::Sm, None, label).key(key).on_press(move |s: &mut State| s.export_set_pane(to))
    };
    let mut r = row()
        .w_full()
        .gap(rem(0.5))
        .center_y()
        .px(rem(0.75))
        .py(rem(0.5))
        .border_sides([0.0, 0.0, 1.0, 0.0], t.border)
        .child(
            row()
                .gap(rem(0.25))
                .role("group")
                .label("Preview")
                .child(tab("export-pane-source", "Markdown", d.pane == Pane::Source, Pane::Source))
                .child(tab("export-pane-preview", "Preview", d.pane == Pane::Preview, Pane::Preview)),
        )
        .child(el().grow(1.0));
    if let Some(words) = d.size_label() {
        r = r.child(text(words).key("export-size").font(sans(rem(0.75), 400.0, 1.33)).color(t.muted).mr(rem(2.0)));
    }
    r
}

fn footer(t: &Theme, d: &ExportDialog) -> El {
    let ready = d.built.is_some();
    let copied = d.copied_until.is_some();
    let save = button(t, Variant::Outline, Size::Sm, Some(&DOWNLOAD), "Save .md").key("export-save");
    let copy = button(t, Variant::Default, Size::Sm, Some(if copied { &CHECK } else { &COPY }), if copied { "Copied" } else { "Copy" })
        .key("export-copy");
    let (save, copy) = if ready {
        (save.on_press_tx(|s: &mut State, tx| s.export_save(tx)), copy.on_press_tx(|s: &mut State, tx| s.export_copy(tx)))
    } else {
        (disabled(save), disabled(copy))
    };
    let mut r = row()
        .w_full()
        .gap(rem(0.5))
        .center_y()
        .px(rem(0.75))
        .py(rem(0.5))
        .border_sides([1.0, 0.0, 0.0, 0.0], t.border)
        .child(text(d.file_name()).key("export-file-name").font(mono(rem(0.75), 400.0, 1.33)).color(t.muted).shrink().min_w(0.0));
    if let Some(why) = &d.action_error {
        r = r.child(
            row()
                .key("export-action-error")
                .role("alert")
                .gap(rem(0.5))
                .items_start()
                .shrink()
                .min_w(0.0)
                .child(icon(&CIRCLE_ALERT, 14.0).color(t.error_text).mt(1.0))
                .child(text(why.clone()).font(sans(rem(0.75), 400.0, 1.33)).color(t.error_text).wrap()),
        );
    }
    r.child(el().grow(1.0)).child(save).child(copy)
}

fn preview(t: &Theme, d: &ExportDialog) -> El {
    col().grow(1.0).h_full().min_w(0.0).min_h(0.0).child(toolbar(t, d)).child(pane(t, d)).child(footer(t, d))
}
