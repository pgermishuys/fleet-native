//! `SmartLinkCard.vue`: a pull request in the Context tab as a card (rows 12.12 and 12.13 of
//! docs/parity/08-session-header-and-context.md). A header with the icon, the title, the state pill and the meta line, and
//! under it the signals of an open pull request: Checks (expandable, with a Send to agent button on each failing run),
//! Reviews, unresolved review threads (expandable, a Send button on each) and merge conflicts. What each says is worked out in
//! `fleet::smart_link_card`; this file only draws it. The Send buttons are the popover's (`fleet::send_to_agent`).
use super::gh::{CheckKind, ItemKind, check_icon, checks_summary, diff_stat, label, pr_state_pill, reviewer_avatar};
use super::header_context::link_icon;
use super::smart_link_row::{action, flash};
use crate::State;
use crate::fleet::context_tab::target_key;
use crate::fleet::pr_state::PrState;
use crate::fleet::send_to_agent::{check_key, thread_key};
use crate::fleet::smart_link_card::{Card, review_summary};
use crate::fleet::smart_links::{
    CheckRun, CheckState, Relationship, ReviewThread, SmartLink, SummaryState, check_state, format_check_failure_prompt,
    format_review_thread_prompt,
};
use crate::icons::{
    CHECK, CHEVRON_DOWN, CHEVRON_RIGHT, EXTERNAL_LINK, GIT_MERGE, LOADER_CIRCLE, MESSAGE_SQUARE, PIN_OFF, SEND, TRIANGLE_ALERT, USERS, X,
};
use crate::theme::{RADIUS_CARD, Theme, mono, sans};
use rune::{Cursor, El, Tx, col, icon, row, text};

fn open_url(url: String) -> impl Fn(&mut State, &mut Tx) + Send + Sync + 'static {
    move |_: &mut State, tx: &mut Tx| {
        let _ = tx.open_url(&url);
    }
}

pub fn smart_link_card(s: &State, t: &Theme, link: &SmartLink) -> El {
    let c = Card::of(link);
    let id = link.id.clone();
    let mut signals: Vec<(Part, El)> = Vec::new();
    if c.merged {
        signals.push((
            Part::Signal,
            signal(
                t,
                format!("context-merged-{id}"),
                icon(&GIT_MERGE, 14.0).color(t.queued),
                "Merged",
                "No longer checked",
                Edge::Static,
                false,
            ),
        ));
    } else if c.open_pr {
        if c.checks.state != SummaryState::None {
            let by_default = c.checks_open_by_default();
            let open = s.context.checks_are_open(&id, by_default);
            let kind = match c.checks.state {
                SummaryState::Failing => CheckKind::Failure,
                SummaryState::Running => CheckKind::Pending,
                _ => CheckKind::Success,
            };
            let bad = c.checks.state == SummaryState::Failing;
            signals.push((
                Part::Signal,
                signal(t, format!("context-checks-{id}"), check_icon(t, kind, 14.0), "Checks", &c.checks.text, Edge::Chevron(open), bad)
                    .on_press({
                        let id = id.clone();
                        move |s: &mut State| s.context.toggle_checks(&id, by_default)
                    }),
            ));
            if let Some(counts) = c.counts {
                signals.push((
                    Part::Bar,
                    col().w_full().pl(34.0).pr(12.0).pb(8.0).child(checks_summary(t, &format!("context-checks-bar-{id}"), counts, true)),
                ));
            }
            if open {
                signals.push((Part::List, check_list(s, t, link, &c)));
            }
        }
        if !c.reviewers.is_empty() {
            let mut r = signal(
                t,
                format!("context-reviews-{id}"),
                icon(&USERS, 14.0).color(t.muted),
                "Reviews",
                &review_summary(&c.reviewers),
                Edge::Static,
                false,
            );
            // The avatars stand where the chevron does.
            let mut avatars = row().gap(5.0).pr(2.0).center_y();
            for rv in &c.reviewers {
                avatars =
                    avatars.child(reviewer_avatar(t, &format!("context-reviewer-{id}-{}", rv.login), &rv.login, Some(rv.state), 16.0));
            }
            r = r.child(avatars);
            signals.push((Part::Signal, r));
        }
        if !c.threads.is_empty() {
            let open = s.context.review_open.contains(&id);
            let n = c.threads.len();
            let detail = format!("{n} unresolved thread{}", if n == 1 { "" } else { "s" });
            signals.push((
                Part::Signal,
                signal(
                    t,
                    format!("context-review-{id}"),
                    icon(&MESSAGE_SQUARE, 14.0).color(t.status_waiting()),
                    "Review",
                    &detail,
                    Edge::Chevron(open),
                    false,
                )
                .on_press({
                    let id = id.clone();
                    move |s: &mut State| s.context.toggle_review(&id)
                }),
            ));
            if open {
                signals.push((Part::List, thread_list(s, t, link, &c)));
            }
        }
        if c.conflict {
            let detail = c.base_ref.as_ref().map(|b| format!("with {b}")).unwrap_or_default();
            signals.push((
                Part::Signal,
                signal(
                    t,
                    format!("context-conflict-{id}"),
                    icon(&TRIANGLE_ALERT, 14.0).color(t.status_waiting()),
                    "Merge conflicts",
                    &detail,
                    Edge::Static,
                    true,
                ),
            ));
        }
    }
    let border = if c.attention { t.border_mix(t.pr_tone(PrState::Blocked), 35.0) } else { t.border };
    let mut card = col()
        .key(target_key(&link.id))
        .role("article")
        .label(format!(
            "{} #{}: {}",
            if crate::fleet::smart_links::is_pull_request(link) { "Pull request" } else { "Issue" },
            c.number,
            c.title
        ))
        .w_full()
        .items_stretch()
        .fill(t.card_bg)
        .border(1.0, border)
        .border_layout()
        .radius(RADIUS_CARD)
        .clip_rounded(RADIUS_CARD)
        .child(head(t, link, &c));
    if !signals.is_empty() {
        card = card.child(col().w_full().items_stretch().border_sides([1.0, 0.0, 0.0, 0.0], t.border).children(ruled(t, signals)));
    }
    if let Some(sentence) = s.agent_sends.error(&link.session_id) {
        card = card.child(
            text(sentence.to_string())
                .key(format!("context-card-error-{id}"))
                .role("alert")
                .font(sans(11.5, 400.0, 1.3))
                .color(t.error_text)
                .wrap()
                .px(12.0)
                .pb(10.0),
        );
    }
    flash(card, t, s, &link.id)
}

