//! One card of the Board (`KanbanCard.vue`): the source pill and the time, the title (or its rename field), GitHub's block for a
//! GitHub card, the Created / Source key / Position details, a note that isn't JSON, and Rename / Archive / Delete. Dragging
//! (`draggable`, `k-card--dragging`) comes with piece O.
use crate::State;
use crate::fleet::board::Card;
use crate::fleet::board::cards::{self, GitHubMeta};
use crate::icons::{EXTERNAL_LINK, GITHUB, TRIANGLE_ALERT};
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, sans};
use crate::ui::age;
use crate::ui::button::{Size, Variant, button, disabled};
use crate::ui::text_menu::text_input;
use crate::view::session_row::RowClock;
use rune::{Color, Cursor, El, Key, KeyEvent, Spring, Tx, col, el, icon, row, text};

/// `.k-card__source-pill`, `.k-card__sync-pill`, `.k-card__stale-pill`: a round pill, 10 px.
fn pill(key: String, fill: Color, ink: Color, weight: f32, lead: Option<El>, words: &str) -> El {
    let style = sans(10.0, weight, 1.5);
    let style = if weight >= 700.0 { style.tracking(0.4) } else { style };
    let words = if weight >= 700.0 { words.to_uppercase() } else { words.to_string() };
    row().center_y().gap(6.0).px(8.0).py(4.0).radius(999.0).fill(fill).children(lead).child(text(words).key(key).font(style).color(ink))
}

/// `dt` over `dd`: the label in 10 px capitals, the value under it.
fn detail(t: &Theme, key: String, label: &str, value: &str, tip: Option<String>) -> El {
    let value = text(value.to_string()).key(format!("{key}-value")).font(sans(11.0, 400.0, 1.5)).color(t.text).wrap_anywhere();
    let value = match tip {
        Some(tip) => value.help(tip),
        None => value,
    };
    col().w(0.0).grow(1.0).min_w(0.0).child(text(label.to_uppercase()).font(sans(10.0, 700.0, 1.5).tracking(0.4)).color(t.muted).wrap()).child(
        // `dd { margin-top: 4px }`
        value.mt(4.0),
    )
}

/// A grid of three equal columns: a short row is padded so its cells keep a third each.
fn grid3(mut cells: Vec<El>) -> El {
    while !cells.len().is_multiple_of(3) {
        cells.push(el().w(0.0).grow(1.0).min_w(0.0));
    }
    let mut g = col().w_full().gap(8.0);
    while !cells.is_empty() {
        let rest = cells.split_off(3);
        g = g.child(row().w_full().gap(8.0).children(std::mem::replace(&mut cells, rest)));
    }
    g
}

/// `.k-card__sync-summary`: the link to the issue, State / Assignee / GitHub updated, and the labels.
fn github_block(t: &Theme, id: &str, meta: Option<&GitHubMeta>, label: &str, clock: &RowClock) -> El {
    let (fill, border) = t.board_github_block();
    let mut block =
        col().key(format!("board-card-{id}-github")).w_full().gap(10.0).padding(10.0).radius(RADIUS_BTN).border(1.0, border).fill(fill);
    block = block.border_layout();
    let Some(meta) = meta else { return block };
    if let Some(url) = meta.html_url.clone() {
        let (fill, border) = t.board_github_link();
        let link = row()
            .key(format!("board-card-{id}-link"))
            .role("link")
            .label(format!("Open {label} on GitHub"))
            .self_start()
            .center_y()
            .gap(6.0)
            .px(10.0)
            .py(6.0)
            .radius(999.0)
            .border(1.0, border)
            .border_layout()
            .fill(fill)
            .cursor(Cursor::Pointer)
            .focus_ring(2.0, 2.0, t.accent)
            .child(icon(&GITHUB, 14.0).color(t.text))
            .child(text(label.to_string()).font(sans(10.0, 600.0, 1.5)).color(t.text))
            .child(icon(&EXTERNAL_LINK, 12.0).color(t.text))
            .on_press_tx(move |_: &mut State, tx: &mut Tx| {
                let _ = tx.open_url(&url);
            });
        block = block.child(link);
    }
    let mut details = Vec::new();
    if let Some(state) = &meta.state {
        details.push(detail(t, format!("board-card-{id}-state"), "State", state, None));
    }
    if let Some(who) = &meta.assignee {
        details.push(detail(t, format!("board-card-{id}-assignee"), "Assignee", who, None));
    }
    if let Some(at) = &meta.updated_at {
        let words = age::long(at, clock.now, clock.offset_secs);
        let tip = age::exact(at, clock.now, clock.offset_secs);
        details.push(detail(t, format!("board-card-{id}-github-updated"), "GitHub updated", &words, Some(tip)));
    }
    if !details.is_empty() {
        block = block.child(grid3(details));
    }
    if !meta.labels.is_empty() {
        let chips = meta.labels.iter().enumerate().map(|(i, name)| {
            row()
                .key(format!("board-card-{id}-label-{i}"))
                .px(8.0)
                .py(4.0)
                .radius(999.0)
                .fill(t.board_label_fill())
                .child(text(name.clone()).font(sans(10.0, 400.0, 1.5)).color(t.text).wrap_anywhere())
        });
        block =
            block.child(row().key(format!("board-card-{id}-labels")).label("GitHub labels").w_full().flex_wrap().gap(6.0).children(chips));
    }
    block
}

