//! Ports `NSD/BasePicker.vue`: the chip that says where a new worktree starts and the menu above it, with the branches to start
//! from, "Fetch origin first" and the new branch's name (rows B3.1-B3.22). What the rows mean is
//! `fleet::new_session::workspace`.
use crate::fleet::new_session::workspace::{BASE_CHIP, BASE_SEARCH_KEY, BRANCH_KEY, base_empty_note};
use crate::icons::*;
use crate::theme::{RADIUS_BTN, Theme, mono, sans};
use crate::ui::chip::{Chip, Option_, option, pop, pop_label, pop_note, pop_separator};
use crate::{State, Tx};
use rune::{Align, Color, Cursor, El, Key, KeyEvent, ScrollSpec, Side, col, el, icon, row, text, text_input};

const WIDTH: f32 = 344.0;
/// `min(240px, 36vh)`; the window is never shorter than 680, so 36vh is at least 245.
const LIST_MAX_H: f32 = 240.0;

fn row_key(i: usize) -> String {
    format!("new-session-base-row-{i}")
}

/// The chip (B3.1-B3.4): a branch, `from ` and the base in muted type before it, ` · no fetch` after an unfetched origin base.
pub fn chip(s: &State, t: &Theme) -> El {
    let base = s.effective_base();
    let no_fetch = base.as_deref().is_some_and(|b| b.starts_with("origin/")) && !s.fetch_origin();
    let disabled = s.new_draft.draft().is_some_and(|d| d.is_starting);
    let mut chip = Chip::new(BASE_CHIP, base.clone().unwrap_or_else(|| "the default branch".into()))
        .glyph(&GIT_BRANCH)
        .head("from ")
        .tail(no_fetch.then(|| " · no fetch".to_string()))
        .open(s.base_menu_open())
        .disabled(disabled);
    if let Some(b) = &base {
        chip = chip.help(Some(format!("New branch starts from {b}")));
    }
    chip.view(t, |s: &mut State, tx: &mut Tx| {
        s.open_base_menu();
        tx.focus(BASE_SEARCH_KEY, false);
        tx.scroll_into_view(row_key(s.pickers_ns.base_highlight));
    })
}

/// `.ns-base-search`: the magnifier and the box (B3.6). Down, Up and Enter work the list (B3.13).
fn search(s: &State, t: &Theme) -> El {
    let field = text_input(&s.pickers_ns.base_query, sans(13.0, 400.0, 1.3))
        .key(BASE_SEARCH_KEY)
        .label("Search branches")
        .placeholder("Search branches…", t.muted)
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .single_line()
        .grow(1.0)
        .min_w(0.0)
        .on_edit(|s: &mut State| &mut s.pickers_ns.base_query)
        .on_input_change(|s: &mut State, _: &mut Tx| s.base_query_changed())
        .on_input_key(|s: &mut State, ev: &KeyEvent, tx: &mut Tx| match ev.key {
            Key::Down | Key::Up => {
                if let Some(at) = s.base_step(if ev.key == Key::Down { 1 } else { -1 }) {
                    tx.scroll_into_view(row_key(at));
                }
                true
            }
            Key::Enter => {
                s.base_choose_highlighted();
                true
            }
            _ => false,
        });
    row()
        .key("new-session-base-search-box")
        .items_center()
        .gap(7.0)
        .mx(-1.0)
        .mt(-1.0)
        .mb(2.0)
        .px(8.0)
        .pt(5.0)
        .pb(7.0)
        .border_sides([0.0, 0.0, 1.0, 0.0], t.border)
        .child(icon(&SEARCH, 14.0).color(t.muted))
        .child(field)
}

fn list(s: &State, t: &Theme) -> El {
    let rows = s.base_rows();
    let selected = s.effective_base();
    let default = s.default_base();
    let hl = s.pickers_ns.base_highlight;
    let mut c = col().key("new-session-base-list").role("listbox").label("Branches").w_full();
    for (i, b) in rows.iter().enumerate() {
        let tag = if default.as_deref() == Some(b.name.as_str()) {
            Some(" · default")
        } else if b.is_current {
            Some(" · checked out")
        } else {
            None
        };
        let name = b.name.clone();
        let spec = Option_ {
            icon: Some(if b.is_remote { &CLOUD } else { &GIT_BRANCH }),
            detail: (!b.message.is_empty()).then(|| format!("{} {}", b.short_hash, b.message)),
            checked: selected.as_deref() == Some(b.name.as_str()),
            highlighted: i == hl,
            tag: tag.map(str::to_string),
            ..Option_::new(row_key(i), b.name.clone())
        };
        c = c.child(option(t, spec, None, move |s, _| s.choose_base(&name)).on_hover(move |s: &mut State| s.pickers_ns.base_highlight = i));
    }
    if rows.is_empty() {
        c = c.child(pop_note(t, &base_empty_note(s.base_is_loading(), &s.pickers_ns.base_query.text)));
    }
    c.scroll(ScrollSpec { stick_bottom: false }).max_h(LIST_MAX_H)
}