/// The parts of the signals column, in order.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Part {
    Signal,
    List,
    Bar,
}

/// Puts the rule above every signal that follows a signal or a list (the stylesheet's two adjacent-sibling rules). A bar
/// sits between a signal and the next, and breaks the adjacency, as it does in Fleet's client.
fn ruled(t: &Theme, parts: Vec<(Part, El)>) -> Vec<El> {
    let mut out = Vec::new();
    let mut prev = Part::Bar;
    for (part, el) in parts {
        let rule = part == Part::Signal && prev != Part::Bar && !out.is_empty();
        out.push(if rule { el.border_sides([1.0, 0.0, 0.0, 0.0], t.border) } else { el });
        prev = part;
    }
    out
}

fn head(t: &Theme, link: &SmartLink, c: &Card) -> El {
    let id = &link.id;
    let title = text(c.title.clone())
        .key(format!("context-link-title-{id}"))
        .role("link")
        .font(sans(13.0, 500.0, 1.35))
        .color(t.text)
        .wrap()
        .cursor(Cursor::Pointer)
        .help(c.href.clone())
        .on_press_tx(open_url(c.href.clone()));
    let mut meta = row().w_full().flex_wrap().center_y().gap(6.0);
    if let Some(p) = &c.pill {
        meta = meta.child(pr_state_pill(
            t,
            &format!("context-state-{id}"),
            if p.pull { ItemKind::Pull } else { ItemKind::Issue },
            p.state,
            &p.label,
        ));
    } else if let Some(note) = c.note {
        meta = meta.child(text(note).key(format!("context-note-{id}")).font(sans(11.5, 400.0, 1.3)).color(t.muted));
    }
    if let Some((head_ref, base_ref)) = &c.branch {
        meta = meta.child(
            text(format!("{head_ref} \u{2192} {base_ref}"))
                .key(format!("context-branch-{id}"))
                .font(mono(11.0, 400.0, 1.3))
                .color(t.muted)
                .truncate()
                .shrink()
                .min_w(0.0)
                .help(format!("{head_ref} into {base_ref}")),
        );
    }
    if let Some((additions, deletions)) = c.diff {
        meta = meta.child(diff_stat(t, &format!("context-diff-{id}"), additions, deletions));
    }
    for l in &c.labels {
        meta = meta.child(label(t, &format!("context-label-{id}-{}", l.name), &l.name, &l.color));
    }
    let number = row()
        .key(format!("context-number-{id}"))
        .role("link")
        .label(format!("Open #{} on GitHub", c.number))
        .h(22.0)
        .px(4.0)
        .gap(3.0)
        .center_y()
        .radius(5.0)
        .cursor(Cursor::Pointer)
        .hover_group()
        .focus_ring(2.0, -1.0, t.accent)
        .on_press_tx(open_url(c.href.clone()))
        .child(text(format!("#{}", c.number)).font(mono(11.5, 400.0, 1.2)).color(t.muted).group_color(t.text))
        .child(icon(&EXTERNAL_LINK, 11.0).color(t.muted).group_color(t.text));
    let mut trail = row().gap(2.0).items_start().child(number);
    let (session, lid) = (link.session_id.clone(), link.id.clone());
    if link.relationship == Relationship::Pinned {
        let (session, lid) = (session.clone(), lid.clone());
        trail = trail.child(action(t, format!("context-unpin-{id}"), &PIN_OFF, "Unpin".into(), move |s: &mut State| {
            s.set_link_pinned(&session, &lid, false)
        }));
    }
    if link.relationship != Relationship::Origin {
        trail = trail
            .child(action(t, format!("context-dismiss-{id}"), &X, "Dismiss".into(), move |s: &mut State| s.dismiss_link(&session, &lid)));
    }
    row()
        .w_full()
        .gap(8.0)
        .items_start()
        .pt(10.0)
        .pr(10.0)
        .pb(10.0)
        .pl(12.0)
        .child(row().w(16.0).mt(2.0).child(link_icon(t, link, 14.0)))
        .child(col().grow(1.0).shrink().min_w(0.0).gap(5.0).child(title).child(meta))
        .child(trail)
}

