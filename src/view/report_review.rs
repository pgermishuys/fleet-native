//! Help → Report a problem, steps 2 and 3: Review (the files, the title, what was replaced, the preview) and Sent. Ports the
//! `report__review` and `report__sent` parts of `components/report/ProblemReportDialog.vue`, with the Review and Sent footers.
//! The window, header and Describe are `view/report.rs`; the data is `fleet::report` and `fleet::report_form`.
use super::report::{CHROME, button, button_with, field};
use crate::State;
use crate::fleet::report::{Busy, Prepared, Stage, count_labels, split_labels, summarize_replacements};
use crate::fleet::report_form::{ReviewFile, SHOT_KEY};
use crate::icons::{CHECK, DOWNLOAD, FILE_TEXT, IMAGE, TRIANGLE_ALERT, X};
use crate::theme::{RADIUS_BTN, Theme, mono, sans};
use crate::ui::confirm::spinner;
use crate::ui::controls::switch;
use rune::{Align, Cursor, El, ScrollSpec, Span, SurfaceFit, Tx, col, el, icon, rich, row, text};

/// The review grid's first column (`180px minmax(0, 1fr)`).
const FILES: f32 = 180.0;
/// What the main column spends above the preview: the title row, the bar, and a problem line each.
const ABOVE: f32 = 92.0;
const PROBLEM: f32 = 21.0;

// ---- Review ---------------------------------------------------------------------------------------------------------------

fn file_row(t: &Theme, key: &str, name: &str, on: bool, icon_of: &'static rune::Icon, count: usize, to: ReviewFile) -> El {
    let mut r = row()
        .key(key.to_string())
        .role("button")
        .label(name.to_string())
        .w_full()
        .gap(8.0)
        .px(8.0)
        .py(6.0)
        .center_y()
        .radius(7.0)
        .cursor(Cursor::Pointer)
        .focus_ring(2.0, 2.0, t.accent)
        .on_press(move |s: &mut State| s.report_form.file = to)
        .child(icon(icon_of, 14.0).color(if on { t.text } else { t.muted }))
        .child(text(name.to_string()).font(sans(12.5, 400.0, 1.4)).color(if on { t.text } else { t.muted }));
    r = if on { r.fill(t.card_bg) } else { r.hover(t.wash(4.0)) };
    if count > 0 {
        r = r.child(el().grow(1.0)).child(
            el().px(6.0)
                .radius(999.0)
                .fill(t.accent_dim)
                .child(text(count.to_string()).key(format!("{key}-count")).font(sans(10.5, 400.0, 1.4)).color(t.text)),
        );
    }
    r
}

fn files(s: &State, t: &Theme, prepared: &Prepared) -> El {
    let f = &s.report_form;
    let mut c = col()
        .key("report-files")
        .role("list")
        .label("Report files")
        .w(FILES)
        .gap(2.0)
        .px(8.0)
        .py(12.0)
        .border_sides([0.0, 1.0, 0.0, 0.0], t.border)
        .fill(t.mix(t.main_bg, t.panel_bg, 0.45))
        .child(file_row(
            t,
            "report-file-report",
            "report.md",
            f.file == ReviewFile::Report,
            &FILE_TEXT,
            count_labels(Some(&prepared.body)),
            ReviewFile::Report,
        ));
    if prepared.log.is_some() {
        let n = count_labels(prepared.log.as_deref());
        c = c.child(file_row(t, "report-file-log", "fleet.log", f.file == ReviewFile::Log, &FILE_TEXT, n, ReviewFile::Log));
    }
    if f.has_screenshot() {
        c = c.child(file_row(
            t,
            "report-file-screenshot",
            "screenshot",
            f.file == ReviewFile::Screenshot,
            &IMAGE,
            0,
            ReviewFile::Screenshot,
        ));
    }
    c
}

/// "2 private details replaced · 1 folder, 1 user name", and the switch; or "No private details found.".
fn bar(s: &State, t: &Theme, prepared: &Prepared) -> El {
    let n = prepared.replacements.len();
    let mut b = row().w_full().flex_wrap().gap(8.0).px(16.0).py(10.0).center_y().border_sides([0.0, 0.0, 1.0, 0.0], t.border);
    if n == 0 {
        return b.child(text("No private details found.".to_string()).key("report-replaced").font(sans(12.0, 400.0, 1.4)).color(t.muted));
    }
    let head = format!("{n} private detail{} replaced", if n == 1 { "" } else { "s" });
    let words = format!("{head} · {}", summarize_replacements(&prepared.replacements));
    let spans = vec![Span::new(0, head.len()).weight(600.0).color(t.text)];
    b = b.child(rich(words, spans).key("report-replaced").font(sans(12.0, 400.0, 1.4)).color(t.muted));
    if s.report_form.file != ReviewFile::Screenshot {
        let on = s.report_form.show_removed;
        b = b.child(el().grow(1.0)).child(
            row()
                .key("report-show-removed")
                .role("switch")
                .label("Show what was removed")
                .checked(on)
                .gap(8.0)
                .center_y()
                .cursor(Cursor::Pointer)
                .focus_ring(2.0, 2.0, t.accent)
                .on_press(|s: &mut State| s.report_form.show_removed = !s.report_form.show_removed)
                .child(switch(t, on))
                .child(text("Show what was removed".to_string()).font(sans(12.0, 400.0, 1.4)).color(t.muted)),
        );
    }
    b
}

