//! The popover under the header's pull request pill (ports the popover of `PullRequestPill.vue`, rows 11.3 to 11.8 of
//! docs/parity/08-session-header-and-context.md): the state, the checks and review threads with a Send to agent button
//! each, and a footer that does the next thing. What it says is worked out in `fleet::pr_popover`; this file only draws it.
//!
//! Like the chips it reads a generic smart link; a plugin that fills the place gets the same popover.
use super::gh::{CheckKind, ItemKind, check_icon, checks_summary, diff_stat, pr_state_pill, reviewer_avatar};
use crate::State;
use crate::fleet::pr_popover::{ListedRun, PrPopover, Primary, RunEnd};
use crate::fleet::send_to_agent::{FIX, check_key, thread_key};
use crate::fleet::smart_links::{CheckState, ReviewThread};
use crate::icons::{ARCHIVE, CHECK, EXTERNAL_LINK, GIT_MERGE, LOADER_CIRCLE, MESSAGE_SQUARE, SEND, SPARKLES, TRIANGLE_ALERT};
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, mono, sans};
use crate::ui::overlay::{anchored, scrim};
use rune::{Align, Cursor, El, Side, Tx, col, el, icon, row, text};

/// The pill the card is placed under.
pub const TRIGGER: &str = "session-pr-pill";
const WIDTH: f32 = 380.0;

fn close(s: &mut State) {
    s.ui.open = None;
}

/// The scrim and the card (`side="bottom" align="end" :side-offset="6"`), or nothing when the session has no pull request.
pub fn overlay(s: &State, t: &Theme) -> Vec<El> {
    let id = s.session().id.clone();
    let Some(link) = s.smart_links.session_pull_request(&id) else { return Vec::new() };
    let m = PrPopover::of(link);
    let mut card = col()
        .key("session-pr-popover")
        .role("dialog")
        .label("Pull request")
        .w(WIDTH)
        .items_stretch()
        .fill(t.panel_bg)
        .border(1.0, t.border)
        .border_layout()
        .radius(RADIUS_CARD)
        .clip_rounded(RADIUS_CARD)
        .child(head(t, &m));
    if m.merged {
        card = card.child(merged(t, &m));
    } else if m.open {
        if m.has_runs {
            card = card.child(checks(s, t, &m, &id));
        }
        if m.shows_review() {
            card = card.child(review(s, t, &m, &id));
        }
        if m.conflict {
            card = card.child(conflict(t, &m));
        }
    }
    if let Some(sentence) = s.agent_sends.error(&id) {
        card = card.child(
            text(sentence.to_string())
                .key("session-pr-error")
                .role("alert")
                .font(sans(12.0, 400.0, 1.3))
                .color(t.error_text)
                .wrap()
                .px(14.0)
                .py(8.0),
        );
    }
    card = card.child(foot(s, t, &m, &id));
    let card =
        t.menu_shadows().into_iter().fold(card, |e, shadow| e.box_shadow(shadow)).focus_return(TRIGGER.to_string()).modal(true, close);
    vec![scrim("session-pr-scrim", close), anchored(card, TRIGGER, Side::Bottom, Align::End, 6.0)]
}

fn section(t: &Theme, key: &str, label: &str) -> El {
    col()
        .key(key.to_string())
        .label(label.to_string())
        .w_full()
        .items_stretch()
        .gap(8.0)
        .px(14.0)
        .py(10.0)
        .border_sides([0.0, 0.0, 1.0, 0.0], t.border)
}

/// `.pr-popover__branch`: a branch name in mono on the accent's wash.
fn branch(t: &Theme, name: &str) -> El {
    row()
        .px(6.0)
        .py(1.0)
        .radius(5.0)
        .max_w(180.0)
        .shrink()
        .min_w(0.0)
        .fill(t.accent_dim)
        .child(text(name.to_string()).font(mono(11.0, 400.0, 1.3)).color(t.text).truncate().shrink().min_w(0.0))
}

fn open_in_browser(url: String) -> impl Fn(&mut State, &mut Tx) + Send + Sync + 'static {
    move |_: &mut State, tx: &mut Tx| {
        let _ = tx.open_url(&url);
    }
}

