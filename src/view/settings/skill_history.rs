//! The skill History dialog (`skills/SkillHistoryDialog.vue`): the user's versions, newest first, each with its note, when it was
//! made and where from, then Fleet's own as the last row. Any row can be the one sessions get, and each compares with Fleet's text.
//! The time is `formatRelative` in words ("4 minutes ago"). Fleet shows the exact time as a tooltip on it; Rune has no tooltips, so
//! the relative time stands alone.
//! Sizes are the component's CSS pixels (11.5, 12.5, 44, 12 and 14), not the root font's.
use super::skill_editor::{text_link, window};
use crate::State;
use crate::fleet::settings::skill_editor::{Detail, Version};
use crate::fleet::settings::skill_history::{Busy, SkillHistory};
use crate::theme::{RADIUS_CARD, Theme, mono, sans};
use crate::ui::confirm::{error_note, spinner};
use crate::ui::dialog::{Dialog, dialog};
use crate::ui::rem;
use rune::{El, col, el, row, text};

/// `sm:max-w-2xl`.
const WIDTH: f32 = 588.0;
/// `max-h-[88dvh]`.
const HEIGHT_OF_WINDOW: f32 = 0.88;
const ACTION: f32 = 12.5;

fn title(t: &Theme, name: &str) -> El {
    row()
        .key("skill-history-title")
        .flex_wrap()
        .gap(rem(0.5))
        .center_y()
        .child(text(name.to_string()).font(mono(rem(1.125), 600.0, 1.0)).color(t.text))
        .child(text("history").font(sans(rem(1.125), 600.0, 1.0)).color(t.text))
}

/// `.skill-history__in-use`.
fn in_use(t: &Theme, key: String) -> El {
    row()
        .key(key)
        .label("In use")
        .px(7.0)
        .radius(999.0)
        .fill(t.accent_dim)
        .child(text("In use").font(sans(11.5, 600.0, 1.5)).color(t.accent_text))
}

/// One row: the number column and the text next to it.
fn line(t: &Theme, key: String, divided: bool, number: &str, muted: bool, content: El) -> El {
    let number = text(number.to_string()).font(mono(ACTION, 600.0, 1.6)).color(if muted { t.muted } else { t.text });
    let r = row().key(key).w_full().items_start().gap(12.0).px(14.0).py(12.0).child(el().w(44.0).child(number)).child(content);
    if divided { r.border_sides([1.0, 0.0, 0.0, 0.0], t.border) } else { r }
}

fn actions(children: Vec<El>) -> El {
    row().w_full().flex_wrap().center_y().gaps(4.0, 14.0).children(children)
}

/// A version's link, with the small spinner in front while it runs.
fn busy_link(t: &Theme, key: String, words: &str, spin: bool, off: bool, press: impl Fn(&mut State) + 'static) -> El {
    let l = text_link(t, key, words, ACTION, off, press);
    if spin { row().gap(4.0).center_y().child(spinner(t.accent_text)).child(l) } else { l }
}

