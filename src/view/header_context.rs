//! The header's context chips and the pull request pill's trigger (ports `SessionContextChips.vue` and the trigger of
//! `PullRequestPill.vue`, rows 10.x and 11.1 of docs/parity/08-session-header-and-context.md). What they say comes from
//! `fleet::context_chips`; this file only draws it. The pill's popover is `pr_popover`.
//!
//! Nothing here knows GitHub: the chips draw generic smart links, and a pull request is whatever has `resource_type` "pull_request".
use super::gh::{CheckKind, ItemKind, check_icon, item_icon};
use crate::State;
use crate::fleet::context_chips::{ContextChips, LinkChip, Pill};
use crate::fleet::pr_state::{Checks, PrState};
use crate::fleet::smart_links::{Enrichment, is_pull_request};
use crate::icons::{CHEVRON_DOWN, CIRCLE_DOT, GIT_PULL_REQUEST, ZAP};
use crate::theme::{RADIUS_BTN, Theme, mono, sans};
use rune::{Cursor, El, icon, row, text};

/// The row at the right of the title block, `None` when the session has no context (the row isn't drawn).
pub fn context_chips(s: &State, t: &Theme) -> Option<El> {
    let session = s.session();
    let chips = ContextChips::of(&s.smart_links, &session.id, session.row.origin.as_ref())?;
    let mut line = row().key("context-chips").role("list").label("Session context").center_y().gap(6.0).shrink().min_w(0.0);
    if let Some(title) = &chips.automation {
        line = line.child(automation_chip(t, title));
    }
    if let Some(p) = &chips.pending {
        let glyph = if p.is_pull_request { &GIT_PULL_REQUEST } else { &CIRCLE_DOT };
        let tip = format!("Started from {} #{}: {}", if p.is_pull_request { "pull request" } else { "issue" }, p.number, p.title);
        line = line.child(origin_button(t, "context-origin-pending", &tip, &p.number, icon(glyph, 13.0).color(t.muted), "origin".into()));
    }
    if let Some(o) = &chips.origin {
        let key = format!("context-chip-{}", o.link.id);
        line = line.child(origin_button(t, &key, &o.description, &o.number, link_icon(t, &o.link, 13.0), o.link.id.clone()));
    }
    if let Some(pill) = &chips.pill {
        line = line.child(pill_trigger(t, pill));
    }
    for c in &chips.links {
        line = line.child(link_chip(t, c));
    }
    if chips.overflow > 0 {
        line = line.child(overflow_chip(t, chips.overflow, chips.overflow_target.clone()));
    }
    Some(line)
}

/// `SmartLinkIcon`: the link's own icon in its state's colour; neutral until GitHub has answered.
pub(super) fn link_icon(t: &Theme, link: &crate::fleet::smart_links::SmartLink, size: f32) -> El {
    let resolved = link.enrichment == Enrichment::Resolved;
    if !is_pull_request(link) {
        let state = if resolved && link.status == "closed" {
            PrState::Closed
        } else if resolved {
            PrState::Open
        } else {
            PrState::Draft
        };
        return item_icon(t, ItemKind::Issue, state, size);
    }
    let state = if resolved { crate::fleet::pr_state::pr_state(&crate::fleet::smart_links::link_pr_facts(link)) } else { PrState::Draft };
    item_icon(t, ItemKind::Pull, state, size)
}

/// Opens the Context tab at what was pressed (`show`): the panel opens and the tab is asked to flash that card.
fn show(target: String) -> impl Fn(&mut State) + Send + Sync + 'static {
    move |s: &mut State| s.show_in_context(&target)
}

fn chip_frame(t: &Theme, key: &str, tip: &str, aria: String) -> El {
    row()
        .key(key.to_string())
        .role("listitem")
        .h(26.0)
        .px(9.0)
        .gap(5.0)
        .center_y()
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .border_layout()
        .hover(t.wash(5.0))
        .cursor(Cursor::Pointer)
        .help(tip.to_string())
        .label(aria)
}

