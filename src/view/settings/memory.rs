//! Settings → Memory (`MemorySection.vue`), the part with no lists: the card, the switch, the four facts shown while it is
//! off, and the Harnesses card. The notes, Dreaming and Clear all are later pieces of the same page. It is a machine page:
//! it shows what the machine Settings shows keeps, and says so in the header's chip.
use super::kit::{self, SwitchState};
use crate::State;
use crate::fleet::settings::memory::MemoryPage;
use crate::icons::{CABLE, COINS, EYE, FILE_TEXT, FOLDER_GIT_2, TRIANGLE_ALERT};
use crate::theme::{RADIUS_CARD, Theme, sans};
use crate::ui::{confirm::spinner, rem};
use rune::{El, Icon, Span, col, icon, rich, row, text};

const ON: &str = "On. New sessions start with their repository's notes and this machine's notes.";
const OFF: &str = "Off. Agents save nothing and sessions carry nothing extra.";
const BLURB: &str = "Let agents keep notes about each repository (test commands, conventions, your corrections) and about this machine \
                     (disk space, memory, crashes, tools that don't work here). New sessions start with both, so agents get better the \
                     longer you work with them.";
const CLAUDE_WARNING: &str = "Claude Code also keeps its own memory in ~/.claude. The two can hold the same lesson twice or disagree. For one \
                              memory, turn Claude Code's own memory off in its settings.";

/// `text-xs text-muted`.
fn small(t: &Theme, words: &str) -> El {
    text(words.to_string()).font(sans(rem(0.75), 400.0, 1.33)).color(t.muted).wrap().shrink().min_w(0.0)
}

/// `p-4 rounded-card border border-border bg-main-bg`: the inner boxes of the card.
fn inner(t: &Theme, key: &str) -> El {
    boxed(col(), t, key)
}

fn boxed(base: El, t: &Theme, key: &str) -> El {
    base.key(key.to_string()).w_full().mt(17.5).px(14.0).py(14.0).radius(RADIUS_CARD).border(1.0, t.border).border_layout().fill(t.main_bg)
}

/// The title and the line under it.
fn heading(t: &Theme) -> El {
    col().w_full().gap(3.5).child(text("Memory").font(sans(rem(0.875), 600.0, 1.43)).color(t.text)).child(small(t, BLURB))
}

fn loading(t: &Theme) -> El {
    row()
        .key("memory-loading")
        .mt(17.5)
        .gap(7.0)
        .center_y()
        .child(spinner(t.muted))
        .child(text("Loading memory…").font(sans(rem(0.875), 400.0, 1.43)).color(t.muted))
}

/// "Remember what agents learn" and its state line, with the switch.
fn switch_row(page: &MemoryPage, t: &Theme) -> El {
    let on = page.enabled();
    let words = col()
        .shrink()
        .min_w(0.0)
        .child(text("Remember what agents learn").font(sans(rem(0.875), 500.0, 1.43)).color(t.text))
        .child(row().key("memory-state").mt(3.5).child(small(t, if on { ON } else { OFF })));
    let switch = kit::setting_switch(
        t,
        "memory-switch",
        "Remember what agents learn",
        on,
        SwitchState { saving: page.saving, disabled: page.saving, pending: false },
        |s: &mut State| s.toggle_memory(),
    );
    boxed(row(), t, "memory-onoff").justify_between().items_start().gap(14.0).child(words).child(switch)
}

/// One of the four facts: an accent icon two pixels down, a bold lead, then the rest (`.memory-fact`: 12px, 1.5).
fn fact(t: &Theme, key: &str, glyph: &'static Icon, lead: &str, rest: &str) -> El {
    let body = format!("{lead} {rest}");
    let spans = vec![Span::new(0, lead.len()).color(t.text).weight(600.0)];
    row()
        .key(key.to_string())
        .w(0.0)
        .grow(1.0)
        .min_w(0.0)
        .gap(10.0)
        .items_start()
        .child(icon(glyph, 15.0).color(t.accent).mt(2.0))
        .child(rich(body, spans).font(sans(12.0, 400.0, 1.5)).color(t.muted).wrap().shrink().min_w(0.0))
}