fn version_row(s: &State, t: &Theme, h: &SkillHistory, d: &Detail, v: &Version, divided: bool) -> El {
    let n = v.number;
    let now = crate::view::message::now_ms(s);
    let offset = crate::conv::time::local_offset_secs();
    let words = match &v.note {
        Some(note) => text(format!("\u{201c}{note}\u{201d}")).font(sans(rem(0.875), 400.0, 1.43)).color(t.text),
        None => text("No note").font(sans(rem(0.875), 400.0, 1.43)).color(t.muted),
    };
    let mut when = crate::fleet::smart_links::format_checked_ago(Some(&v.created_at), now, offset).unwrap_or_default();
    if let Some(title) = &v.session_title {
        when = if when.is_empty() { format!("from {title}") } else { format!("{when} \u{b7} from {title}") };
    }
    let any = h.busy.is_some();
    let mut acts = Vec::new();
    if v.active {
        acts.push(in_use(t, format!("skill-version-in-use-{n}")));
    } else {
        acts.push(busy_link(
            t,
            format!("skill-version-use-{n}"),
            "Use this version",
            h.busy == Some(Busy::Version(n)),
            any,
            move |s: &mut State| s.skill_history_use(Some(n)),
        ));
    }
    let open = h.comparing.as_ref().filter(|c| c.number == n);
    acts.push(text_link(
        t,
        format!("skill-version-compare-{n}"),
        if open.is_some() { "Hide" } else { "Compare with Fleet's" },
        ACTION,
        false,
        move |s: &mut State| s.skill_history_compare(n),
    ));
    let mut body = col()
        .grow(1.0)
        .shrink()
        .min_w(0.0)
        .gap(3.5)
        .child(words.wrap().w_full())
        .child(text(when).key(format!("skill-version-when-{n}")).font(sans(rem(0.75), 400.0, 1.33)).color(t.muted).wrap().w_full());
    body = body.child(actions(acts));
    if let Some(c) = open {
        body = body.child(crate::view::skill_diff::skill_diff(
            t,
            s,
            &format!("skill-version-diff-{n}"),
            &d.fleet_content,
            &c.content,
            &format!("Fleet's \u{2192} v{n}"),
        ));
    }
    line(t, format!("skill-version-{n}"), divided, &format!("v{n}"), false, body)
}

fn fleet_row(t: &Theme, h: &SkillHistory, d: &Detail, divided: bool) -> El {
    let status = if d.version.is_none() {
        in_use(t, "skill-version-in-use-fleet".into())
    } else {
        busy_link(t, "skill-version-use-fleet".into(), "Use Fleet's", h.busy == Some(Busy::Fleet), h.busy.is_some(), |s: &mut State| {
            s.skill_history_use(None)
        })
    };
    let body = col()
        .grow(1.0)
        .shrink()
        .min_w(0.0)
        .gap(3.5)
        .child(text("Fleet's version, as this Fleet ships it.").font(sans(rem(0.875), 400.0, 1.43)).color(t.muted).wrap().w_full())
        .child(actions(vec![status]));
    line(t, "skill-version-fleet".into(), divided, "Fleet", true, body)
}

fn list(s: &State, t: &Theme, h: &SkillHistory, d: &Detail) -> El {
    let mut rows =
        col().key("skill-history-list").w_full().radius(RADIUS_CARD).border(1.0, t.border).border_layout().clip_rounded(RADIUS_CARD);
    for (i, v) in d.versions.iter().enumerate() {
        rows = rows.child(version_row(s, t, h, d, v, i > 0));
    }
    rows.child(fleet_row(t, h, d, !d.versions.is_empty()))
}

fn body(s: &State, t: &Theme, h: &SkillHistory) -> Vec<El> {
    let mut b = Vec::new();
    if h.loading() {
        b.push(super::skills_tools::loading(t, "skill-history-loading", "Loading the history\u{2026}"));
    }
    if let Some(d) = &h.detail {
        b.push(list(s, t, h, d));
    }
    if let Some(why) = &h.error {
        b.push(error_note(t, "skill-history-error", why, None));
    }
    b
}

/// The dialog and its scrim; `None` while it is closed.
pub fn view(s: &State, t: &Theme) -> Option<Vec<El>> {
    let h = &s.settings.history;
    if !h.open {
        return None;
    }
    let (w, win_h) = window(s);
    Some(dialog(
        t,
        Dialog {
            key: "skill-history",
            title: format!("{} history", h.name),
            title_el: Some(title(t, &h.name)),
            description: Some(
                "Your versions, newest first, with why each one changed. Sessions you start use the one marked In use.".into(),
            ),
            body: body(s, t, h),
            close_button: true,
            click_outside: true,
            dismiss: Some(|s: &mut State| s.skill_history_close()),
            width: Some(WIDTH.min(w - 32.0)),
            max_height: Some(win_h * HEIGHT_OF_WINDOW),
            ..Default::default()
        },
    ))
}
