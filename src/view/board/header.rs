//! The Board's header and what sits under it (`KanbanBoard.vue`): the `PageHeader` bar with its eyebrow, the subtitle, the mode
//! button and Sync now, the four tiles, the sync toast and the error banner.
use crate::State;
use crate::fleet::board::store::Mode;
use crate::theme::{RADIUS_CARD, Theme, sans};
use crate::ui::button::{Size, Variant, button, disabled};
use crate::ui::page_header;
use rune::{El, col, row, text};

/// `subtitle`: no singulars, as Fleet has none ("1 active cards across 1 lanes").
pub fn subtitle(s: &State) -> String {
    let b = &s.board;
    if b.is_loading && !b.is_loaded {
        return "Loading your persisted board…".into();
    }
    if !b.has_board() {
        return "Create your first lane to start a persistent board for manual and API-backed cards.".into();
    }
    let counts = format!("{} active cards across {} lanes.", b.active_cards().count(), b.lanes.len());
    match b.inbox_lane() {
        Some(lane) => format!("{counts} Inbox lane: {}.", lane.name),
        None => format!("{counts} Choose an inbox lane to capture manual work quickly."),
    }
}

/// The bar: "Persistent board", the board's name (else "Kanban Board"), the subtitle, then Edit Board / Save and Sync now.
pub fn bar(s: &State, t: &Theme) -> El {
    let b = &s.board;
    let title = b.board.as_ref().map_or("Kanban Board", |board| board.name.as_str());
    let manage = b.mode == Mode::Manage;
    // `disabled="!isLoaded"`: it stays enabled while a change runs (FB: "keeps the mode toggle available during mutations").
    let mode = button(t, Variant::Outline, Size::Sm, None, if manage { "Save" } else { "Edit Board" })
        .key("board-mode")
        .role("button")
        .checked(manage)
        .border(1.0, t.accent_border(24.0));
    let mode = if b.is_loaded { mode.on_press(|s: &mut State| s.board_toggle_pressed()) } else { disabled(mode) };
    let mut actions = vec![mode];
    if b.has_board() {
        let label = if b.page.syncing { "Syncing…" } else { "Sync now" };
        let sync = button(t, Variant::Default, Size::Sm, None, label).key("kanban-sync-button");
        let busy = (b.is_loading && !b.is_loaded) || b.is_mutating();
        actions.push(if busy { disabled(sync) } else { sync.on_press(|s: &mut State| s.board_sync_pressed()) });
    }
    // Order as Fleet's: mode, Sync now, then manage mode's Rename board and Add lane.
    actions.extend(super::manage::header_buttons(s, t));
    let form = b.compose.board_rename.as_ref().filter(|_| manage).map(|edit| super::manage::board_rename_form(s, t, edit));
    page_header::bar_eyebrow(
        t,
        "board-header",
        "Persistent board",
        title,
        &subtitle(s),
        page_header::Slots { form, tinted: manage },
        actions,
    )
}

/// One tile of the summary: the value over its label.
fn tile(t: &Theme, label: &str, value: String) -> El {
    col()
        .key(format!("board-tile-{}", label.to_lowercase()))
        .w(0.0)
        .grow(1.0)
        .min_w(0.0)
        .gap(4.0)
        .padding(12.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.card_bg)
        .child(
            text(value)
                .key(format!("board-tile-{}-value", label.to_lowercase()))
                .font(sans(20.0, 700.0, 1.5))
                .color(t.text)
                .wrap_anywhere(),
        )
        .child(
            text(label.to_uppercase())
                .key(format!("board-tile-{}-label", label.to_lowercase()))
                .font(sans(10.0, 600.0, 1.5).tracking(0.5))
                .color(t.muted),
        )
}

