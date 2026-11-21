//! An item's description and conversation (`GitHubWorkItemDetailPage.vue`'s `.gh-item__body`, rows 11.x of docs/parity/21-github.md):
//! the body in Markdown, then each comment, review and change of state down a rule with a dot at each, and the rail's column beside
//! them (empty until piece L fills it).
use crate::State;
use crate::fleet::pr_state::PrState;
use crate::plugins::github::Kind;
use crate::plugins::github::item_page::{Dot, reference_state, timeline_tone, timeline_words};
use crate::plugins::github::items::{ItemDetail, TimelineEntry, TimelineKind, visible_body};
use crate::theme::{RADIUS_CARD, Theme, sans};
use crate::ui::age;
use crate::view::gh::{ItemKind, item_icon, reviewer_avatar};
use crate::view::prose::{notes_style, prose, style};
use crate::view::session_row::RowClock;
use rune::{Cursor, El, Tx, col, el, row, text};

/// `max-width: 78ch` at the description's and a comment's sizes.
const DESCRIPTION_W: f32 = 630.0;
const COMMENT_W: f32 = 606.0;

/// The rail's width (`grid-template-columns: minmax(0, 1fr) 300px`).
pub const RAIL_W: f32 = 300.0;

fn dot(t: &Theme, tone: Dot) -> El {
    let ring = match tone {
        Dot::Plain => t.text.alpha(0.18),
        Dot::Approved => t.check_pass(),
        Dot::Changes => t.pr_tone(PrState::Blocked),
        Dot::Merged => t.pr_tone(PrState::Merged),
        Dot::Closed => t.pr_tone(PrState::Closed),
    };
    // `::before`: 11 square, a 2 px ring, the panel's fill under it, 24 left of the entry and 14 down.
    el().absolute(-24.0, 14.0).size(11.0, 11.0).radius(11.0).border(2.0, ring).border_layout().fill(t.panel_bg)
}

/// `.gh-timeline__ref`: a mention, with its item's icon.
fn reference(s: &State, t: &Theme, key: &str, entry: &TimelineEntry) -> Option<El> {
    let r = entry.reference.as_ref()?;
    let _ = s;
    let kind = if r.kind == Kind::Pull { ItemKind::Pull } else { ItemKind::Issue };
    let words = format!("#{} {}", r.number, entry.body.as_deref().unwrap_or_default());
    let entry = entry.clone();
    Some(
        row()
            .key(key.to_string())
            .role("link")
            .label(words.clone())
            .gap(4.0)
            .center_y()
            .min_w(0.0)
            .cursor(Cursor::Pointer)
            .focusable(true)
            .focus_ring(2.0, 1.0, t.accent)
            .child(item_icon(t, kind, reference_state(&entry), 12.0))
            .child(text(words).font(sans(12.5, 400.0, 1.5)).color(t.text))
            .on_press_tx(move |s: &mut State, tx: &mut Tx| s.github_item_open_reference(&entry, tx)),
    )
}

fn entry(s: &State, t: &Theme, clock: &RowClock, i: usize, e: &TimelineEntry) -> El {
    let small = || sans(12.5, 400.0, 1.5);
    let mut head = row().key(format!("github-item-entry-{i}-head")).flex_wrap().center_y().gap(6.0);
    if let Some(author) = &e.author {
        head = head.child(reviewer_avatar(t, &format!("github-item-entry-{i}-avatar"), author, None, 20.0));
    }
    head = head
        .child(text(e.author.clone().unwrap_or_else(|| "Someone".into())).font(sans(12.5, 500.0, 1.5)).color(t.text))
        .child(text(timeline_words(e)).font(small()).color(t.muted));
    if let (TimelineKind::Referenced, Some(link)) = (e.kind, reference(s, t, &format!("github-item-entry-{i}-ref"), e)) {
        head = head.child(link);
    }
    head = head.child(text(format!("\u{b7} {}", age::long(&e.created_at, clock.now, clock.offset_secs))).font(small()).color(t.muted));
    let mut out = col().key(format!("github-item-entry-{i}")).gap(6.0).py(9.0).child(dot(t, timeline_tone(e))).child(head);
    let body = visible_body(e.body.as_deref());
    if e.kind != TimelineKind::Referenced && !body.is_empty() {
        let card = style(t).body_font(sans(13.0, 400.0, 1.65));
        out = out.child(
            col()
                .key(format!("github-item-entry-{i}-body"))
                .max_w(COMMENT_W)
                .px(12.0)
                .py(9.0)
                .radius(RADIUS_CARD)
                .fill(t.card_bg)
                .border(1.0, t.border)
                .border_layout()
                .child(prose(&card, &format!("github-item-entry-{i}-md"), &body, false)),
        );
    }
    out
}

fn conversation(s: &State, t: &Theme, detail: &ItemDetail) -> El {
    let clock = RowClock::now(s);
    col()
        .key("github-item-conversation")
        .role("list")
        .label("Conversation")
        .pl(18.0)
        .border_sides([0.0, 0.0, 0.0, 1.0], t.border)
        .border_layout()
        .children(detail.timeline.iter().enumerate().map(|(i, e)| entry(s, t, &clock, i, e)))
}

/// `.gh-item__main`: the description (or "No description.") and the conversation.
fn main(s: &State, t: &Theme, detail: &ItemDetail) -> El {
    let description = visible_body(detail.body.as_deref());
    let mut main = col().key("github-item-main").grow(1.0).shrink().min_w(0.0).gap(18.0).pt(20.0).px(28.0).pb(32.0);
    main = if description.is_empty() {
        main.child(text("No description.").key("github-item-empty").font(sans(14.0, 400.0, 1.5).italic()).color(t.muted))
    } else {
        main.child(col().key("github-item-description").max_w(DESCRIPTION_W).child(prose(
            &notes_style(t),
            "github-item-description-md",
            &description,
            false,
        )))
    };
    if !detail.timeline.is_empty() {
        main = main.child(conversation(s, t, detail));
    }
    main
}

/// The rail's column: its fill, its left rule and its width; piece L draws what is in it.
fn rail(t: &Theme) -> El {
    col()
        .key("github-item-rail")
        .role("complementary")
        .label("Details")
        .w(RAIL_W)
        .self_stretch()
        .border_sides([0.0, 0.0, 0.0, 1.0], t.border)
        .border_layout()
        .fill(t.wash(2.0))
}

pub fn body(s: &State, t: &Theme, detail: &ItemDetail) -> El {
    row().key("github-item-body").w_full().grow(1.0).min_h(0.0).child(main(s, t, detail)).child(rail(t))
}