fn head(t: &Theme, m: &PrPopover) -> El {
    let meta = || row().w_full().center_y().gap(6.0).min_w(0.0);
    let mut first = meta()
        .child(pr_state_pill(t, "session-pr-state", ItemKind::Pull, m.state, m.label))
        .child(text(format!("{} #{}", m.repo, m.number)).font(sans(11.5, 400.0, 1.2)).color(t.muted).truncate().shrink().min_w(0.0));
    if let Some((additions, deletions)) = m.diff {
        first = first.child(el().grow(1.0)).child(diff_stat(t, "session-pr-diff", additions, deletions));
    }
    let title = text(m.title.clone())
        .key("session-pr-title")
        .role("link")
        .font(sans(13.0, 600.0, 1.35))
        .color(t.text)
        .wrap()
        .cursor(Cursor::Pointer)
        .help(m.href.clone())
        .on_press_tx(open_in_browser(m.href.clone()));
    let mut second = meta();
    if let Some(head_ref) = &m.head_ref {
        second = second
            .child(branch(t, head_ref))
            .child(text("\u{2192}").font(sans(11.5, 400.0, 1.2)).color(t.muted))
            .child(branch(t, &m.base_ref));
    }
    if !m.reviewers.is_empty() {
        let mut avatars = row().key("session-pr-reviewers").gap(6.0).pr(2.0).center_y();
        for r in &m.reviewers {
            avatars = avatars.child(reviewer_avatar(t, &format!("session-pr-reviewer-{}", r.login), &r.login, Some(r.state), 18.0));
        }
        second = second.child(el().grow(1.0)).child(avatars);
    }
    col()
        .w_full()
        .items_stretch()
        .gap(6.0)
        .pt(12.0)
        .px(14.0)
        .pb(10.0)
        .border_sides([0.0, 0.0, 1.0, 0.0], t.border)
        .child(first)
        .child(title)
        .child(second)
}

fn line(t: &Theme, glyph: El, words: El) -> El {
    let _ = t;
    row().w_full().center_y().gap(8.0).child(glyph).child(words)
}

fn merged(t: &Theme, m: &PrPopover) -> El {
    let tone = t.pr_tone_text(crate::fleet::pr_state::PrState::Merged);
    let words = row().center_y().gap(4.0).child(text("Merged into").font(sans(12.5, 500.0, 1.3)).color(tone)).child(branch(t, &m.base_ref));
    section(t, "session-pr-merged", "Merged").child(line(t, icon(&GIT_MERGE, 14.0).color(tone), words)).child(
        text("Nothing is left to do on this branch. You can archive the session.").font(sans(12.0, 400.0, 1.35)).color(t.muted).wrap(),
    )
}

fn run_kind(state: CheckState) -> CheckKind {
    match state {
        CheckState::Failing => CheckKind::Failure,
        CheckState::Running => CheckKind::Pending,
        CheckState::Passed => CheckKind::Success,
        CheckState::Neutral => CheckKind::Neutral,
    }
}

fn spinner(t: &Theme, size: f32) -> El {
    icon(&LOADER_CIRCLE, size).color(t.muted).cycle(crate::view::ring_popover::spinner())
}

/// `.pr-popover__send`: the small button beside a failing check or a thread.
fn send_button(s: &State, t: &Theme, session: &str, key: &str, idle: &str, tip: String, spin: bool) -> El {
    let sent = s.agent_sends.is_sent(session, key);
    let sending = s.agent_sends.is_sending(session, key);
    let glyph = if sent {
        icon(&CHECK, 11.0).color(t.muted)
    } else if sending && spin {
        spinner(t, 11.0)
    } else {
        icon(&SEND, 11.0).color(t.text)
    };
    let words = if sent { "Sent" } else { idle };
    let off = sent || sending;
    let mut b = row()
        .key(format!("session-pr-send-{}", key.replace(':', "-")))
        .role("button")
        .h(22.0)
        .px(7.0)
        .gap(4.0)
        .center_y()
        .radius(6.0)
        .border(1.0, t.border)
        .border_layout()
        .help(tip.clone())
        .label(tip)
        .child(glyph)
        .child(text(words.to_string()).font(sans(11.5, 400.0, 1.2)).color(if off { t.muted } else { t.text }));
    if off {
        b = b.disabled(true);
    } else {
        b = b.hover(t.wash(6.0)).cursor(Cursor::Pointer);
    }
    b
}

