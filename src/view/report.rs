//! Help → Report a problem: the window and its Describe step. Ports `components/report/ProblemReportDialog.vue`'s header,
//! Describe body (the kind, the three fields, the screenshot, the attached parts) and footer. Review and Sent are piece R's
//! (`view/report_review.rs`); until it lands those stages show a line saying so. The data is `fleet::report` and
//! `fleet::report_form`; this file only draws it. Sizes are the component's CSS pixels, not rem.
use crate::State;
use crate::fleet::report::{Busy, KINDS, Kind, Stage};
use crate::fleet::report_form::{ITEMS, Item, ReportForm, SHOT_KEY};
use crate::icons::{IMAGE, SHIELD_CHECK, X};
use crate::theme::{RADIUS_BTN, RADIUS_PANEL, Theme, sans};
use crate::ui::dialog::{lift, overlay};
use crate::ui::overlay::centred;
use rune::{Cursor, El, ScrollSpec, Spring, SurfaceFit, Transition, Vec2, col, el, icon, row, text, text_input};

const KEY: &str = "report";
/// `sm:max-w-[820px]`, and the window less 16 px a side.
const WIDTH: f32 = 820.0;
/// `min(720px, 100vh - 32px)`.
const HEIGHT: f32 = 720.0;
const ASIDE: f32 = 290.0;

fn window(s: &State) -> (f32, f32) {
    let (w, h) = s.ui.sel.window;
    if w > 0.0 && h > 0.0 { (w, h) } else { (1280.0, 760.0) }
}

/// The window and its scrim, to mount over the app; `None` while the report is closed.
pub fn dialog(s: &State, t: &Theme) -> Option<Vec<El>> {
    if !s.report.open {
        return None;
    }
    let (w, h) = window(s);
    let sending = s.report.busy == Some(Busy::Send);
    let mut panel = col()
        .key("report-dialog")
        .role("dialog")
        .label("Report a problem")
        .w(WIDTH.min(w - 32.0))
        .max_h(HEIGHT.min(h - 32.0))
        .fill(t.panel_bg)
        .border(1.0, t.border)
        .border_layout()
        .radius(RADIUS_PANEL)
        .child(head(s, t));
    panel = match s.report.stage {
        Stage::Describe => panel.child(describe(s, t, HEIGHT.min(h - 32.0))),
        Stage::Review => panel.child(placeholder(t, "Review comes with the next piece of this dialog.")),
        Stage::Sent => panel.child(placeholder(t, "Sent")),
    };
    panel = panel.child(foot(s, t));
    // Esc and the overlay close it, except while it is sending.
    let close = move |s: &mut State| s.report_close();
    let panel = lift(t, panel)
        .transition(Transition::scale_anchor(0.95, Vec2::new(0.5, 0.5)))
        .token_motion(Spring::new(0.2, 0.0))
        .modal(!sending, close);
    Some(vec![overlay(KEY), centred("report-frame", panel)])
}

fn placeholder(t: &Theme, words: &str) -> El {
    col().w_full().padding(20.0).child(text(words.to_string()).font(sans(12.5, 400.0, 1.4)).color(t.muted))
}

// ---- the header -----------------------------------------------------------------------------------------------------

fn step(t: &Theme, words: &str, on: bool) -> El {
    let mut pill = row().px(8.0).py(2.0).radius(999.0).child(text(words.to_string()).font(sans(11.5, 400.0, 1.4)).color(if on {
        t.text
    } else {
        t.muted
    }));
    if on {
        pill = pill.fill(t.accent_dim);
    }
    pill
}