#[derive(Clone, Copy)]
enum Edge {
    Static,
    Chevron(bool),
}

/// `.signal`: the glyph, what it is, the detail at the right and the chevron. A bad one has the error's wash.
fn signal(t: &Theme, key: String, glyph: El, what: &str, detail: &str, edge: Edge, bad: bool) -> El {
    let mut r = row()
        .key(key)
        .label(format!("{what} {detail}"))
        .w_full()
        .px(12.0)
        .py(8.0)
        .gap(8.0)
        .center_y()
        .child(row().w(16.0).center_y().child(glyph))
        .child(text(what.to_string()).font(sans(12.5, 500.0, 1.3)).color(t.text))
        .child(row().grow(1.0).shrink().min_w(0.0).justify_end().child(
            text(detail.to_string()).font(sans(12.0, 400.0, 1.3).features("'tnum' 1")).color(t.muted).truncate().shrink().min_w(0.0),
        ));
    if bad {
        r = r.fill(t.feedback_red(0.06));
    }
    if let Edge::Chevron(open) = edge {
        r = r
            .role("button")
            .cursor(Cursor::Pointer)
            .focus_ring(2.0, -1.0, t.accent)
            .hover(if bad { t.feedback_red(0.10) } else { t.wash(4.0) })
            .child(row().w(12.0).child(icon(if open { &CHEVRON_DOWN } else { &CHEVRON_RIGHT }, 12.0).color(t.muted)));
    }
    r
}

fn spinner(t: &Theme) -> El {
    icon(&LOADER_CIRCLE, 11.0).color(t.muted).cycle(crate::view::ring_popover::spinner())
}

/// `.send-btn`: 22 tall, a border, the glyph and the words; the accent on hover; "Sent" in the running green.
fn send_button(s: &State, t: &Theme, link: &SmartLink, send_key: String, idle: &str, tip: String, text_for: String) -> El {
    let session = &link.session_id;
    let sent = s.agent_sends.is_sent(session, &send_key);
    let sending = s.agent_sends.is_sending(session, &send_key);
    let glyph = if sent {
        icon(&CHECK, 11.0).color(t.running_text)
    } else if sending {
        spinner(t)
    } else {
        icon(&SEND, 11.0).color(t.text).group_color(t.accent_text)
    };
    let words = if sent { "Sent" } else { idle };
    let ink = if sent { t.running_text } else { t.text };
    let word = text(words.to_string()).font(sans(11.5, 500.0, 1.2)).color(ink);
    let mut b = row()
        .key(format!("context-send-{}-{}", link.id, send_key.replace(':', "-")))
        .role("button")
        .label(if sent { words.to_string() } else { tip.clone() })
        .h(22.0)
        .px(8.0)
        .gap(4.0)
        .center_y()
        .radius(6.0)
        .fill(t.card_bg)
        .border(1.0, if sent { rune::Color::TRANSPARENT } else { t.border })
        .border_layout()
        .child(glyph);
    if sent || sending {
        b = b.disabled(true).child(word);
    } else {
        b = b
            .help(tip)
            .cursor(Cursor::Pointer)
            .hover_group()
            .focus_ring(2.0, -1.0, t.accent)
            .child(word.group_color(t.accent_text))
            .on_press(move |s: &mut State| s.send_to_agent(&send_key, text_for.clone()));
    }
    b
}

