//! The side conversation's tab (what the panel folds into), its peek and the Undo toast. Ports the tab, `.side-peek` and
//! `.side-toast` of `components/session/SideConversationPanel.vue`; the panel itself is `view::side`.
use super::conversation::COLUMN;
use super::side::{DISCARD_LABEL, DISCARD_TIP, plain_title, shortcut};
use crate::State;
use crate::fleet::side::SideState;
use crate::fleet::side_tab::{OPEN_KEY, Word, dot_opacity};
use crate::icons::*;
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, mono, sans};
use rune::{Color, Cursor, El, Shadow, col, el, icon, row, text};

/// The tab's top corners are round (10) and its bottom is open: it is drawn with all four and the bottom ones cut off.
const TAB_RADIUS: f32 = 10.0;
/// 1 border + 3 padding + 22 (the × and the open button) + 5 padding: the tab as it shows.
const TAB_H: f32 = 31.0;
/// The tab sits 5 px over the composer's top border (`margin-bottom: -5px`).
const OVERLAP: f32 = 5.0;
const PEEK_W: f32 = 380.0;

/// The unread dot's ring (`@keyframes side-unread`): 60 % of the accent at the start, growing to 7 px, gone at 80 %.
fn pulse_ring(t: &Theme, phase: f32) -> Option<Shadow> {
    let grow = (phase / 0.8).min(1.0);
    let spread = 7.0 * (1.0 - (1.0 - grow) * (1.0 - grow));
    (grow < 1.0).then(|| Shadow { x: 0.0, y: 0.0, blur: 0.0, spread, color: t.accent.alpha(0.6 * (1.0 - grow)), inset: false })
}

/// The state words after the title: "· Thinking" with three dots, "· New answer" with its dot, "· Answered", or nothing.
fn state(s: &State, t: &Theme, st: &SideState) -> El {
    let word = st.word();
    let mut e = row().key("side-tab-state").items_center().gap(5.0);
    if word == Word::Nothing {
        return e;
    }
    e = e.child(text("\u{b7}").font(sans(12.0, 400.0, 1.4)).color(t.muted).mr(1.0));
    match word {
        Word::Thinking => {
            let dots = (0..3).map(|i| el().size(4.0, 4.0).radius(2.0).fill(t.muted.alpha(dot_opacity(s.clock, i))));
            e = e.child(row().key("side-tab-dots").gap(2.0).children(dots));
        }
        Word::NewAnswer { pulse } => {
            let mut dot = el().key("side-tab-unread").size(7.0, 7.0).radius(3.5).fill(t.accent);
            if let Some(ring) = pulse.then(|| s.side_pulse_phase().and_then(|p| pulse_ring(t, p))).flatten() {
                dot = dot.box_shadow(ring);
            }
            e = e.child(dot);
        }
        Word::Answered | Word::Nothing => {}
    }
    e.child(text(word.text()).font(sans(12.0, 400.0, 1.4)).color(t.muted))
}

/// The open button: the icon, "btw", the question and the state; a click opens the panel.
fn open_button(s: &State, t: &Theme, st: &SideState, title: &str) -> El {
    row()
        .key(OPEN_KEY)
        .items_center()
        .gap(7.0)
        .min_w(0.0)
        .shrink()
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 2.0, t.accent)
        .label(format!("Open the side conversation: {title}"))
        .child(icon(&MESSAGE_CIRCLE_QUESTION_MARK, 14.0).color(t.accent))
        .child(text("btw").font(sans(12.0, 600.0, 1.4)).color(t.accent_text))
        .child(text(title.to_string()).key("side-tab-question").font(sans(12.0, 500.0, 1.4)).color(t.text).truncate().shrink().min_w(0.0))
        .child(state(s, t, st))
        .on_press_tx(|s: &mut State, tx| s.side_set_minimized(false, tx))
}

/// The tab's ×: 22 square, discards like the header's.
fn close(t: &Theme) -> El {
    el().key("side-tab-close")
        .size(22.0, 22.0)
        .center()
        .radius(RADIUS_BTN)
        .label(DISCARD_LABEL)
        .help(DISCARD_TIP)
        .hover(t.wash(6.0))
        .hover_group()
        .cursor(Cursor::Pointer)
        .child(icon(&X, 14.0).color(t.muted).group_color(t.text))
        .on_press(|s: &mut State| s.side_discard())
}

/// The peek above the tab: "btw · latest answer", the answer in at most six lines, and how to open it.
fn peek_card(t: &Theme, st: &SideState, wrap_w: f32, amount: f32) -> El {
    let width = PEEK_W.min(wrap_w - 28.0).max(120.0);
    let kbd =
        row().px(4.0).radius(4.0).border(1.0, t.border).border_layout().child(text(shortcut()).font(mono(11.0, 400.0, 1.4)).color(t.muted));
    let foot =
        row().items_center().gap(4.0).child(text("Click the tab to open \u{b7}").font(sans(11.5, 400.0, 1.4)).color(t.muted)).child(kbd);
    let card = col()
        .key("side-peek")
        .role("tooltip")
        .w(width)
        .gap(6.0)
        .px(12.0)
        .py(10.0)
        .fill(t.card_bg)
        .border(1.0, t.border)
        .border_layout()
        .radius(RADIUS_CARD)
        .box_shadow(Shadow { x: 0.0, y: 16.0, blur: 36.0, spread: -18.0, color: Color::BLACK.alpha(0.5), inset: false })
        .opacity(amount)
        .offset(0.0, 4.0 * (1.0 - amount))
        .inert()
        .child(text("BTW \u{b7} LATEST ANSWER").font(sans(11.0, 600.0, 1.4).tracking(0.06 * 11.0)).color(t.muted))
        .child(text(st.peek_text()).key("side-peek-text").font(sans(13.0, 400.0, 1.5)).color(t.text).max_lines(6).w_full())
        .child(foot);
    card.pin(Some(14.0), None, None, Some(8.0))
}

