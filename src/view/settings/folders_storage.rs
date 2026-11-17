//! Settings → Folders, the Storage card (`StorageSection.vue`): what Fleet's worktree checkouts take on the machine shown, the three
//! splits, and the preview-then-clean flow with its report. Counts come first, sizes fill in as each folder is measured.
use super::folders_locations::{pill, secondary};
use crate::State;
use crate::fleet::settings::storage::{Checkout, Sizes, StoragePage, is_are, noun, size_label, sum_of};
use crate::icons::CIRCLE_ALERT;
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, mono, sans};
use crate::ui::{confirm, rem};
use rune::{Align, El, Span, col, icon, rich, row, text};

const TEXT_SM: f32 = 12.25;

fn muted(t: &Theme, words: &str) -> El {
    text(words.to_string()).font(sans(rem(0.75), 400.0, 1.33)).color(t.muted).wrap().shrink().min_w(0.0)
}

/// Text with parts set in mono and/or bold: `(words, mono, bold)`.
fn mixed(parts: &[(&str, bool, bool)], font: rune::TextStyle, color: rune::Color) -> El {
    let (mut body, mut spans) = (String::new(), Vec::new());
    for (words, is_mono, bold) in parts {
        let at = body.len();
        body.push_str(words);
        if *is_mono || *bold {
            let mut span = Span::new(at, body.len());
            if *is_mono {
                span = span.mono().size(font.size);
            }
            if *bold {
                span = span.weight(700.0);
            }
            spans.push(span);
        }
    }
    rich(body, spans).font(font).color(color).wrap().shrink().min_w(0.0)
}

fn heading(t: &Theme, words: &str, key: &str) -> El {
    text(words.to_uppercase()).key(key.to_string()).font(sans(10.0, 600.0, 1.4).tracking(0.05 * 10.0)).color(t.muted)
}

fn pill_tag(t: &Theme, words: &str) -> El {
    row()
        .w(84.0)
        .center()
        .px(7.0)
        .radius(999.0)
        .border(1.0, t.border)
        .border_layout()
        .child(text(words.to_string()).font(mono(10.0, 400.0, 1.4)).color(t.muted))
}

/// One checkout of the preview or the report: the title line with its size, the reason under it. `tag` is the pill ahead of it.
fn entry(t: &Theme, key: String, first: bool, tag: Option<&str>, c: &Checkout, sizes: &Sizes) -> El {
    let title = text(format!("{} · {}", c.repository, c.title)).font(sans(TEXT_SM, 400.0, 1.43)).color(t.text).truncate();
    let words = col().grow(1.0).shrink().min_w(0.0).child(title).child(muted(t, &c.reason));
    let mut r = row().key(key).w_full().gap(10.5).items_start().py(7.0);
    if !first {
        r = r.border_sides([1.0, 0.0, 0.0, 0.0], t.border).border_layout();
    }
    if let Some(tag) = tag {
        r = r.child(pill_tag(t, tag).mt(1.0));
    }
    r.child(words).child(text(size_label(c, sizes)).font(mono(rem(0.75), 400.0, 1.33)).color(t.text).mt(2.0))
}

fn group(t: &Theme, key: &str, title: &str, tag: Option<&str>, list: &[&Checkout], sizes: &Sizes, top: f32) -> Option<El> {
    if list.is_empty() {
        return None;
    }
    let mut c = col().w_full().child(heading(t, title, &format!("{key}-title")).mt(top));
    for (i, ch) in list.iter().enumerate() {
        c = c.child(entry(t, format!("{key}-{i}"), i == 0, tag, ch, sizes));
    }
    Some(c)
}