fn external(t: &Theme, key: String, url: String, tip: String) -> El {
    row()
        .key(key)
        .role("link")
        .label(tip)
        .size(20.0, 20.0)
        .center()
        .radius(5.0)
        .cursor(Cursor::Pointer)
        .hover_group()
        .focus_ring(2.0, -1.0, t.accent)
        .on_press_tx(open_url(url))
        .child(icon(&EXTERNAL_LINK, 11.0).color(t.muted).group_color(t.text))
}

fn run_kind(state: CheckState) -> CheckKind {
    match state {
        CheckState::Failing => CheckKind::Failure,
        CheckState::Running => CheckKind::Pending,
        CheckState::Passed => CheckKind::Success,
        CheckState::Neutral => CheckKind::Neutral,
    }
}

/// `.signal__list`: the runs, one line each.
fn list(key: String) -> El {
    col().key(key).w_full().items_stretch().gap(2.0).pl(36.0).pr(12.0).pt(2.0).pb(8.0)
}

fn check_list(s: &State, t: &Theme, link: &SmartLink, c: &Card) -> El {
    let mut l = list(format!("context-checks-list-{}", link.id));
    for run in &c.runs {
        l = l.child(run_row(s, t, link, run));
    }
    l
}

fn run_row(s: &State, t: &Theme, link: &SmartLink, run: &CheckRun) -> El {
    let state = check_state(run);
    let mut actions = row().gap(4.0).center_y();
    if state == CheckState::Failing {
        let name = &run.name;
        actions = actions.child(send_button(
            s,
            t,
            link,
            check_key(name),
            "Send to agent",
            format!("Send the {name} failure to the agent"),
            format_check_failure_prompt(link, run),
        ));
    } else if state == CheckState::Running {
        actions = actions.child(text("running").font(sans(11.0, 400.0, 1.2)).color(t.muted));
    }
    if !run.html_url.is_empty() {
        actions = actions.child(external(
            t,
            format!("context-run-open-{}-{}", link.id, run.name),
            run.html_url.clone(),
            format!("Open {} on GitHub", run.name),
        ));
    }
    row()
        .key(format!("context-run-{}-{}", link.id, run.name))
        .w_full()
        .min_h(26.0)
        .gap(7.0)
        .center_y()
        .child(row().w(14.0).child(check_icon(t, run_kind(state), 12.0)))
        .child(
            text(run.name.clone())
                .font(sans(12.0, 400.0, 1.3))
                .color(t.text)
                .truncate()
                .grow(1.0)
                .shrink()
                .min_w(0.0)
                .help(run.name.clone()),
        )
        .child(actions)
}

fn thread_list(s: &State, t: &Theme, link: &SmartLink, c: &Card) -> El {
    let mut l = list(format!("context-review-list-{}", link.id));
    for th in &c.threads {
        l = l.child(thread_row(s, t, link, th));
    }
    l
}

fn thread_row(s: &State, t: &Theme, link: &SmartLink, th: &ReviewThread) -> El {
    let tid = &th.thread_node_id;
    let file = th.path.rsplit('/').next().unwrap_or("");
    let at = format!("{file}{}", th.line.map(|n| format!(":{n}")).unwrap_or_default());
    let full = format!("{}{}", th.path, th.line.map(|n| format!(":{n}")).unwrap_or_default());
    let mut name = col().grow(1.0).shrink().min_w(0.0).child(text(at).font(mono(11.0, 400.0, 1.3)).color(t.text).truncate().help(full));
    if let Some(first) = th.comments.first() {
        name = name.child(
            text(format!("{}: {}", first.author_login, first.body))
                .font(sans(11.5, 400.0, 1.3))
                .color(t.muted)
                .truncate()
                .help(first.body.clone()),
        );
    }
    let mut actions = row().gap(4.0).center_y().child(send_button(
        s,
        t,
        link,
        thread_key(tid),
        "Send",
        "Send this review comment to the agent".into(),
        format_review_thread_prompt(link, th),
    ));
    if let Some(url) = th.comments.first().map(|c| c.url.clone()).filter(|u| !u.is_empty()) {
        actions = actions.child(external(
            t,
            format!("context-thread-open-{}-{tid}", link.id),
            url,
            format!("Open the thread on {} on GitHub", th.path),
        ));
    }
    row()
        .key(format!("context-thread-{}-{tid}", link.id))
        .w_full()
        .gap(7.0)
        .items_start()
        .py(3.0)
        .child(row().w(14.0).mt(3.0).child(icon(&MESSAGE_SQUARE, 12.0).color(t.status_waiting())))
        .child(name)
        .child(actions)
}