fn head(s: &State, t: &Theme) -> El {
    let sub = match s.report.stage {
        Stage::Describe => "Nothing leaves your machine until you send it.",
        Stage::Review => "This is everything that will be sent.",
        Stage::Sent => "Sent to the Fleet maintainers.",
    };
    let heading = col().grow(1.0).gap(2.0).min_w(0.0).child(text("Report a problem").font(sans(15.0, 600.0, 1.2)).color(t.text)).child(
        row()
            .gap(6.0)
            .center_y()
            .child(icon(&SHIELD_CHECK, 13.0).color(t.running))
            .child(text(sub.to_string()).font(sans(12.5, 400.0, 1.2)).color(t.muted)),
    );
    let mut head = row()
        .key("report-head")
        .w_full()
        .gap(12.0)
        .items_start()
        .pl(20.0)
        .pt(16.0)
        .pr(20.0)
        .pb(12.0)
        .border_sides([0.0, 0.0, 1.0, 0.0], t.border)
        .child(heading);
    if s.report.stage != Stage::Sent {
        head = head.child(
            row()
                .key("report-steps")
                .role("list")
                .label("Steps")
                .gap(6.0)
                .center_y()
                .child(step(t, "1 Describe", s.report.stage == Stage::Describe))
                .child(text("›".to_string()).font(sans(11.5, 400.0, 1.4)).color(t.muted))
                .child(step(t, "2 Review", s.report.stage == Stage::Review)),
        );
    }
    head.child(
        el().key("report-close")
            .role("button")
            .label("Close")
            .size(26.0, 26.0)
            .radius(RADIUS_BTN)
            .center()
            .cursor(Cursor::Pointer)
            .hover(t.wash(6.0))
            .focus_ring(2.0, 2.0, t.accent)
            .on_press(|s: &mut State| s.report_close())
            .child(icon(&X, 16.0).color(t.muted)),
    )
}

// ---- Describe -------------------------------------------------------------------------------------------------------

/// A field's label (`.report__field > span`): 11.5/500 muted, and an "(optional)…" in its own words after it.
fn label(t: &Theme, words: &str, optional: Option<&str>) -> El {
    let mut l = row().gap(4.0).child(text(words.to_string()).font(sans(11.5, 500.0, 1.4)).color(t.muted));
    if let Some(o) = optional {
        l = l.child(text(o.to_string()).font(sans(11.5, 500.0, 1.4)).color(t.muted.alpha(0.75)));
    }
    l
}

/// The textarea and the two inputs (`.report__field textarea, input`): 1 px border, `--card-bg`, 13/1.45, padding 8 10.
fn field(t: &Theme, key: &str, edit: &rune::TextEdit, placeholder: &str, rows: usize) -> El {
    let f = text_input(edit, sans(13.0, 400.0, 1.45))
        .key(key.to_string())
        .placeholder(placeholder.to_string(), t.muted)
        .label(placeholder.to_string())
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .w_full()
        .px(10.0)
        .py(8.0)
        .fill(t.card_bg)
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .focus_ring(1.0, 0.0, t.accent.alpha(0.6));
    if rows > 1 { f.min_h(rows as f32 * 13.0 * 1.45 + 18.0) } else { f.single_line().h(13.0 * 1.45 + 18.0) }
}

fn kind_pill(t: &Theme, kind: Kind, on: bool) -> El {
    let mut p = row()
        .key(format!("report-kind-{}", kind.id()))
        .role("radio")
        .label(kind.label().to_string())
        .checked(on)
        .px(10.0)
        .py(4.0)
        .radius(999.0)
        .border(1.0, if on { rune::Color::TRANSPARENT } else { t.border })
        .border_layout()
        .cursor(Cursor::Pointer)
        .focus_ring(2.0, 2.0, t.accent)
        .on_press(move |s: &mut State| s.report_form.kind = kind)
        .child(text(kind.label().to_string()).font(sans(12.0, 400.0, 1.4)).color(if on { t.text } else { t.muted }));
    if on {
        p = p.fill(t.accent_dim);
    }
    p
}

/// A ghost `.report__button`: 12.5/500, padding 6 12, a 1 px border, `--radius-btn`.
fn button(t: &Theme, key: &str, words: &str, primary: bool, ghost: bool, disabled: bool) -> El {
    let (fill, border, ink) = if primary {
        (t.accent, rune::Color::TRANSPARENT, t.primary_fg)
    } else if ghost {
        (rune::Color::TRANSPARENT, t.border, t.text)
    } else {
        (t.card_bg, t.border, t.text)
    };
    let b = row()
        .key(key.to_string())
        .role("button")
        .label(words.to_string())
        .px(12.0)
        .py(6.0)
        .gap(6.0)
        .center_y()
        .fill(fill)
        .border(1.0, border)
        .border_layout()
        .radius(RADIUS_BTN)
        .focus_ring(2.0, 2.0, t.accent)
        .child(text(words.to_string()).font(sans(12.5, 500.0, 1.4)).color(ink));
    if disabled { b.disabled(true).opacity(0.5) } else { b.cursor(Cursor::Pointer).hover(t.wash(8.0)) }
}