fn totals(page: &StoragePage, t: &Theme) -> El {
    let all: Vec<&Checkout> = page.checkouts.iter().collect();
    let (_, label) = sum_of(&all, &page.sizes);
    let mut c = col().w_full().mt(17.5).child(
        mixed(
            &[(&label, true, false), (&format!(" in {}", noun(page.checkouts.len())), false, false)],
            sans(rem(1.5), 600.0, 1.33),
            t.text,
        )
        .key("storage-total"),
    );
    let mut boxes = row().w_full().mt(10.5).gap(10.5);
    for (key, words, list) in page.splits() {
        let (_, label) = sum_of(&list, &page.sizes);
        boxes = boxes.child(
            col()
                .key(format!("storage-split-{key}"))
                .grow(1.0)
                .w(0.0)
                .px(10.5)
                .py(7.0)
                .radius(RADIUS_BTN)
                .border(1.0, t.border)
                .border_layout()
                .fill(t.main_bg)
                .child(text(label).font(mono(TEXT_SM, 400.0, 1.43)).color(t.text))
                .child(muted(t, &format!("{} {words}", list.len()))),
        );
    }
    c = c.child(boxes).child(muted(t, "Sizes fill in as Fleet measures each folder. Only checkouts Fleet made are counted.").mt(7.0));
    c
}

fn always_kept(t: &Theme) -> El {
    let lead = "Always kept:";
    let body = format!(
        "{lead} Uncommitted changes · a session that is working · a folder another session uses · an open terminal · anything Fleet didn't make. The branch and the conversation always stay; the next message to the session puts the checkout back."
    );
    let spans = vec![Span::new(0, lead.len()).weight(500.0).color(t.text)];
    rich(body, spans).font(sans(rem(0.75), 400.0, 1.33)).color(t.muted).wrap().shrink().min_w(0.0).mt(14.0)
}

fn preview_panel(page: &StoragePage, t: &Theme) -> El {
    let (removable, kept, recent) = (page.removable(), page.kept(), page.recent());
    let (_, freed) = sum_of(&removable, &page.sizes);
    let mut tail = format!("{} would be kept", kept.len());
    if !recent.is_empty() {
        tail.push_str(&format!(", and {} more {} too recent", recent.len(), is_are(recent.len())));
    }
    let busy = page.cleaning;
    let lead = if busy { confirm::spinner(t.primary_fg) } else { row() };
    let label = if busy { format!("Removing {}…", noun(removable.len())) } else { format!("Remove {}", noun(removable.len())) };
    let confirm_button = pill(
        row().fill(t.accent),
        "storage-confirm",
        &label,
        busy || removable.is_empty(),
        lead,
        text(label.clone()).font(sans(TEXT_SM, 500.0, 1.43)).color(t.primary_fg),
        |s: &mut State| s.clean_storage(),
    );
    let cancel = pill(
        secondary(t),
        "storage-cancel",
        "Cancel",
        busy,
        row(),
        text("Cancel".to_string()).font(sans(TEXT_SM, 400.0, 1.43)).color(t.text),
        |s: &mut State| s.cancel_storage_preview(),
    );
    let mut c = col().key("storage-preview-panel").w_full().mt(10.5).child(mixed(
        &[
            ("Would remove ", false, false),
            (&noun(removable.len()), false, true),
            (" and free ", false, false),
            (&freed, true, true),
            (&format!(". {tail}."), false, false),
        ],
        sans(TEXT_SM, 400.0, 1.43),
        t.text,
    ));
    c = c.child(muted(t, "Nothing has been removed yet. Branches, commits and conversations stay."));
    c = c.children(group(t, "storage-remove", "Would remove", Some("will remove"), &removable, &page.sizes, 14.0));
    c = c.children(group(t, "storage-keep", "Would keep", Some("will keep"), &kept, &page.sizes, 14.0));
    c.child(row().w_full().mt(14.0).gap(7.0).child(confirm_button).child(cancel))
}