/// The rename field (`.k-card__rename-input`): 240 characters, Enter saves, Esc cancels.
fn rename_field(s: &State, t: &Theme, card: &Card) -> El {
    let id = card.id.clone();
    let edit = s.board.compose.renames.get(&card.id).cloned().unwrap_or_default();
    let busy = s.board.is_mutating();
    let (a, b, c) = (id.clone(), id.clone(), id.clone());
    let field = text_input(&edit, sans(13.0, 400.0, 1.4))
        .key(format!("board-rename-{id}-input"))
        .label("Card title")
        .single_line()
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .w_full()
        .px(10.0)
        .py(8.0)
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.board_faint_fill())
        .focus_ring(1.0, 0.0, t.accent)
        .disabled(busy)
        .on_edit(move |s: &mut State| s.board.compose.renames.entry(a.clone()).or_default())
        .on_input_change(move |s: &mut State, _: &mut Tx| s.board_rename_typed(&b))
        .on_input_key(move |s: &mut State, ev: &KeyEvent, _: &mut Tx| match ev.key {
            Key::Escape => {
                s.board_cancel_rename(&c);
                true
            }
            _ => false,
        });
    let id = card.id.clone();
    field.on_submit(move |s: &mut State, _: &mut Tx| s.board_submit_rename(&id))
}

/// The card's three buttons, disabled while a change runs.
fn actions(s: &State, t: &Theme, card: &Card, renaming: bool) -> El {
    let busy = s.board.is_mutating();
    let off = |b: El| if busy { disabled(b) } else { b };
    let id = card.id.clone();
    let first = if renaming {
        let id = id.clone();
        button(t, Variant::Ghost, Size::Sm, None, "Save")
            .key(format!("board-card-{}-save", card.id))
            .on_press(move |s: &mut State| s.board_submit_rename(&id))
    } else {
        let id = id.clone();
        button(t, Variant::Ghost, Size::Sm, None, "Rename")
            .key(format!("board-card-{}-rename", card.id))
            .on_press(move |s: &mut State| s.board_begin_rename(&id))
    };
    let archive = {
        let id = id.clone();
        button(t, Variant::Ghost, Size::Sm, None, "Archive")
            .key(format!("board-card-{}-archive", card.id))
            .on_press(move |s: &mut State| s.board_archive_pressed(&id))
    };
    let delete = button(t, Variant::Destructive, Size::Sm, None, "Delete")
        .key(format!("board-card-{}-delete", card.id))
        .on_press_tx(move |s: &mut State, tx: &mut Tx| s.board_delete_ask(&id, tx));
    row().w_full().flex_wrap().center_y().justify_between().gap(8.0).child(off(first)).child(off(archive)).child(off(delete))
}