/// What turning it on costs and who reads the notes: two columns, from the `sm` width, which the window never goes under.
fn facts(page: &MemoryPage, t: &Theme) -> El {
    let (repository, machine) = page.overview.as_ref().map_or((0, 0), |o| (o.max_repository_notes, o.max_machine_notes));
    let costs = format!(
        "Every request a session sends carries its notes: about 500 tokens of rules plus 30–50 per note, up to {repository} repository and {machine} machine notes."
    );
    col()
        .key("memory-facts")
        .w_full()
        .mt(14.0)
        .gap(10.5)
        .child(
            row()
                .w_full()
                .gap(10.5)
                .child(fact(t, "memory-fact-costs", &COINS, "Costs tokens.", &costs))
                .child(fact(
                    t,
                    "memory-fact-visible",
                    &EYE,
                    "Saved automatically, never quietly.",
                    "Each note shows in a small notice with Undo, and stays in the conversation. Edit or forget any note here.",
                )),
        )
        .child(
            row()
                .w_full()
                .gap(10.5)
                .child(fact(t, "memory-fact-files", &FILE_TEXT, "Plain Markdown files", "in Fleet's data folder, one per note, never in your repository."))
                .child(fact(
                    t,
                    "memory-fact-lists",
                    &FOLDER_GIT_2,
                    "Two lists.",
                    "Repository notes reach only that repository's sessions, worktrees included. Machine notes reach every session on this machine.",
                )),
        )
}

/// `li`: the harness and what it does with notes, `py-2`, a rule above all but the first.
fn harness_row(t: &Theme, first: bool, name: &str, what: &str, below: Option<El>) -> El {
    let line = row().w_full().justify_between().gap(10.5).child(small(t, name)).child(small(t, what));
    let mut r = col().w_full().py(7.0).child(line);
    if !first {
        r = r.border_sides([1.0, 0.0, 0.0, 0.0], t.border);
    }
    r.children(below)
}

/// Which harnesses read and save notes. Static words (`MemorySection.vue`, the last card).
fn harnesses(t: &Theme) -> El {
    let warning = row()
        .key("memory-claude-warning")
        .mt(3.5)
        .gap(7.0)
        .items_start()
        .child(icon(&TRIANGLE_ALERT, 13.0).color(t.status_waiting()).mt(1.75))
        .child(small(t, CLAUDE_WARNING));
    inner(t, "memory-harnesses")
        .child(
            row()
                .gap(7.0)
                .center_y()
                .child(icon(&CABLE, 14.0).color(t.text))
                .child(text("Harnesses").font(sans(rem(0.75), 500.0, 1.33)).color(t.text)),
        )
        .child(
            col()
                .w_full()
                .mt(7.0)
                .child(harness_row(t, true, "OpenCode and OpenCode 2", "Read and save notes", None))
                .child(harness_row(t, false, "Claude Code", "Reads and saves notes", Some(warning)))
                .child(harness_row(t, false, "Pi", "Not yet", None)),
        )
}

/// The card (`MemorySection`): the heading, then loading, or the switch with what follows it, or why nothing loaded.
pub fn card(s: &State, t: &Theme) -> El {
    let page = &s.settings.memory;
    let mut card = col()
        .key("memory-card")
        .w_full()
        .px(14.0)
        .py(14.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.card_bg)
        .child(heading(t));
    let failure = |message: &str| col().w_full().mt(3.5).child(kit::error_note(t, "memory-error", message));
    if page.loading {
        card = card.child(loading(t));
    } else if page.overview.is_some() {
        card = card.child(switch_row(page, t));
        if let Some(message) = &page.error {
            card = card.child(failure(message));
        }
        if !page.enabled() {
            card = card.child(facts(page, t));
        } else if page.notes.is_some() {
            // Dreaming's nightly row and panel go between the toolbar and the lists (pieces R and S).
            card = card.child(super::memory_notes::toolbar(s, t));
            card = card.children(super::memory_notes::lists(s, t));
            card = card.children(super::memory_expired::expired(s, t));
            card = card.child(super::memory_clear::clear_all(s, t));
        }
        card = card.child(harnesses(t));
    } else if let Some(message) = &page.error {
        card = card.child(failure(message));
    }
    card
}

pub fn content(s: &State, t: &Theme) -> El {
    col()
        .key("memory-section")
        .w_full()
        .gap(21.0)
        .child(super::super::settings::page_header(s, t, &crate::fleet::settings_nav::Page::Memory))
        .child(card(s, t))
}