/// `kanban-summary`: Lanes, Cards, Manual and Inbox, four equal columns 12 apart.
pub fn tiles(s: &State, t: &Theme) -> El {
    let b = &s.board;
    let manual = b.active_cards().filter(|c| c.source_type.is_none()).count();
    row()
        .key("board-summary")
        .role("region")
        .label("Board summary")
        .w_full()
        .gap(12.0)
        .px(24.0)
        .pt(12.0)
        .child(tile(t, "Lanes", b.lanes.len().to_string()))
        .child(tile(t, "Cards", b.active_cards().count().to_string()))
        .child(tile(t, "Manual", manual.to_string()))
        .child(tile(t, "Inbox", b.inbox_lane().map_or("Unset".to_string(), |l| l.name.clone())))
}

/// A band of copy and buttons: the toast and the banner share it (margin 12 24 0, padding 12 14, gap 16).
fn band(key: &str, tint: (rune::Color, rune::Color, rune::Color), copy: El, buttons: Vec<El>) -> El {
    let (_, border, fill) = tint;
    row().key(key.to_string()).w_full().px(24.0).pt(12.0).child(
        row()
            .w_full()
            .center_y()
            .gap(16.0)
            .px(14.0)
            .py(12.0)
            .radius(RADIUS_CARD)
            .border(1.0, border)
            .border_layout()
            .fill(fill)
            .child(copy.grow(1.0).shrink().min_w(0.0))
            .child(row().gap(8.0).center_y().children(buttons)),
    )
}

/// The sync toast (`role=status`): "Board synced" over "{n} added, {n} updated, {n} stale", and Dismiss.
pub fn toast(s: &State, t: &Theme) -> Option<El> {
    let (feedback, _) = s.board.page.feedback.as_ref()?;
    let tint = t.board_toast();
    let line = |key: &str, words: &str, weight: f32| {
        text(words.to_string()).key(key.to_string()).font(sans(12.0, weight, 1.5)).color(tint.0).wrap()
    };
    let copy = col().gap(4.0).child(line("kanban-sync-feedback-title", &feedback.title, 700.0)).child(line(
        "kanban-sync-feedback-message",
        &feedback.message,
        400.0,
    ));
    let dismiss = button(t, Variant::Ghost, Size::Sm, None, "Dismiss")
        .key("board-toast-dismiss")
        .label("Dismiss sync feedback")
        .on_press(|s: &mut State| s.board_dismiss_toast());
    Some(band("kanban-sync-feedback", tint, copy, vec![dismiss]).role("status"))
}

/// The error banner (`role=alert`), for a failed load and a failed change alike: Try again reloads, Dismiss clears.
pub fn banner(s: &State, t: &Theme) -> Option<El> {
    let error = s.board.error.as_ref()?;
    let tint = t.board_banner();
    let copy = text(error.clone()).key("board-error-text").font(sans(12.0, 400.0, 1.5)).color(tint.0).wrap();
    let retry =
        button(t, Variant::Default, Size::Sm, None, "Try again").key("board-error-retry").on_press(|s: &mut State| s.board_reload());
    let dismiss =
        button(t, Variant::Outline, Size::Sm, None, "Dismiss").key("board-error-dismiss").on_press(|s: &mut State| s.board.clear_error());
    Some(band("board-error", tint, copy, vec![retry, dismiss]).role("alert"))
}

/// The first load's wait (`kanban-state`): centred across, 12 apart. Fleet's `justify-content: center` has nothing to centre in:
/// the board's container is as tall as its content, so the words sit right under the tiles.
pub fn loading(t: &Theme) -> El {
    col()
        .key("board-loading")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .items_center()
        .gap(12.0)
        .px(24.0)
        .py(32.0)
        .child(text("Loading board…").font(sans(20.0, 700.0, 1.5)).color(t.text))
        .child(
            text("Fetching lanes and cards from the persisted board store.")
                .font(sans(13.0, 400.0, 1.6))
                .color(t.muted)
                .wrap()
                .max_w(460.0)
                .align(rune::Align::Center),
        )
}
