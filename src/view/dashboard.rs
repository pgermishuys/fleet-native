//! The home page (`FleetDashboard.vue`, `PageHeader.vue`, `EmptyState.vue`, `ErrorNote.vue`, `routes/index.tsx`'s centre): the
//! greeting and what needs the user, the harness banner, the loading, error and empty states, and the three sections of
//! sessions, "Show N more" and the retention filter's menu. On another machine the same page draws that machine's sessions
//! (read by `fleet::dashboard_machine`) with no row actions.
use super::dashboard_row::{At, dashboard_row};
use super::session_row::RowClock;
use crate::State;
use crate::fleet::dashboard::{self, Bucket};
use crate::fleet::store::Retention;
use crate::icons::{CHEVRON_DOWN, DOWNLOAD, LOADER_CIRCLE, MESSAGE_SQUARE, PLUS};
use crate::mock::Session;
use crate::theme::{RADIUS_BTN, RADIUS_CARD, RADIUS_PANEL, Theme, sans};
use crate::ui::button::{Size, Variant, button};
use crate::ui::rem;
use rune::{Align, Cursor, Cycle, El, ScrollSpec, col, el, icon, row, text};

/// How many quiet sessions show before "Show more" (`RECENT_LIMIT`).
pub const RECENT_LIMIT: usize = 8;
/// The page's column (`max-width: 1040px`).
const COLUMN: f32 = 1040.0;

/// Where the dashboard's sessions are: each one's place in `State::projects`, the way the rows open them.
#[derive(Clone, Copy)]
struct Placed<'a> {
    at: At,
    session: &'a Session,
}

/// The sessions the dashboard lists, in each section, newest first (`shown`, `sections`).
fn placed(s: &State) -> [Vec<Placed<'_>>; 3] {
    // Another machine's page lists what it read itself; the live machine's is the session list's.
    let all: Vec<Placed> = if s.dashboard_on_other().is_some() {
        s.dash.other.sessions.iter().map(|session| Placed { at: At::There, session }).collect()
    } else {
        s.projects
            .iter()
            .enumerate()
            .flat_map(|(p, project)| project.sessions.iter().enumerate().map(move |(i, session)| Placed { at: At::Here(p, i), session }))
            .collect()
    };
    let shown = dashboard::shown(all.iter().map(|x| x.session), s.store.retention, &|id| s.archive.hides(id));
    // `workflowWaiting`: the sessions whose run waits on the user in them, on the machine the page shows.
    let waiting = s.workflow_runs(&s.dashboard_machine_key()).waiting_session_ids();
    let sections = dashboard::sections(&shown, &|id| waiting.contains(id));
    sections.map(|list| list.iter().filter_map(|x| all.iter().find(|a| std::ptr::eq(a.session, *x)).copied()).collect())
}

fn spinner(t: &Theme) -> El {
    static FADE: [(f32, f32); 3] = [(0.0, 1.0), (0.5, 0.35), (1.0, 1.0)];
    icon(&LOADER_CIRCLE, 14.0).color(t.muted).cycle(Cycle { period: 1.2, phase: 0.0, stops: &FADE, still: 1.0, fps: 30.0 })
}

/// `PageHeader`: the greeting (`text-2xl`, 21 px) and the line under it, with New session on the right.
fn header(s: &State, t: &Theme, needs: usize, working: usize) -> El {
    let hour = dashboard::hour_of(s.now_ms.unwrap_or_else(crate::fleet::link::now_ms), crate::conv::time::local_offset_secs());
    let lead = col()
        .grow(1.0)
        .min_w(0.0)
        .child(text(dashboard::greeting(hour)).key("home-greeting").font(sans(21.0, 600.0, 1.25).tracking(-0.525)).color(t.text))
        .child(text(dashboard::status_line(needs, working)).key("home-status").font(sans(rem(0.875), 400.0, 1.5)).color(t.muted).mt(4.0));
    row().w_full().center_y().gap(16.0).child(lead).child(new_session_button(t))
}

fn new_session_button(t: &Theme) -> El {
    button(t, Variant::Default, Size::Sm, Some(&PLUS), "New session")
        .key("home-new-session")
        .on_press(|s: &mut State| s.open_new_session_page(None))
}