fn report_panel(page: &StoragePage, t: &Theme) -> Option<El> {
    let report = page.report.as_ref()?;
    let removed: Vec<&Checkout> = report.removed.iter().collect();
    let kept: Vec<&Checkout> = report.kept.iter().collect();
    let (_, freed) = sum_of(&removed, &page.report_sizes);
    let recent = page.recent().len();
    let mut line = format!("Removed {}. Kept {}", noun(removed.len()), kept.len());
    if recent > 0 {
        line.push_str(&format!(", and {recent} {} too recent", is_are(recent)));
    }
    line.push('.');
    let mut c = col()
        .key("storage-report")
        .w_full()
        .mt(10.5)
        .child(mixed(&[("Freed ", false, false), (&freed, true, false)], sans(TEXT_SM, 600.0, 1.43), t.text))
        .child(muted(t, &line));
    c = c.children(group(t, "storage-removed", "Removed", None, &removed, &page.report_sizes, 10.5));
    c = c.children(group(t, "storage-kept", "Kept", None, &kept, &page.report_sizes, 10.5));
    Some(
        c.child(
            muted(t, "Each removed session shows “Checkout removed”. Its checkout comes back on its next message, or with Restore now.")
                .mt(10.5),
        ),
    )
}

fn clean_box(page: &StoragePage, t: &Theme) -> El {
    let none = page.removable().is_empty();
    let mut head = row().w_full().flex_wrap().center_y().gap(10.5).child(text("Clean now").font(sans(TEXT_SM, 600.0, 1.43)).color(t.text));
    if !page.previewing {
        head = head.child(pill(
            secondary(t),
            "storage-preview",
            "Preview…",
            none,
            row(),
            text("Preview…".to_string()).font(sans(TEXT_SM, 400.0, 1.43)).color(t.text),
            |s: &mut State| s.preview_storage(),
        ));
    }
    let days = page.archived_days;
    if none {
        head = head.child(
            muted(t, &format!("Nothing to remove right now. Only checkouts of sessions archived for {days} days or more can go."))
                .key("storage-hint"),
        );
    } else if !page.previewing {
        head = head
            .child(muted(t, &format!("Clean now removes the checkouts of sessions archived for {days} days or more.")).key("storage-hint"));
    }
    let mut c = col().w_full().mt(17.5).padding(14.0).radius(RADIUS_BTN).border(1.0, t.border).border_layout().fill(t.main_bg).child(head);
    if page.previewing {
        c = c.child(preview_panel(page, t));
    }
    c.children(report_panel(page, t))
}

/// The card (`StorageSection`), inside the anchor `storage` that Settings scrolls to.
pub fn card_view(s: &State, t: &Theme) -> El {
    let page = &s.settings.storage;
    let mut c = col()
        .key("settings-anchor-storage")
        .w_full()
        .px(14.0)
        .py(14.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.card_bg)
        .align(Align::Start)
        .child(
            col()
                .w_full()
                .gap(3.5)
                .child(text("Storage").font(sans(TEXT_SM, 600.0, 1.43)).color(t.text))
                .child(muted(t, "What Fleet's worktree checkouts take on this machine.")),
        );
    if let Some(why) = &page.error {
        c = c.child(
            row()
                .key("storage-error")
                .role("alert")
                .w_full()
                .mt(14.0)
                .gap(7.0)
                .center_y()
                .child(icon(&CIRCLE_ALERT, 14.0).color(t.error))
                .child(text(why.clone()).font(sans(TEXT_SM, 400.0, 1.43)).color(t.error).wrap().shrink().min_w(0.0)),
        );
    }
    if page.loading {
        return c.child(
            row()
                .key("storage-loading")
                .mt(17.5)
                .gap(7.0)
                .center_y()
                .child(confirm::spinner(t.muted))
                .child(text("Looking at your checkouts…").font(sans(TEXT_SM, 400.0, 1.43)).color(t.muted)),
        );
    }
    if page.error.is_some() && page.checkouts.is_empty() {
        return c;
    }
    c.child(totals(page, t)).child(always_kept(t)).child(clean_box(page, t))
}