fn problems(t: &Theme, prepared: &Prepared) -> Option<El> {
    if prepared.problems.is_empty() {
        return None;
    }
    Some(col().key("report-problems").w_full().gap(2.0).px(16.0).py(8.0).border_sides([0.0, 0.0, 1.0, 0.0], t.border).children(
        prepared.problems.iter().map(|(item, why)| {
            row()
                .gap(6.0)
                .center_y()
                .child(icon(&TRIANGLE_ALERT, 13.0).color(t.status_waiting_text))
                .child(text(format!("Left out: {item}. {why}")).font(sans(12.0, 400.0, 1.4)).color(t.status_waiting_text))
        }),
    ))
}

/// The text of the file with each `‹kind-n›` label on `--accent-dim`; with "Show what was removed" the original is struck in
/// the error colour in front of it. Returns what to draw and the struck byte ranges.
type Strikes = Vec<(usize, usize, Option<rune::Color>)>;

fn preview_text(s: &State, t: &Theme, prepared: &Prepared) -> (String, Vec<Span>, Strikes) {
    let source = if s.report_form.file == ReviewFile::Log { prepared.log.as_deref().unwrap_or("") } else { prepared.body.as_str() };
    let (mut out, mut spans, mut strikes) = (String::new(), Vec::new(), Vec::new());
    for run in split_labels(source) {
        if !run.label {
            out.push_str(&run.text);
            continue;
        }
        let was = prepared.replacements.iter().find(|r| r.label == run.text).map(|r| r.shown.as_str());
        if let (true, Some(was)) = (s.report_form.show_removed, was.filter(|w| !w.is_empty())) {
            let at = out.len();
            out.push_str(was);
            spans.push(Span::new(at, out.len()).color(t.error_text).bg(t.accent_dim, 3.0));
            strikes.push((at, out.len(), Some(t.error.alpha(0.7))));
            out.push(' ');
        }
        let at = out.len();
        out.push_str(&run.text);
        spans.push(Span::new(at, out.len()).color(t.text).bg(t.accent_dim, 3.0));
    }
    (out, spans, strikes)
}

fn preview(s: &State, t: &Theme, prepared: &Prepared, height: f32) -> El {
    let (words, spans, strikes) = preview_text(s, t, prepared);
    let body = rich(words, spans).key("report-preview").font(mono(11.5, 400.0, 1.65)).color(t.text).wrap().w_full().strikes(strikes);
    col()
        .key("report-preview-box")
        .w_full()
        .min_h(220.0_f32.min(height))
        .max_h(height)
        .px(16.0)
        .py(14.0)
        .scroll(ScrollSpec { stick_bottom: false })
        .child(body)
}

/// The screenshot as it will be sent, at most 340 high, and the note with Remove screenshot.
fn shot_view(s: &State, t: &Theme) -> El {
    let f = &s.report_form;
    let mut view = col().key("report-shotview").w_full().gap(12.0).padding(16.0).items_center();
    if let (Some(shot), Some((image, _))) = (&f.screenshot, s.images.ready(SHOT_KEY)) {
        let (w, h) = (shot.width as f32, shot.height as f32);
        let scale = (606.0 / w).min(340.0 / h).min(1.0);
        view = view.child(
            el().key("report-shot")
                .role("img")
                .label("The screenshot that will be sent")
                .size((w * scale).round(), (h * scale).round())
                .radius(RADIUS_BTN)
                .border(1.0, t.border)
                .border_layout()
                .pixels_fit(image.pixels(), SurfaceFit::Fill),
        );
    }
    view.child(
        row()
            .w_full()
            .max_w(520.0)
            .gap(12.0)
            .center_y()
            .child(
                text(
                    "Not changed: anything on screen, such as session and folder names, is visible to whoever reads the report."
                        .to_string(),
                )
                .font(sans(12.0, 400.0, 1.4))
                .color(t.muted)
                .wrap()
                .grow(1.0)
                .min_w(0.0),
            )
            .child(
                button_with(t, "report-remove-screenshot", "Remove screenshot", false, false, false, Some(icon(&X, 13.0).color(t.text)))
                    .on_press(|s: &mut State| s.report_form.remove_screenshot()),
            ),
    )
}