fn number_text(t: &Theme, number: &str) -> El {
    text(format!("#{number}")).font(mono(11.5, 400.0, 1.2)).color(t.text)
}

fn automation_chip(t: &Theme, title: &str) -> El {
    let tip = format!("Started by automation: {title}");
    chip_frame(t, "context-automation", &tip, format!("Started by automation {title}. Show in Context"))
        .max_w(220.0)
        .on_press(show("origin".into()))
        .child(icon(&ZAP, 13.0).color(t.status_waiting_text))
        .child(text(title.to_string()).font(sans(12.0, 400.0, 1.2)).color(t.text).truncate().shrink().min_w(0.0))
}

fn link_chip(t: &Theme, c: &LinkChip) -> El {
    let mut frame = chip_frame(t, &format!("context-chip-{}", c.link.id), &c.description, format!("{}. Show in Context", c.description));
    if c.attention {
        // `--pr-blocked` at 45%.
        frame = frame.border(1.0, t.pr_tone(PrState::Blocked).alpha(0.45));
    }
    frame.on_press(show(c.link.id.clone())).child(link_icon(t, &c.link, 13.0)).child(number_text(t, &c.number))
}

fn overflow_chip(t: &Theme, n: usize, target: String) -> El {
    chip_frame(t, "context-more", "", format!("{n} more linked pull requests and issues. Show in Context"))
        .on_press(show(target))
        .child(text(format!("+{n}")).font(sans(12.0, 400.0, 1.2)).color(t.muted))
}

/// `.context-origin`: where the session came from, quiet: no border, muted, "from #N".
fn origin_button(t: &Theme, key: &str, tip: &str, number: &str, glyph: El, target: String) -> El {
    row()
        .key(key.to_string())
        .role("listitem")
        .h(26.0)
        .px(7.0)
        .center_y()
        .gap(5.0)
        .radius(RADIUS_BTN)
        .hover(t.wash(5.0))
        .cursor(Cursor::Pointer)
        .help(tip.to_string())
        .label(format!("{tip}. Show in Context"))
        .on_press(show(target))
        .child(glyph)
        .child(text("from").font(sans(12.0, 400.0, 1.2)).color(t.muted))
        .child(text(format!("#{number}")).font(mono(11.5, 400.0, 1.2)).color(t.muted))
}

/// The pill's trigger: a number part in the state's colour and, once GitHub has answered, the state in words.
fn pill_trigger(t: &Theme, p: &Pill) -> El {
    let tone = t.pr_tone(p.state);
    let ink = t.pr_tone_text(p.state);
    let number = row()
        .key("session-pr-number")
        .center_y()
        .gap(4.0)
        .px(8.0)
        .fill(tone.alpha(0.12))
        .child(item_icon(t, ItemKind::Pull, p.state, 13.0))
        .child(text(format!("#{}", p.number)).font(mono(11.5, 500.0, 1.2)).color(ink));
    let mut frame = row()
        .key("session-pr-pill")
        .role("listitem")
        .h(26.0)
        .max_w(280.0)
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .border_layout()
        .hover(t.wash(4.0))
        .cursor(Cursor::Pointer)
        .help(p.description.clone())
        .label(p.description.clone())
        .on_press(|s: &mut State| s.ui.open = Some(crate::ui::Open::Popover("pr")));
    frame = frame.clip_rounded(RADIUS_BTN).child(number);
    if p.resolved {
        let mut words = row().key("session-pr-words").center_y().gap(5.0).px(8.0).shrink().min_w(0.0);
        if let Some(checks) = p.checks {
            let kind = match checks {
                Checks::Failing => CheckKind::Failure,
                Checks::Pending => CheckKind::Pending,
                _ => CheckKind::Success,
            };
            words = words.child(check_icon(t, kind, 12.0));
        }
        words = words
            .child(text(p.words.clone()).font(sans(12.0, 400.0, 1.2)).color(t.muted).truncate().shrink().min_w(0.0))
            .child(icon(&CHEVRON_DOWN, 12.0).color(t.muted));
        frame = frame.child(words);
    }
    frame
}