pub fn view(s: &State, t: &Theme, card: &Card, clock: &RowClock) -> El {
    let id = &card.id;
    let manual = card.source_type.is_none();
    let github = cards::is_github(card);
    let meta = cards::github_meta(card);
    let renaming = s.board.compose.renames.contains_key(id);

    let (pill_fill, pill_ink) = t.board_source_pill(manual);
    let source = pill(format!("board-card-{id}-source"), pill_fill, pill_ink, 700.0, None, &cards::source_label(card));
    let mut badges = row().flex_wrap().center_y().gap(6.0).child(source);
    if github {
        let label = cards::github_issue_label(meta.as_ref());
        let lead = icon(&GITHUB, 12.0).color(t.text);
        badges = badges.child(pill(format!("board-card-{id}-issue"), t.board_sync_pill(), t.text, 600.0, Some(lead), &label));
    }
    if meta.as_ref().is_some_and(|m| m.stale) {
        let (fill, ink) = t.board_stale_pill();
        let lead = icon(&TRIANGLE_ALERT, 12.0).color(ink);
        let stale = pill(format!("board-card-{id}-stale"), fill, ink, 600.0, Some(lead), "Stale");
        badges = badges.child(stale.key(format!("board-card-{id}-stale-pill")).role("status").label("Sync is stale"));
    }
    let updated = format!("Updated {}", age::long(&card.updated_at, clock.now, clock.offset_secs));
    let meta_row = row().w_full().flex_wrap().center_y().justify_between().gap(8.0).child(badges).child(
        text(updated).key(format!("board-card-{id}-updated")).font(sans(11.0, 400.0, 1.5)).color(t.muted).help(age::exact(
            &card.updated_at,
            clock.now,
            clock.offset_secs,
        )),
    );

    let title = if renaming {
        rename_field(s, t, card)
    } else {
        text(card.title.clone()).key(format!("board-card-{id}-title")).font(sans(13.0, 700.0, 1.4)).color(t.text).wrap_anywhere()
    };

    let mut body = col().grow(1.0).min_w(0.0).gap(10.0).padding(12.0).child(meta_row).child(title);
    if github {
        let label = cards::github_issue_label(meta.as_ref());
        body = body.child(github_block(t, id, meta.as_ref(), &label, clock));
    }
    let created = age::long(&card.created_at, clock.now, clock.offset_secs);
    body = body.child(grid3(vec![
        detail(
            t,
            format!("board-card-{id}-created"),
            "Created",
            &created,
            Some(age::exact(&card.created_at, clock.now, clock.offset_secs)),
        ),
        detail(t, format!("board-card-{id}-key"), "Source key", &cards::source_detail(card), None),
        detail(t, format!("board-card-{id}-position"), "Position", &card.position.to_string(), None),
    ]));
    if let Some(note) = cards::metadata_text(card) {
        body = body.child(text(note).key(format!("board-card-{id}-metadata")).font(sans(11.0, 400.0, 1.5)).color(t.muted).wrap_anywhere());
    }
    body = body.child(actions(s, t, card, renaming));

    // `border-left: 3px` in the card's own colour: a stripe, with the border drawn on the other three sides.
    let stripe = el().w(3.0).self_stretch().fill(t.board_card_edge(manual));
    let hovered = s.board.compose.hover.as_deref() == Some(id.as_str());
    let key = id.clone();
    let c = row()
        .key(format!("board-card-{id}"))
        .role("article")
        .label(card.title.clone())
        .w_full()
        .radius(RADIUS_CARD)
        .clip_rounded(RADIUS_CARD)
        .border_sides([1.0, 1.0, 1.0, 0.0], t.border)
        .border_layout()
        .fill(t.board_card_fill(manual))
        .cursor(Cursor::Grab)
        .on_hover_change(move |s: &mut State, over: bool, _: &mut Tx| s.board_hover_card(&key, over))
        .child(stripe)
        .child(body);
    // `:hover { transform: translateY(-2px); box-shadow: 0 12px 24px rgba(0,0,0,.18) }`, 0.2 s.
    let lifted = if hovered { c.offset(0.0, -2.0).shadow(12.0, 24.0, Color::BLACK.alpha(0.18)) } else { c };
    lifted.offset_motion(Spring::new(0.2, 0.0))
}