pub fn review(s: &State, t: &Theme, window: f32) -> El {
    let Some(prepared) = s.report.prepared.as_ref() else { return el() };
    let spare = window - CHROME - if s.report.error.is_some() { 24.0 } else { 0.0 };
    let above = ABOVE + PROBLEM * prepared.problems.len() as f32;
    let title = row()
        .w_full()
        .gap(10.0)
        .px(16.0)
        .pt(10.0)
        .center_y()
        .child(text("Title".to_string()).font(sans(11.5, 400.0, 1.4)).color(t.muted))
        .child(field(t, "report-title", &s.report_form.title, "", 1).grow(1.0).on_edit(|s: &mut State| &mut s.report_form.title));
    let mut main = col().key("report-main").grow(1.0).min_w(0.0).child(title).child(bar(s, t, prepared));
    if let Some(p) = problems(t, prepared) {
        main = main.child(p);
    }
    main = main.child(if s.report_form.file == ReviewFile::Screenshot && s.report_form.has_screenshot() {
        shot_view(s, t)
    } else {
        preview(s, t, prepared, (spare - above).max(120.0))
    });
    row().key("report-review").w_full().max_h(spare).child(files(s, t, prepared)).child(main)
}

// ---- Sent -----------------------------------------------------------------------------------------------------------------

pub fn sent(s: &State, t: &Theme) -> El {
    let id = s.report.sent_id.clone().unwrap_or_default();
    let words = |w: String| text(w).font(sans(13.0, 400.0, 1.4)).color(t.muted).wrap().w_full().align(Align::Center);
    let lead = "Only the Fleet maintainers can see it. If you talk to us about it, mention ";
    let line = format!("{lead}{id}.");
    let spans = vec![Span::new(lead.len(), lead.len() + id.len()).mono().color(t.text).bg(t.card_bg, 3.0)];
    let mut c = col()
        .key("report-sent")
        .w_full()
        .gap(10.0)
        .items_center()
        .pt(32.0)
        .px(24.0)
        .pb(20.0)
        .child(el().size(34.0, 34.0).radius(17.0).fill(t.running.alpha(0.15)).center().child(icon(&CHECK, 18.0).color(t.running_text)))
        .child(text("Report sent".to_string()).font(sans(16.0, 400.0, 1.4)).color(t.text))
        .child(col().max_w(290.0).items_center().child(
            rich(line, spans).key("report-sent-id").font(sans(13.0, 400.0, 1.4)).color(t.muted).wrap().w_full().align(Align::Center),
        ));
    let contact = s.report_form.contact.text.trim();
    if !contact.is_empty() {
        c = c.child(col().max_w(290.0).items_center().child(
            words(format!("You left a contact, so a reply goes to {contact}.")).key("report-sent-contact").font(sans(12.0, 400.0, 1.4)),
        ));
    }
    c
}

// ---- the footers ----------------------------------------------------------------------------------------------------------

fn note(t: &Theme, key: &str, words: &str) -> El {
    text(words.to_string()).key(key.to_string()).font(sans(12.0, 400.0, 1.4)).color(t.muted).truncate().grow(1.0).min_w(0.0)
}

pub fn review_foot(s: &State, t: &Theme, buttons: El) -> El {
    let busy = s.report.busy;
    let can_send = s.report.prepared.as_ref().is_some_and(|p| p.can_send);
    let save_lead = if busy == Some(Busy::Save) { spinner(t.text) } else { icon(&DOWNLOAD, 14.0).color(t.text) };
    let save = button_with(t, "report-save", "Save as file", false, false, busy.is_some(), Some(save_lead));
    let send_lead = (busy == Some(Busy::Send)).then(|| spinner(t.primary_fg));
    let send = button_with(t, "report-send", "Send report", true, false, !can_send || busy.is_some(), send_lead);
    buttons
        .child(button(t, "report-back", "Back", false, true, busy.is_some()).on_press(|s: &mut State| s.report_back()))
        .child(note(
            t,
            "report-note",
            if can_send { "Only the Fleet maintainers will see this." } else { "Sending is off on this Fleet. Save it as a file instead." },
        ))
        .child(save.on_press_tx(|s: &mut State, tx: &mut Tx| s.report_save_reviewed(tx)))
        .child(send.on_press(|s: &mut State| s.report_send_reviewed()))
}

pub fn sent_foot(s: &State, t: &Theme, buttons: El) -> El {
    let copied = s.report_form.copied;
    debug_assert!(s.report.stage == Stage::Sent);
    buttons
        .child(el().grow(1.0))
        .child(
            button(t, "report-copy-id", if copied { "Copied" } else { "Copy report id" }, false, false, false)
                .on_press_tx(|s: &mut State, tx: &mut Tx| s.report_copy_id(tx)),
        )
        .child(button(t, "report-done", "Done", true, false, false).on_press(|s: &mut State| s.report_close()))
}