fn run_row(s: &State, t: &Theme, session: &str, m: &PrPopover, r: &ListedRun) -> El {
    let name = &r.run.name;
    let mut line = row()
        .key(format!("session-pr-run-{name}"))
        .w_full()
        .min_h(24.0)
        .center_y()
        .gap(8.0)
        .child(check_icon(t, run_kind(r.state), 13.0))
        .child(text(name.clone()).font(sans(12.5, 400.0, 1.3)).color(t.text).truncate().grow(1.0).shrink().min_w(0.0));
    line = match &r.end {
        RunEnd::Send => {
            let key = check_key(name);
            let send = send_button(s, t, session, &key, "Send to agent", format!("Send the {name} failure to the agent"), true);
            let text_for = crate::fleet::smart_links::format_check_failure_prompt(&m.link, &r.run);
            line.child(send.on_press(move |s: &mut State| s.send_to_agent(&key, text_for.clone())))
        }
        RunEnd::Word(word) => line.child(text(word.clone()).font(sans(11.5, 400.0, 1.2)).color(t.muted)),
    };
    line
}

fn checks(s: &State, t: &Theme, m: &PrPopover, session: &str) -> El {
    let mut sec = section(t, "session-pr-checks", "Checks");
    if let Some(counts) = m.counts {
        sec = sec.child(checks_summary(t, "session-pr-checks-summary", counts, false));
    }
    if !m.listed.is_empty() {
        let mut list = col().w_full().items_stretch().gap(2.0);
        for r in &m.listed {
            list = list.child(run_row(s, t, session, m, r));
        }
        sec = sec.child(list);
    }
    sec
}

fn thread_row(s: &State, t: &Theme, session: &str, m: &PrPopover, th: &ReviewThread) -> El {
    let id = &th.thread_node_id;
    let path = format!("{}{}", th.path, th.line.map(|l| format!(":{l}")).unwrap_or_default());
    let mut line = row().key(format!("session-pr-thread-{id}")).w_full().gap(8.0).py(3.0);
    if let Some(first) = th.comments.first() {
        line = line.child(reviewer_avatar(t, &format!("session-pr-thread-avatar-{id}"), &first.author_login, None, 18.0));
    }
    let quote = th.comments.first().map(|c| c.body.clone()).unwrap_or_default();
    let body = col()
        .grow(1.0)
        .shrink()
        .min_w(0.0)
        .child(text(path).font(mono(11.0, 400.0, 1.3)).color(t.muted).truncate())
        .child(text(quote).font(sans(12.5, 400.0, 1.3)).color(t.text).max_lines(2));
    let key = thread_key(id);
    let send = send_button(s, t, session, &key, "Send", "Send this review comment to the agent".into(), false);
    let text_for = crate::fleet::smart_links::format_review_thread_prompt(&m.link, th);
    line.child(body).child(send.on_press(move |s: &mut State| s.send_to_agent(&key, text_for.clone())))
}

fn review(s: &State, t: &Theme, m: &PrPopover, session: &str) -> El {
    let mut sec = section(t, "session-pr-review", "Review");
    let words = text(m.review_line().unwrap_or_default()).font(sans(12.5, 500.0, 1.3)).color(t.text);
    sec = sec.child(line(t, icon(&MESSAGE_SQUARE, 14.0).color(t.pr_tone_text(crate::fleet::pr_state::PrState::Blocked)), words));
    if !m.threads.is_empty() {
        let mut list = col().w_full().items_stretch().gap(2.0);
        for th in m.threads.iter().take(crate::fleet::pr_popover::MAX_THREADS) {
            list = list.child(thread_row(s, t, session, m, th));
        }
        sec = sec.child(list);
    }
    sec
}

