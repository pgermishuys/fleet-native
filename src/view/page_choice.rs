//! The bar under a page that waits for a pick (component 14, piece C): `.page-canvas__choice` of
//! `components/canvas/PageCanvas.vue`. The page only proposes; this bar sends. The words come from `fleet::web_page`.
use crate::State;
use crate::fleet::progress::format_time;
use crate::fleet::web_page::{BarKind, ShownPage, page_choice_bar};
use crate::icons::*;
use crate::theme::{RADIUS_BTN, Theme, sans};
use crate::web::choice::Answer;
use rune::{Color, Cursor, El, Span, el, icon, rich, row, text};

/// The bar for the page's choice, or `None` for a page that waits on nothing.
pub fn choice_bar(s: &State, t: &Theme, tab: &str, page: &ShownPage) -> Option<El> {
    let choice = page.choice.as_ref()?;
    let ui = s.choice_ui(tab, choice);
    let live = s.choice_is_live(choice);
    let bar = page_choice_bar(choice, live, ui.proposal.as_ref(), &ui.refusal);
    let sending = ui.sending == Some(choice.status);
    let live_bar = matches!(bar.kind, BarKind::Waiting | BarKind::Proposed | BarKind::Refused);
    let ink = if live_bar { t.text } else { t.muted };

    // The sentence, the bold pick, then the time and any error in 11 px after a gap, all one flowing line.
    let (mut words, mut spans) = (bar.text.clone(), Vec::new());
    if let Some(strong) = &bar.strong {
        let at = words.len();
        words.push_str(strong);
        spans.push(Span::new(at, words.len()).weight(600.0).color(t.text));
    }
    words.push_str(&bar.after);
    let small = |words: &mut String, spans: &mut Vec<Span>, add: &str, color: Color| {
        words.push_str("   ");
        let at = words.len();
        words.push_str(add);
        spans.push(Span::new(at, words.len()).size(11.0).color(color));
    };
    // Only an answer shows its time; an unanswered bar has the time in its state and Fleet doesn't draw it.
    if bar.kind == BarKind::Answered {
        let time = format_time(bar.at.as_deref(), crate::conv::time::local_offset_secs());
        if !time.is_empty() {
            small(&mut words, &mut spans, &format!("Answered {time}"), ink);
        }
    }
    if !ui.error.is_empty() {
        small(&mut words, &mut spans, &ui.error, t.error_text);
    }
    let sentence = rich(words, spans).key("page-choice-text").font(sans(12.0, 400.0, 1.4)).color(ink).wrap();
    let mut message = row().grow(1.0).shrink().min_w(0.0).gap(6.0);
    if bar.kind == BarKind::Refused {
        message = message.role("alert").child(icon(&TRIANGLE_ALERT, 13.0).color(t.status_waiting_text).mt(2.0));
    }
    let message = message.child(sentence.grow(1.0).shrink().min_w(0.0));

    let mut r = row().key("page-choice-bar").w_full().px(12.0).py(8.0).gap(8.0).center_y().border_layout();
    r = if live_bar {
        r.border_sides([1.0, 0.0, 0.0, 0.0], t.border_mix(t.status_waiting(), 40.0)).fill(t.status_waiting().alpha(0.06))
    } else {
        r.border_sides([1.0, 0.0, 0.0, 0.0], t.border)
    };
    r = r.child(message);
    if !live_bar {
        return Some(r);
    }
    let tab = tab.to_string();
    let first = if bar.kind == BarKind::Proposed {
        let tab = tab.clone();
        button(t, "page-choice-change", "Change", false, sending).on_press(move |s: &mut State| s.change_choice(&tab))
    } else {
        let tab = tab.clone();
        button(t, "page-choice-skip", "Skip", false, sending).on_press(move |s: &mut State| s.answer_choice(&tab, Answer::Skip))
    };
    let send_off = sending || bar.kind != BarKind::Proposed;
    let send = button(t, "page-choice-send", if sending { "Sending\u{2026}" } else { "Send to agent" }, true, send_off);
    let send = if bar.kind == BarKind::Refused { send.help(bar.text.clone()) } else { send };
    Some(r.child(first).child(send.on_press(move |s: &mut State| s.answer_choice(&tab, Answer::Send))))
}

/// `.page-canvas__choice-btn`: 12 px, padding 4 10, a border; primary is the accent. Off at half strength.
fn button(t: &Theme, key: &str, words: &str, primary: bool, off: bool) -> El {
    let (ink, fill, border, hover) = if primary {
        (Color::WHITE, t.accent, t.accent, super::toolcard::mix(t.accent, Color::WHITE, 0.1))
    } else {
        (t.text, Color::TRANSPARENT, t.border, t.accent_dim)
    };
    let b = el()
        .key(key.to_string())
        .role("button")
        .label(words.to_string())
        .px(10.0)
        .py(4.0)
        .radius(RADIUS_BTN)
        .fill(fill)
        .border(1.0, border)
        .border_layout()
        .child(text(words.to_string()).font(sans(12.0, 400.0, 1.3)).color(ink));
    if off {
        return b.opacity(0.5).disabled(true).cursor(Cursor::Default);
    }
    b.hover(hover).cursor(Cursor::Pointer).focusable(true).focus_ring(2.0, 0.0, t.accent)
}
