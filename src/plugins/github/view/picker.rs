//! `GitHubItemPicker.vue`: the `#` popup above the New session box: the folder's open pull requests and issues, a head with the
//! repository, the list in groups, and the keys under it. The host's state is `picker::Picker`; core places this above the frame.
use super::row::icon_state;
use crate::icons::GITHUB;
use crate::plugins::github::picker::{Picker, repository};
use crate::plugins::github::{Kind, state_ref};
use crate::theme::{RADIUS_CARD, Theme, mono, sans};
use crate::view::gh::{ItemKind, item_icon};
use crate::{State, Tx};
use rune::{Color, Cursor, El, Shadow, col, el, icon, row, text};

/// `max-height: 340px`: the popup stops growing here and the list scrolls.
const MAX_H: f32 = 340.0;
const SCROLL: &str = "github-picker-scroll";

/// The popup's box, as wide as the message frame.
pub fn popup(s: &State, t: &Theme) -> El {
    let p = &state_ref(&s.places).picker;
    let repo = repository(s).map(|(o, r)| format!("{o}/{r}"));
    let query = s.new_hash_trigger().map(|t| t.query).unwrap_or_default();
    let label = match &repo {
        Some(r) => format!("Pull requests and issues in {r}"),
        None => "Pull requests and issues".to_string(),
    };
    let mut list = col().key("github-picker-list").w_full().items_stretch();
    if repo.is_none() {
        list = list.child(note(t, "Pick a folder whose repository is on GitHub to link one of its issues or pull requests.", false));
    } else if let Some(error) = &p.error {
        list = list.child(note(t, error, true));
    } else if !p.loading && p.groups.is_empty() {
        list = list.child(note(t, "No open pull requests or issues match.", false));
    }
    let mut index = 0;
    for group in &p.groups {
        list = list.child(
            row()
                .w_full()
                .pt(8.0)
                .px(12.0)
                .pb(3.0)
                .child(text(group.label.to_uppercase()).font(sans(10.5, 600.0, 1.5).tracking(0.05 * 10.5)).color(t.muted)),
        );
        for item in &group.items {
            list = list.child(item_row(s, t, p, item, index));
            index += 1;
        }
    }
    let scroller = list.scroll(rune::ScrollSpec { stick_bottom: false }).key(SCROLL).w_full().shrink().min_h(0.0);
    col()
        .key("github-item-picker")
        .role("listbox")
        .label(label)
        .w(s.pickers.composer_w)
        .max_h(MAX_H)
        .items_stretch()
        .radius(RADIUS_CARD)
        .fill(t.card_bg)
        .border(1.0, t.border)
        .border_layout()
        .clip_rounded(RADIUS_CARD)
        .box_shadow(Shadow { x: 0.0, y: 10.0, blur: 30.0, spread: 0.0, color: Color::BLACK.alpha(0.3), inset: false })
        .child(head(t, p, repo.as_deref(), &query))
        .child(scroller)
        .child(foot(t))
}

fn head(t: &Theme, p: &Picker, repo: Option<&str>, query: &str) -> El {
    let mut r = row()
        .key("github-item-picker-head")
        .w_full()
        .center_y()
        .gap(8.0)
        .px(12.0)
        .py(8.0)
        .border_sides([0.0, 0.0, 1.0, 0.0], t.border)
        .child(icon(&GITHUB, 13.0).color(t.muted))
        .child(text(repo.unwrap_or("GitHub").to_string()).font(sans(12.0, 400.0, 1.5)).color(t.muted));
    if p.loading {
        r = r.child(xs_spinner(t));
    }
    r.child(el().grow(1.0)).child(text(format!("#{query}")).font(mono(12.0, 400.0, 1.5)).color(t.text))
}

/// `Spinner size="xs"`: 12px, turning.
fn xs_spinner(t: &Theme) -> El {
    static FADE: [(f32, f32); 3] = [(0.0, 1.0), (0.5, 0.35), (1.0, 1.0)];
    icon(&crate::icons::LOADER_CIRCLE, 12.0).color(t.muted).cycle(rune::Cycle {
        period: 1.2,
        phase: 0.0,
        stops: &FADE,
        still: 1.0,
        fps: 30.0,
    })
}

fn note(t: &Theme, words: &str, error: bool) -> El {
    let ink = if error { t.error_text } else { t.muted };
    row()
        .key("github-item-picker-note")
        .w_full()
        .px(12.0)
        .py(12.0)
        .child(text(words.to_string()).font(sans(12.5, 400.0, 1.5)).color(ink).wrap())
}

/// `.gh-picker__row`: the icon, the number, the title, and "has a session" at the right.
fn item_row(s: &State, t: &Theme, p: &Picker, item: &crate::plugins::github::items::ItemSummary, index: usize) -> El {
    let kind = if item.kind == Kind::Pull { ItemKind::Pull } else { ItemKind::Issue };
    let selected = index == p.index;
    let mut r = row()
        .key(format!("github-item-picker-row-{index}"))
        .role("option")
        .label(format!("{} #{}: {}", if item.kind == Kind::Pull { "Pull request" } else { "Issue" }, item.number, item.title))
        .w_full()
        .center_y()
        .gap(8.0)
        .px(12.0)
        .py(6.0)
        .focusable(false)
        .cursor(Cursor::Pointer)
        .child(el().size(16.0, 16.0).center().child(item_icon(t, kind, icon_state(item), 14.0)))
        .child(row().w(44.0).justify_end().child(text(format!("#{}", item.number)).font(mono(11.5, 400.0, 1.5)).color(t.muted)))
        .child(text(item.title.clone()).font(sans(12.5, 400.0, 1.5)).color(t.text).truncate().grow(1.0).min_w(0.0));
    if s.picker_has_session(item) {
        r = r.child(text("has a session").font(sans(11.0, 400.0, 1.5)).color(t.muted));
    }
    if selected {
        r = r.fill(t.accent.alpha(0.14));
    }
    r.on_hover(move |s: &mut State| state_mut(s).index = index).on_press_tx({
        let item = item.clone();
        move |s: &mut State, tx: &mut Tx| s.picker_pick(&item, false, tx)
    })
}

fn state_mut(s: &mut State) -> &mut Picker {
    &mut crate::plugins::github::state(&mut s.places).picker
}

/// `.gh-picker__foot`: the keys.
fn foot(t: &Theme) -> El {
    let say = |w: &str| text(w.to_string()).font(sans(11.0, 400.0, 1.5)).color(t.muted);
    let kbd = |k: &str| row().px(4.0).radius(4.0).fill(t.wash(8.0)).child(text(k.to_string()).font(mono(10.5, 400.0, 1.5)).color(t.muted));
    row()
        .key("github-item-picker-foot")
        .w_full()
        .flex_wrap()
        .center_y()
        .gaps(4.0, 12.0)
        .px(12.0)
        .py(7.0)
        .border_sides([1.0, 0.0, 0.0, 0.0], t.border)
        .child(row().gap(4.0).child(kbd("↑↓")).child(say("move")))
        .child(row().gap(4.0).child(kbd("Enter")).child(say("start from it")))
        .child(row().child(kbd("Shift")).child(say("+")).child(kbd("Enter")).child(say(" open its session")))
        .child(el().grow(1.0))
        .child(kbd("Esc"))
}