/// The banner when no harness can start a session: Fleet's reason under its words (Fleet #1049).
fn harness_banner(t: &Theme, reason: &str) -> El {
    let tile = el()
        .size(34.0, 34.0)
        .center()
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.card_bg)
        .child(icon(&DOWNLOAD, 17.0).color(t.text));
    let words = col()
        .grow(1.0)
        .min_w(0.0)
        .child(text("No harness is ready yet").font(sans(rem(1.0), 600.0, 1.5)).color(t.text))
        .child(
            text("Fleet runs sessions through a harness such as OpenCode or Claude Code. Install one to start.")
                .font(sans(13.0, 400.0, 1.5))
                .color(t.muted)
                .wrap()
                .w_full(),
        )
        .child(text(reason.to_string()).key("home-harness-reason").font(sans(13.0, 400.0, 1.5)).color(t.muted).wrap().w_full().mt(2.0));
    row()
        .key("home-harness-banner")
        .w_full()
        .center_y()
        .gap(14.0)
        .px(16.0)
        .py(14.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.accent.alpha(0.35))
        .border_layout()
        .fill(t.accent_dim)
        .child(tile)
        .child(words)
        .child(
            button(t, Variant::Default, Size::Sm, None, "Set up a harness")
                .key("home-harness-setup")
                .on_press(|s: &mut State| s.open_harness_setup()),
        )
}

fn error_note(t: &Theme, message: &str, reason: &str) -> El {
    crate::ui::error_note::error_note(t, "home-load-error", message, reason)
}

/// `EmptyState bordered`: nothing to list. (Fleet's border is dashed; Rune has no dashed line, so this one is solid.)
fn empty(t: &Theme, archived: bool) -> El {
    col()
        .key("home-empty")
        .w_full()
        .items_center()
        .gap(4.0)
        .px(24.0)
        .py(56.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .child(icon(&MESSAGE_SQUARE, 28.0).color(t.muted).mb(8.0))
        .child(text(if archived { "No archived sessions" } else { "No sessions yet" }).font(sans(rem(1.0), 600.0, 1.5)).color(t.text))
        .child(
            text("Start a session to put an agent to work. It shows up here while it runs, and when it needs you.")
                .font(sans(rem(0.875), 400.0, 1.43))
                .color(t.muted)
                .align(Align::Center)
                .wrap()
                .max_w(560.0),
        )
        .child(
            row().mt(12.0).gap(8.0).center_y().child(new_session_button(t)).child(filter_label(t, retention_word(archived_as(archived)))),
        )
}

fn archived_as(archived: bool) -> Retention {
    if archived { Retention::Archived } else { Retention::Active }
}

fn retention_word(r: Retention) -> &'static str {
    match r {
        Retention::Active => "Active",
        Retention::Archived => "Archived",
        Retention::All => "All",
    }
}

/// `RetentionFilter`'s button: "Show: Active" and a chevron; it opens the menu of the three (`menus::retention`).
fn filter_label(t: &Theme, word: &str) -> El {
    row()
        .key("home-filter")
        .role("button")
        .label(format!("Show: {word}"))
        .gap(rem(0.375))
        .center_y()
        .px(rem(0.625))
        .h(rem(1.75))
        .radius(RADIUS_BTN)
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 1.0, t.accent)
        .hover(t.wash(7.0))
        .hover_group()
        .on_press(|s: &mut State| s.ui.open = Some(crate::ui::Open::Menu("retention")))
        .child(text(format!("Show: {word}")).font(sans(12.5, 400.0, 1.3)).color(t.muted).group_color(t.text))
        .child(icon(&CHEVRON_DOWN, rem(0.875)).color(t.muted).group_color(t.text))
}

/// The word of a retention in the filter and its menu.
pub fn retention_label(r: Retention) -> &'static str {
    retention_word(r)
}

/// A section's title: the name and its count (`dashboard__section-title`).
fn title(key: &str, name: &str, count: usize, ink: rune::Color, pad: f32) -> El {
    row()
        .key(key.to_string())
        .gap(6.0)
        .center_y()
        .px(pad)
        .child(text(name.to_string()).font(sans(12.0, 600.0, 1.3)).color(ink))
        .child(text(count.to_string()).font(sans(12.0, 500.0, 1.3).features("'tnum' 1")).color(ink.alpha(0.8)))
}

fn rows(s: &State, t: &Theme, clock: &RowClock, list: &[Placed]) -> El {
    col().w_full().children(list.iter().map(|x| dashboard_row(s, t, clock, x.at, x.session)))
}

