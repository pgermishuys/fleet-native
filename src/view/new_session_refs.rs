//! The pieces of the New session page the message box's `@`, images and GitHub place add (`NewSessionComposer.vue`; rows A7.1-A7.6,
//! A8.11): the chip of the attached pull request or issue above the field, and the line under the chips that says which picks the
//! new folder turned back into text.
use crate::State;
use crate::fleet::new_session::places::remove_label;
use crate::icons::{CIRCLE_ALERT, CIRCLE_DOT, GIT_PULL_REQUEST, X};
use crate::theme::{RADIUS_BTN, Theme, mono, sans};
use rune::{Cursor, El, col, el, icon, rich, row, text};

/// `.new-session__attachments`: the attached item as a chip, above the field inside the frame.
pub fn preset_row(s: &State, t: &Theme) -> Option<El> {
    let preset = s.new_draft.draft()?.github_preset.clone()?;
    let starting = s.new_session_starting();
    let glyph = if preset.source_type == "github-pull-request" { &GIT_PULL_REQUEST } else { &CIRCLE_DOT };
    let label = remove_label(&preset);
    let remove = el()
        .key("new-session-github-remove")
        .role("button")
        .label(label.clone())
        .help(label)
        .size(20.0, 20.0)
        .center()
        .radius(4.0)
        .child(icon(&X, 12.0).color(t.muted).group_color(t.text));
    let remove = if starting {
        remove.opacity(0.5).disabled(true)
    } else {
        remove
            .hover(t.wash(8.0))
            .hover_group()
            .cursor(Cursor::Pointer)
            .focus_ring(2.0, 1.0, t.accent)
            .on_press_tx(|s: &mut State, tx| s.remove_github_preset(tx))
    };
    let chip = row()
        .key("new-session-github-attachment")
        .help(preset.html_url.clone())
        .min_w(0.0)
        .center_y()
        .gap(6.0)
        .pl(8.0)
        .pr(4.0)
        .py(3.0)
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.wash(4.0))
        .child(icon(glyph, 13.0).color(t.muted))
        .child(text(format!("#{}", preset.number)).font(mono(11.5, 400.0, 1.4)).color(t.muted))
        .child(text(preset.title.clone()).font(sans(12.5, 400.0, 1.4)).color(t.text).truncate().shrink().min_w(0.0))
        .child(remove);
    Some(row().key("new-session-attachments").w_full().flex_wrap().gap(6.0).pt(10.0).px(12.0).child(chip))
}

/// `p.new-session__folder-notice`: the paths the new folder doesn't have and what happens to them.
pub fn folder_notice(s: &State, t: &Theme) -> Option<El> {
    let (paths, folder) = s.new_folder_notice()?;
    let one = paths.len() == 1;
    let tail = format!(" {} in {folder}, so {} as text.", if one { "isn't" } else { "aren't" }, if one { "it stays" } else { "they stay" });
    // The paths in code, joined with ", " and " and ".
    let mut line = String::new();
    let mut spans = Vec::new();
    for (i, path) in paths.iter().enumerate() {
        if i > 0 {
            line.push_str(if i + 1 == paths.len() { " and " } else { ", " });
        }
        let start = line.len();
        line.push_str(path);
        spans.push(rune::Span::new(start, line.len()).mono().size(11.5));
    }
    line.push_str(&tail);
    Some(
        row()
            .key("new-session-folder-notice")
            .role("status")
            .w_full()
            .mt(8.0)
            .px(10.0)
            .py(7.0)
            .gap(8.0)
            .radius(8.0)
            .fill(t.status_waiting().alpha(0.12))
            .child(col().mt(3.0).child(icon(&CIRCLE_ALERT, 14.0).color(t.status_waiting())))
            .child(rich(line, spans).font(sans(12.5, 400.0, 1.5)).color(t.text).wrap().shrink().min_w(0.0)),
    )
}