/// What replaces the panel while it is folded: the tab on the composer's top edge, with the peek over it. `section_w` is the
/// section's inner width (the column, or what the window leaves).
pub fn tab(s: &State, t: &Theme, st: &SideState, section_w: f32) -> El {
    let title = st.side.as_ref().map(|side| plain_title(&side.title).to_string()).unwrap_or_default();
    let inner = row().items_center().gap(4.0).pt(3.0).pb(0.0).pl(10.0).pr(4.0).child(open_button(s, t, st, &title)).child(close(t));
    let tab = col()
        .key("side-tab")
        .h(TAB_H + TAB_RADIUS)
        .max_w(520.0)
        .ml(14.0)
        .shrink()
        .min_w(0.0)
        .fill(t.card_bg)
        .border(1.0, t.accent_border(45.0))
        .border_layout()
        .radius(TAB_RADIUS)
        .child(inner);
    // The bottom corners and edge belong to the composer's border: cut them off.
    let cut = el().key("side-tab-body").h(TAB_H).max_w(520.0 + 14.0).clip_rounded(0.0).child(tab);
    // The entrance: 260 ms from opacity 0, 8 px down and 60 % size, from its bottom left.
    let cut = match s.side_tab_enter() {
        Some(p) => cut.origin(0.0, 1.0).opacity(p).offset(0.0, 8.0 * (1.0 - p)).scale(0.6 + 0.4 * p),
        None => cut,
    };
    let amount = s.side_peek_amount();
    let wrap = col()
        .key("side-tab-wrap")
        .w_full()
        .max_w(COLUMN)
        .mb(-OVERLAP)
        .on_hover_change(|s: &mut State, inside, _| s.side_ui.tab.hover = inside);
    // A zero-high anchor on the tab's top edge, always there: the peek is pinned 8 px above it. The tab keeps its place in the tree
    // when the peek comes and goes, so a press that started on it as the pointer arrived still lands.
    let anchor =
        el().key("side-peek-anchor").w_full().h(0.0).children((amount > 0.0).then(|| peek_card(t, st, section_w.min(COLUMN), amount)));
    wrap.child(anchor).child(cut)
}

/// The Undo toast on the composer's column, just above the composer: "Side question discarded", the seconds left, Undo, and the bar
/// draining to nothing over Undo's window.
pub fn dock(s: &State, t: &Theme) -> Option<El> {
    let st = s.side()?;
    st.discarded.as_ref()?;
    let left = st.undo_seconds_left(s.clock)?;
    let width = 360.0_f32.min(s.sheet_w.min(COLUMN + 48.0) - 48.0 - 28.0).max(160.0);
    let on = t.main_bg;
    let enter = s.side_toast_enter();
    let undo = row()
        .key("side-undo")
        .items_center()
        .gap(6.0)
        .px(10.0)
        .py(4.0)
        .radius(RADIUS_BTN)
        .fill(on.alpha(0.18))
        .cursor(Cursor::Pointer)
        .child(icon(&UNDO_2, 14.0).color(on))
        .child(text("Undo").font(sans(13.0, 600.0, 1.4)).color(on))
        .on_press(|s: &mut State| s.side_undo());
    let words = row()
        .w_full()
        .items_center()
        .gap(12.0)
        .pl(14.0)
        .pr(8.0)
        .py(9.0)
        .child(text("Side question discarded").font(sans(13.0, 400.0, 1.4)).color(on))
        .child(el().grow(1.0))
        .child(text(format!("{left} s")).key("side-undo-left").font(sans(13.0, 400.0, 1.4).features("'tnum' 1")).color(on.alpha(0.7)))
        .child(undo);
    let bar = row().w_full().h(2.0).child(el().key("side-undo-drain").w(width * s.side_drain()).h(2.0).fill(on.alpha(0.45)));
    let toast = col()
        .key("side-undo-toast")
        .role("status")
        .w(width)
        .ml(14.0)
        .radius(RADIUS_CARD)
        .clip_rounded(RADIUS_CARD)
        .fill(t.text)
        .box_shadow(Shadow { x: 0.0, y: 12.0, blur: 32.0, spread: -12.0, color: Color::BLACK.alpha(0.45), inset: false })
        .opacity(enter)
        .offset(0.0, 6.0 * (1.0 - enter))
        .child(words)
        .child(bar);
    Some(col().key("side-undo-dock").w_full().items_center().child(row().w_full().max_w(COLUMN + 48.0).px(24.0).py(6.0).child(toast)))
}