fn section(s: &State, t: &Theme, clock: &RowClock, which: Bucket, list: &[Placed]) -> El {
    let (key, name) = match which {
        Bucket::NeedsYou => ("home-needs-you", "Needs you"),
        Bucket::Working => ("home-working", "Working"),
        Bucket::Recent => ("home-recent", "Recent"),
    };
    if which == Bucket::NeedsYou {
        // What needs the user sits on a faint wash of the waiting colour.
        return col()
            .key(key)
            .w_full()
            .gap(6.0)
            .pt(10.0)
            .pb(4.0)
            .radius(RADIUS_CARD)
            .border(1.0, t.status_waiting().alpha(0.22))
            .border_layout()
            .fill(t.status_waiting().alpha(0.05))
            .child(title("home-needs-you-title", name, list.len(), t.status_waiting_text, 14.0))
            .child(rows(s, t, clock, list).px(4.0));
    }
    let mut head =
        row().w_full().center_y().justify_between().gap(12.0).child(title(&format!("{key}-title"), name, list.len(), t.muted, 10.0));
    if which == Bucket::Recent {
        head = head.child(filter_label(t, retention_word(s.store.retention)));
    }
    let mut out = col().key(key).w_full().gap(6.0).child(head);
    if list.is_empty() {
        // `EmptyState compact`: "Nothing else here".
        out = out.child(
            text("Nothing else here")
                .key("home-recent-empty")
                .font(sans(rem(0.875), 500.0, 1.43))
                .color(t.text)
                .align(Align::Center)
                .w_full()
                .py(16.0),
        );
    } else {
        let all = which == Bucket::Recent && s.dash.show_all_recent;
        let shown = if which == Bucket::Recent && !all { &list[..list.len().min(RECENT_LIMIT)] } else { list };
        out = out.child(rows(s, t, clock, shown));
    }
    if which == Bucket::Recent && list.len() > RECENT_LIMIT {
        let word = if s.dash.show_all_recent { "Show fewer".to_string() } else { format!("Show {} more", list.len() - RECENT_LIMIT) };
        out = out.child(
            row()
                .key("home-more")
                .role("button")
                .label(word.clone())
                .px(6.0)
                .py(4.0)
                .ml(4.0)
                .radius(RADIUS_BTN)
                .cursor(Cursor::Pointer)
                .focusable(true)
                .focus_ring(2.0, 1.0, t.accent)
                .hover_group()
                .on_press(|s: &mut State| s.toggle_recent())
                .child(text(word).font(sans(12.5, 400.0, 1.3)).color(t.muted).group_color(t.text)),
        );
    }
    out
}

/// The page: the sheet's place on Home. A panel with its own scroll and the column centred in it.
pub fn page(s: &State, t: &Theme) -> El {
    let clock = RowClock::now(s);
    let [needs, working, recent] = placed(s);
    let mut body = col().key("home").w_full().max_w(COLUMN).gap(24.0).child(header(s, t, needs.len(), working.len()));
    // A banner on this machine only, and only once the harness list says nothing is ready.
    if s.machines.current_key() == crate::fleet::settings::HOME_MACHINE_KEY
        && let Some(reason) = s.setup_harnesses().no_harness_reason()
    {
        body = body.child(harness_banner(t, &reason));
    }
    body = body.child(super::summary_bar::bar(s, t));
    let nothing = needs.is_empty() && working.is_empty() && recent.is_empty();
    // Another machine's list is asked for once the page is shown: until then it is loading, not empty.
    let other = s.dashboard_on_other().is_some();
    let loading = nothing && if other { s.dash.other.key.is_none() || s.dash.other.loading } else { s.store.is_loading };
    let error = if other { s.dash.other.error.as_ref() } else { s.store.error.as_ref() };
    if let Some(reason) = error {
        body = body.child(error_note(t, "Couldn't load sessions.", reason));
    }
    if loading {
        body = body.child(
            row()
                .key("home-loading")
                .w_full()
                .center()
                .gap(8.0)
                .py(48.0)
                .child(spinner(t))
                .child(text("Loading sessions…").font(sans(13.0, 400.0, 1.3)).color(t.muted)),
        );
    } else if nothing && error.is_none() {
        body = body.child(empty(t, s.store.retention == Retention::Archived));
    } else if !(nothing && error.is_some()) {
        if !needs.is_empty() {
            body = body.child(section(s, t, &clock, Bucket::NeedsYou, &needs));
        }
        if !working.is_empty() {
            body = body.child(section(s, t, &clock, Bucket::Working, &working));
        }
        body = body.child(section(s, t, &clock, Bucket::Recent, &recent));
    }
    col()
        .key("home-page")
        .grow(1.0)
        .h_full()
        .min_w(0.0)
        .fill(t.panel_bg)
        .border(1.0, t.border)
        .border_layout()
        .radius(RADIUS_PANEL)
        .clip_rounded(RADIUS_PANEL)
        .child(
            col()
                .w_full()
                .grow(1.0)
                .min_h(0.0)
                .padding(24.0)
                .items_center()
                .scroll(ScrollSpec { stick_bottom: false })
                .key("home-scroll")
                .child(body),
        )
}

/// What the page's rows are sorted into, for a test that reads the list (the sessions' ids, newest first, per section).
pub fn listed(s: &State) -> [Vec<String>; 3] {
    placed(s).map(|list| list.iter().map(|x| x.session.id.clone()).collect())
}