fn conflict(t: &Theme, m: &PrPopover) -> El {
    let words =
        row().center_y().gap(4.0).child(text("Conflicts with").font(sans(12.5, 500.0, 1.3)).color(t.text)).child(branch(t, &m.base_ref));
    section(t, "session-pr-conflict", "Conflicts").child(line(
        t,
        icon(&TRIANGLE_ALERT, 14.0).color(t.pr_tone_text(crate::fleet::pr_state::PrState::Blocked)),
        words,
    ))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Look {
    Plain,
    Primary,
    Ghost,
}

/// `.pr-popover__btn`: 28 tall, the glyph and the words.
fn button(t: &Theme, key: &str, look: Look, glyph: El, words: Option<&str>) -> El {
    let ink = match look {
        Look::Plain => t.text,
        Look::Primary => t.primary_fg,
        Look::Ghost => t.muted,
    };
    let mut b = row()
        .key(key.to_string())
        .role("button")
        .h(28.0)
        .px(10.0)
        .gap(6.0)
        .center_y()
        .radius(RADIUS_BTN)
        .cursor(Cursor::Pointer)
        .child(glyph);
    b = match look {
        Look::Plain => b.fill(t.card_bg).border(1.0, t.border).border_layout().hover(t.wash(6.0)),
        Look::Primary => b.fill(t.accent),
        Look::Ghost => b.hover(t.wash(6.0)).hover_group(),
    };
    if let Some(words) = words {
        let label = text(words.to_string()).font(sans(12.5, 500.0, 1.2)).color(ink);
        b = b.label(words.to_string()).child(if look == Look::Ghost { label.group_color(t.text) } else { label });
    }
    b
}

fn foot(s: &State, t: &Theme, m: &PrPopover, session: &str) -> El {
    let mut f = row().key("session-pr-foot").w_full().center_y().gap(6.0).px(14.0).py(10.0).fill(t.wash(3.0));
    match &m.primary {
        Primary::Archive => {
            let id = session.to_string();
            f = f.child(
                button(t, "session-pr-archive", Look::Plain, icon(&ARCHIVE, 13.0).color(t.text), Some("Archive session")).on_press_tx(
                    move |s: &mut State, tx: &mut Tx| {
                        s.ui.open = None;
                        let on = s.store.machine_of(&id).map(str::to_string);
                        s.archive_sessions(std::slice::from_ref(&id), on.as_deref(), tx);
                    },
                ),
            );
        }
        Primary::Fix(label) => {
            let sent = s.agent_sends.is_sent(session, FIX);
            let sending = s.agent_sends.is_sending(session, FIX);
            let glyph = if sent {
                icon(&CHECK, 13.0).color(t.primary_fg)
            } else if sending {
                icon(&LOADER_CIRCLE, 13.0).color(t.primary_fg).cycle(crate::view::ring_popover::spinner())
            } else {
                icon(&SPARKLES, 13.0).color(t.primary_fg)
            };
            let mut b = button(t, "session-pr-fix", Look::Primary, glyph, Some(if sent { "Sent to the agent" } else { label }));
            if sent || sending {
                b = b.disabled(true).opacity(0.7);
            } else {
                let prompt = crate::fleet::smart_links::format_fix_prompt(&m.link, &m.failing, &m.threads);
                b = b.on_press(move |s: &mut State| s.send_to_agent(FIX, prompt.clone()));
            }
            f = f.child(b);
        }
        Primary::MergeOnGithub => {
            f = f.child(
                button(t, "session-pr-merge", Look::Primary, icon(&EXTERNAL_LINK, 13.0).color(t.primary_fg), Some("Merge on GitHub"))
                    .on_press_tx(open_in_browser(m.href.clone())),
            );
        }
        Primary::None => {}
    }
    let link_id = m.link.id.clone();
    f.child(el().grow(1.0))
        .child(button(t, "session-pr-show-context", Look::Ghost, el(), Some("Show in Context")).on_press(move |s: &mut State| {
            s.ui.open = None;
            s.show_in_context(&link_id);
        }))
        .child(
            button(t, "session-pr-open", Look::Ghost, icon(&EXTERNAL_LINK, 13.0).color(t.muted).group_color(t.text), None)
                .help("Open on GitHub".to_string())
                .label("Open on GitHub".to_string())
                .on_press_tx(open_in_browser(m.href.clone())),
        )
}