/// `Switch` as `.ns-base-fetch` draws it: a 28 by 16.1 track, `--text` at 22% unchecked and `--primary` checked, a white thumb
/// with a small shadow; half opaque while Fetch doesn't apply (B3.18).
fn switch(t: &Theme, on: bool, enabled: bool) -> El {
    let (w, h, thumb) = (28.0, 16.1, 14.0);
    let mut track = row()
        .key("new-session-base-fetch")
        .role("switch")
        .label("Fetch origin first")
        .checked(on)
        .w(w)
        .h(h)
        .center_y()
        .radius(h / 2.0)
        .fill(if on { t.accent } else { t.text.alpha(0.22) })
        .border(1.0, Color::TRANSPARENT)
        .focus_ring(3.0, 0.0, t.accent.alpha(0.5))
        .child(
            el().size(thumb, thumb)
                .radius(thumb / 2.0)
                .fill(Color::WHITE)
                .box_shadow(rune::Shadow { x: 0.0, y: 1.0, blur: 2.0, spread: 0.0, color: Color::BLACK.alpha(0.3), inset: false })
                .ml(if on { w - thumb - 2.0 } else { 0.0 }),
        );
    if enabled {
        track = track.cursor(Cursor::Pointer).on_press(|s: &mut State| s.toggle_fetch_origin());
    } else {
        track = track.disabled(true).opacity(0.5);
    }
    track
}

/// `.ns-base-fetch`: gap 10, `6px 8px`; the words are the switch's label, so pressing them toggles it too (B3.17).
fn fetch(s: &State, t: &Theme) -> El {
    let origin = s.base_is_origin();
    let detail = match (origin, s.effective_base()) {
        (true, Some(b)) => format!("Starts from the newest {b}"),
        _ => "Only for origin’s branches".to_string(),
    };
    let words = col()
        .key("new-session-base-fetch-label")
        .grow(1.0)
        .min_w(0.0)
        .cursor(Cursor::Pointer)
        .on_press(|s: &mut State| s.toggle_fetch_origin())
        .child(text("Fetch origin first").font(sans(13.0, 400.0, 1.3)).color(t.text))
        .child(text(detail).font(sans(11.5, 400.0, 1.3)).color(t.muted));
    row().w_full().px(8.0).py(6.0).gap(10.0).center_y().child(words).child(switch(t, s.fetch_origin(), origin))
}

/// `.ns-field`: the New branch name (B3.20). Empty leaves the naming to the server.
fn branch_field(s: &State, t: &Theme) -> El {
    let placeholder = s.generated_branch().unwrap_or_else(|| "Named from your message".into());
    let input = text_input(&s.pickers_ns.branch, mono(12.0, 400.0, 1.3))
        .key(BRANCH_KEY)
        .label("New branch")
        .placeholder(placeholder, t.muted)
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .single_line()
        .w_full()
        .on_edit(|s: &mut State| &mut s.pickers_ns.branch)
        .on_input_change(|s: &mut State, _: &mut Tx| s.branch_name_edited())
        .on_input_key(|s: &mut State, ev: &KeyEvent, _: &mut Tx| {
            let enter = ev.key == Key::Enter;
            if enter {
                s.close_picker_menu();
            }
            enter
        });
    col().w_full().gap(4.0).px(8.0).pt(6.0).pb(8.0).child(text("New branch").font(sans(11.5, 400.0, 1.3)).color(t.muted)).child(
        row()
            .w_full()
            .px(8.0)
            .py(6.0)
            .items_center()
            .radius(RADIUS_BTN - 2.0)
            .border(1.0, t.border)
            .fill(t.wash(3.0))
            .focus_within_ring(1.0, -1.0, t.accent)
            .child(input),
    )
}

/// The menu: 6px above the chip, its left edge on the chip's (B3.5).
pub fn overlay(s: &State, t: &Theme) -> Vec<El> {
    let body =
        vec![search(s, t), pop_label(t, "Start from"), list(s, t), pop_separator(t), fetch(s, t), pop_separator(t), branch_field(s, t)];
    let panel =
        pop(t, "new-session-base-pop", WIDTH, body).focus_return(BASE_CHIP.to_string()).modal(true, |s: &mut State| s.close_picker_menu());
    vec![
        crate::ui::overlay::scrim("new-session-base-scrim", |s: &mut State| s.close_picker_menu()),
        crate::ui::overlay::anchored(panel, BASE_CHIP, Side::Top, Align::Start, 6.0),
    ]
}