fn thumb(s: &State, t: &Theme, form: &ReportForm) -> El {
    let mut frame =
        el().key("report-thumb").w(200.0).h(120.0).radius(RADIUS_BTN).border(1.0, t.border).border_layout().fill(t.main_bg).center();
    match (&form.screenshot, s.images.ready(SHOT_KEY)) {
        (Some(_), Some((image, _))) => {
            frame = frame.role("img").label("Screenshot of Fleet when you opened the report").pixels_fit(image.pixels(), SurfaceFit::Fill);
        }
        (Some(_), None) => {}
        (None, _) => frame = frame.child(icon(&IMAGE, 18.0).color(t.muted)),
    }
    frame
}

fn screenshot(s: &State, t: &Theme, form: &ReportForm) -> El {
    let words = |w: &str| text(w.to_string()).font(sans(12.0, 400.0, 1.4)).color(t.muted).wrap().w_full();
    let mut meta = col().grow(1.0).min_w(0.0).justify_center().items_start().gap(6.0);
    match &form.screenshot {
        Some(shot) => {
            meta = meta
                .child(words(&format!("Taken {} · {}×{}", shot.taken, shot.width, shot.height)).key("report-shot-taken"))
                .child(words("Shows your screen as it is. You can look at it and remove it on the next step."));
        }
        None => meta = meta.child(words("Couldn't take a screenshot. The report works without one.").key("report-shot-none")),
    }
    let retake = button(t, "report-retake", if form.capturing { "Taking…" } else { "Retake" }, false, true, form.capturing)
        .on_press(|s: &mut State| s.report_retake());
    row().gap(12.0).child(thumb(s, t, form)).child(meta.child(retake))
}

fn left(s: &State, t: &Theme) -> El {
    let form = &s.report_form;
    let kinds =
        row().role("radiogroup").label("Kind").flex_wrap().gap(6.0).children(KINDS.iter().map(|k| kind_pill(t, *k, form.kind == *k)));
    col()
        .grow(1.0)
        .min_w(0.0)
        .gap(12.0)
        .pl(20.0)
        .pt(16.0)
        .pr(20.0)
        .pb(16.0)
        .child(kinds)
        .child(
            col().gap(5.0).child(label(t, "What happened?", None)).child(
                field(t, "report-description", &form.description, "What you were doing, and what went wrong.", 4)
                    .on_edit(|s: &mut State| &mut s.report_form.description),
            ),
        )
        .child(
            col()
                .gap(5.0)
                .child(label(t, "What did you expect?", Some("(optional)")))
                .child(field(t, "report-expected", &form.expected, "", 1).on_edit(|s: &mut State| &mut s.report_form.expected)),
        )
        .child(col().gap(5.0).child(label(t, "Screenshot", None)).child(screenshot(s, t, form)))
        .child(
            col()
                .gap(5.0)
                .child(label(t, "Want a reply?", Some("(optional: GitHub name or email)")))
                .child(field(t, "report-contact", &form.contact, "@your-name", 1).on_edit(|s: &mut State| &mut s.report_form.contact)),
        )
}

/// One `.report__item`: a 14 px box, its name (struck through when off) and what it holds.
fn item(t: &Theme, form: &ReportForm, it: Item) -> El {
    let on = form.is_on(it);
    let usable = it != Item::Screenshot || form.screenshot.is_some();
    let counts = on && usable;
    let name = text(it.name().to_string()).font(sans(12.5, 400.0, 1.4)).color(if counts { t.text } else { t.muted }).strike(
        0,
        if counts { 0 } else { it.name().len() },
        None,
    );
    let mark = el().size(14.0, 14.0).mt(2.0).radius(3.0).border(1.0, if counts { t.accent } else { t.muted }).border_layout();
    let mark = if counts { mark.fill(t.accent).center().child(icon(&crate::icons::CHECK, 11.0).color(t.primary_fg)) } else { mark };
    let r = row()
        .key(format!("report-include-{}", it.id()))
        .role("checkbox")
        .label(it.name().to_string())
        .checked(counts)
        .w_full()
        .gap(8.0)
        .padding(6.0)
        .radius(7.0)
        .items_start()
        .child(mark)
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .child(name)
                .child(text(it.detail().to_string()).font(sans(11.0, 400.0, 1.35)).color(t.muted).wrap().w_full()),
        );
    if usable {
        r.cursor(Cursor::Pointer).hover(t.wash(4.0)).focus_ring(2.0, 2.0, t.accent).on_press(move |s: &mut State| s.report_form.toggle(it))
    } else {
        r.disabled(true)
    }
}

