//! Ports `components/canvas/ProgressSubagentCard.vue`: a subagent in the Progress tab, what it's doing, its own count, and a
//! link to its session. Placed under a step (`ProgressCanvas.vue`'s step detail) or in the "Subagents" list at the bottom.
use crate::State;
use crate::fleet::delegations::DelegationStatus;
use crate::fleet::progress::{Subagent, subagent_title};
use crate::icons::BOT;
use crate::theme::{RADIUS_BTN, Theme, sans};
use rune::{Cursor, El, Span, Tx, col, el, icon, rich, row, text};

/// The status line: `completed` Finished, `error` Failed, `cancelled` Cancelled, `running` "Now: …" or Working, else Starting.
pub fn status_words(a: &Subagent) -> String {
    match a.status {
        DelegationStatus::Completed => "Finished".into(),
        DelegationStatus::Error => "Failed".into(),
        DelegationStatus::Cancelled => "Cancelled".into(),
        DelegationStatus::Running => match a.current.as_deref().filter(|c| !c.is_empty()) {
            Some(now) => format!("Now: {now}"),
            None => "Working".into(),
        },
        DelegationStatus::Pending => "Starting".into(),
    }
}

fn finished(a: &Subagent) -> bool {
    matches!(a.status, DelegationStatus::Completed | DelegationStatus::Error | DelegationStatus::Cancelled)
}

/// One card. `parent` is the session whose Progress tab this is.
pub fn subagent_card(t: &Theme, a: &Subagent, parent: &str) -> El {
    let (mut content, mut spans) = (a.agent.clone(), vec![Span::new(0, a.agent.len()).weight(600.0)]);
    if let Some(task) = subagent_title(a) {
        content.push_str(&format!(" · {task}"));
    }
    let mut top = row().w_full().gap(8.0).center_y().child(icon(&BOT, 14.0).color(t.muted)).child(
        rich(std::mem::take(&mut content), std::mem::take(&mut spans))
            .font(sans(12.0, 400.0, 1.5))
            .color(t.text)
            .truncate()
            .grow(1.0)
            .min_w(0.0),
    );
    if a.total > 0 {
        top = top.child(text(format!("{}/{}", a.done, a.total)).font(sans(11.5, 400.0, 1.5).features("'tnum' 1")).color(t.muted));
    }
    let mut inner = col().w_full().gap(4.0).px(9.0).py(7.0).radius(RADIUS_BTN - 1.0).fill(t.card_bg).items_stretch().child(top);
    if a.total > 0 {
        let fill = el().key("subagent-bar-fill").w_frac((a.done as f32 / a.total as f32).clamp(0.0, 1.0)).h(3.0).fill(if finished(a) {
            t.running
        } else {
            t.accent
        });
        inner = inner.child(el().key("subagent-bar").w_full().h(3.0).radius(2.0).fill(t.wash(9.0)).clip_rounded(2.0).child(fill));
    }
    inner = inner.child(text(status_words(a)).key("subagent-status").font(sans(11.5, 400.0, 1.5)).color(t.muted).truncate());
    // The 1 px border is a ring behind the card, so hover can lift it to `mix(--text 18%)`.
    let card = col().key(format!("subagent-{}", a.delegation_id)).w_full().padding(1.0).radius(RADIUS_BTN).fill(t.border).items_stretch();
    match a.child_session_id.clone() {
        Some(child) => {
            let (parent, title) = (parent.to_string(), a.title.clone().unwrap_or_else(|| a.agent.clone()));
            card.role("link")
                .label(format!("{} {}", a.agent, status_words(a)))
                .cursor(Cursor::Pointer)
                .hover(t.wash(18.0))
                .focusable(true)
                .focus_ring(2.0, 1.0, t.accent)
                .on_press_tx(move |s: &mut State, tx: &mut Tx| s.open_subagent_session(&parent, &child, &title, tx))
                .child(inner)
        }
        None => card.child(inner),
    }
}

/// The step's cards, as the step's detail column holds them.
pub fn cards(t: &Theme, list: &[&Subagent], parent: &str) -> El {
    list.iter().fold(col().w_full().gap(6.0).items_stretch(), |c, a| c.child(subagent_card(t, a, parent)))
}

/// `progress-canvas__subagents`: the label "Subagents" and a card each, for the ones no step shows.
pub fn unplaced_list(t: &Theme, list: &[&Subagent], parent: &str) -> El {
    col()
        .key("progress-subagents")
        .role("region")
        .label("Subagents")
        .w_full()
        .gap(6.0)
        .items_stretch()
        .child(super::progress_tab::eyebrow(t, "progress-subagents-label", "Subagents"))
        .child(cards(t, list, parent))
}
