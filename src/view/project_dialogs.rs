//! `ConfirmDeleteProjectDialog.vue`: an alert with two radio cards (what happens to the project's sessions), Cancel and
//! "Delete project". Drawn on `ui::dialog`.
use crate::State;
use crate::fleet::projects::{DeleteDialog, DeleteMode};
use crate::icons::TRIANGLE_ALERT;
use crate::theme::{Theme, sans};
use crate::ui::button::{Size, Variant, button, disabled};
use crate::ui::dialog::{Dialog, dialog};
use crate::ui::rem;
use rune::{Color, Cursor, El, Span, col, el, rich, row, text};

/// "Choose what should happen to the 3 sessions in this project.", and where the count is (it is set in medium weight).
pub fn description(count: usize) -> (String, std::ops::Range<usize>) {
    let head = "Choose what should happen to the ";
    let n = count.to_string();
    let s = if count == 1 { "" } else { "s" };
    (format!("{head}{n} session{s} in this project."), head.len()..head.len() + n.len())
}

/// A native radio, 13px, in the accent colour (as the export dialog's).
fn radio(t: &Theme, on: bool, ink: Color) -> El {
    let b = el().size(13.0, 13.0).mt(1.75).border(1.0, if on { ink } else { t.muted }).border_layout().radius(6.5);
    if on { b.center().child(el().size(5.0, 5.0).radius(2.5).fill(ink)) } else { b }
}

#[allow(clippy::too_many_arguments)]
fn card(t: &Theme, key: &str, on: bool, deleting: bool, danger: bool, title: &str, line: &str, mode: DeleteMode) -> El {
    let tint = if danger { t.destructive } else { t.accent };
    // `bg-secondary/60`, `border-primary bg-primary/5`; the destructive card is tinted at rest.
    let (fill, hover, edge) = if danger {
        (
            t.destructive.alpha(if on { 0.10 } else { 0.05 }),
            t.destructive.alpha(0.10),
            if on { t.destructive } else { t.destructive.alpha(0.4) },
        )
    } else {
        (if on { t.accent.alpha(0.05) } else { t.wash(8.0 * 0.6) }, t.secondary(), if on { t.accent } else { t.border })
    };
    let mut c = row()
        .key(key.to_string())
        .role("radio")
        .label(title.to_string())
        .checked(on)
        .w_full()
        .gap(rem(0.75))
        .items_start()
        .padding(rem(1.0))
        .fill(fill)
        .border(1.0, edge)
        .border_layout()
        .hover(hover)
        .focus_ring(2.0, 2.0, t.accent)
        .child(radio(t, on, tint))
        .child(
            col()
                .gap(rem(0.25))
                .grow(1.0)
                .min_w(0.0)
                .child(text(title.to_string()).font(sans(rem(0.875), 500.0, 1.3)).color(t.text))
                .child(text(line.to_string()).font(sans(rem(0.875), 400.0, 1.3)).color(t.muted).wrap().w_full()),
        );
    c = if deleting {
        c.opacity(0.5).disabled(true)
    } else {
        c.cursor(Cursor::Pointer).on_press(move |s: &mut State| s.pick_delete_mode(mode))
    };
    c
}

/// The dialog and its scrim, to mount over the window; `None` while it is closed.
pub fn delete(s: &State, t: &Theme) -> Option<Vec<El>> {
    let d: &DeleteDialog = s.proj.delete.as_ref()?;
    let (words, count_at) = description(d.count);
    let desc = rich(words, vec![Span::new(count_at.start, count_at.end).color(t.text).weight(500.0)])
        .font(sans(rem(0.875), 400.0, 1.4))
        .color(t.muted)
        .wrap();
    let cards = col()
        .w_full()
        .gap(rem(0.75))
        .child(card(
            t,
            "delete-project-keep",
            d.mode == DeleteMode::MoveToScratch,
            d.deleting,
            false,
            "Delete project only",
            "Move all sessions to Ungrouped and keep them available.",
            DeleteMode::MoveToScratch,
        ))
        .child(card(
            t,
            "delete-project-all",
            d.mode == DeleteMode::DeleteSessions,
            d.deleting,
            true,
            "Delete project and all sessions",
            "This permanently removes the project and every session inside it.",
            DeleteMode::DeleteSessions,
        ));
    let cancel = button(t, Variant::Outline, Size::Default, None, "Cancel").key("delete-project-cancel");
    let confirm = button(t, Variant::Destructive, Size::Default, None, if d.deleting { "Deleting…" } else { "Delete project" })
        .key("delete-project-confirm");
    let (cancel, confirm) = if d.deleting {
        (disabled(cancel), disabled(confirm))
    } else {
        (cancel.on_press(|s: &mut State| s.cancel_delete()), confirm.on_press_tx(|s: &mut State, tx: &mut crate::Tx| s.confirm_delete(tx)))
    };
    Some(dialog(
        t,
        Dialog {
            key: "delete-project",
            title: format!("Delete {}?", d.name),
            title_icon: Some((&TRIANGLE_ALERT, t.error_text)),
            description_el: Some(desc),
            body: vec![cards],
            footer: vec![cancel, confirm],
            close_button: false,
            click_outside: false,
            dismiss: Some(|s: &mut State| s.cancel_delete()),
            ..Default::default()
        },
    ))
}