fn aside(s: &State, t: &Theme) -> El {
    let form = &s.report_form;
    let head = row()
        .w_full()
        .justify_between()
        .mb(4.0)
        .child(text("Attached".to_string()).font(sans(12.5, 600.0, 1.4)).color(t.text))
        .child(text(format!("{} of {}", form.count(), ITEMS.len())).key("report-count").font(sans(11.0, 400.0, 1.4)).color(t.muted));
    let guard = row()
        .w_full()
        .gap(7.0)
        .items_start()
        .pt(10.0)
        .border_sides([1.0, 0.0, 0.0, 0.0], t.border)
        .child(el().mt(1.0).child(icon(&SHIELD_CHECK, 14.0).color(t.running)))
        .child(
            text("Folder paths, session names, branch names, tokens and email addresses in the text and the log are replaced before you review it. The screenshot isn't changed.".to_string())
                .font(sans(11.5, 400.0, 1.45))
                .color(t.muted)
                .wrap()
                .grow(1.0)
                .min_w(0.0),
        );
    col()
        .key("report-aside")
        .w(ASIDE)
        .gap(4.0)
        .pl(16.0)
        .pt(14.0)
        .pr(14.0)
        .pb(14.0)
        .border_sides([0.0, 0.0, 0.0, 1.0], t.border)
        .fill(t.mix(t.main_bg, t.panel_bg, 0.45))
        .child(head)
        .children(ITEMS.iter().map(|it| item(t, form, *it)))
        .child(el().grow(1.0).min_h(0.0))
        .child(guard)
}

/// What the header and the footer take of the window's height (68.5 and 55.5, and a line more for an error).
const CHROME: f32 = 126.0;

fn describe(s: &State, t: &Theme, window: f32) -> El {
    // The body scrolls inside what is left of the window's height, as `.report__body` does (`overflow: auto`). Flex can't
    // hand it the rest: the window's own height is its content's, so the limit is worked out here.
    let spare = window - CHROME - if s.report.error.is_some() { 24.0 } else { 0.0 };
    row().key("report-body").w_full().max_h(spare).scroll(ScrollSpec { stick_bottom: false }).child(left(s, t)).child(aside(s, t))
}

// ---- the footer -----------------------------------------------------------------------------------------------------

fn foot(s: &State, t: &Theme) -> El {
    let mut foot =
        col().key("report-foot").w_full().gap(8.0).pl(20.0).pt(12.0).pr(20.0).pb(12.0).border_sides([1.0, 0.0, 0.0, 0.0], t.border);
    if let Some(error) = &s.report.error {
        foot = foot
            .child(text(error.clone()).key("report-error").role("alert").font(sans(12.5, 400.0, 1.4)).color(t.error_text).wrap().w_full());
    }
    let buttons = row().w_full().gap(8.0).center_y();
    foot.child(match s.report.stage {
        Stage::Describe => describe_foot(s, t, buttons),
        _ => buttons,
    })
}

fn describe_foot(s: &State, t: &Theme, buttons: El) -> El {
    let about = match s.report_about() {
        Some((_, title)) => format!("About: {title}"),
        None => "Not about a session".to_string(),
    };
    let preparing = s.report.busy == Some(Busy::Prepare);
    let blocked = !s.report_form.can_review() || s.report.busy.is_some();
    let mut review = button(t, "report-review", "Review what's sent", true, false, blocked);
    if preparing {
        review = review.child(crate::ui::confirm::spinner(t.primary_fg));
    }
    buttons
        .child(text(about).key("report-about").font(sans(12.0, 400.0, 1.4)).color(t.muted).truncate().grow(1.0).min_w(0.0))
        .child(button(t, "report-cancel", "Cancel", false, true, false).on_press(|s: &mut State| s.report_close()))
        .child(review.on_press(|s: &mut State| s.report_review()))
}
