//! Where a page canvas's page is (component 14, piece A): `server-canvas.test.ts`'s address case under its name, and the
//! address fields `shownPage` reads.
use super::super::pages::{ChoiceStatus, PageChoice, read_page_choice};
use super::super::web_page::*;
use serde_json::json;

fn page(page_id: &str, entry: &str) -> ShownPage {
    ShownPage { page_id: page_id.into(), entry: entry.into(), ..ShownPage::default() }
}

// server-canvas.test.ts: "addresses the page on Fleet, escaping each part of the file name"
#[test]
fn addresses_the_page_on_fleet_escaping_each_part_of_the_file_name() {
    assert_eq!(page_address(&page("pg_1", "options a.html")), "/pages/pg_1/options%20a.html");
    assert_eq!(page_address(&page("pg_1", "sub/b#c.html")), "/pages/pg_1/sub/b%23c.html");
}

#[test]
fn reads_the_fields_the_address_needs_and_leaves_the_rest_empty() {
    let read = shown_page(&json!({"pageId": "pg_2", "entry": "index.html", "source": "/a/pages/index.html", "shownAt": "t1"}));
    assert_eq!(
        read,
        ShownPage {
            page_id: "pg_2".into(),
            entry: "index.html".into(),
            source: "/a/pages/index.html".into(),
            shown_at: "t1".into(),
            ..Default::default()
        }
    );
    assert_eq!(shown_page(&json!({"pageId": 7, "entry": null})), ShownPage::default());
    assert_eq!(shown_page(&json!("not a record")), ShownPage::default());
}
// server-canvas.test.ts: "reads a page canvas's state and fills what's missing"
#[test]
fn reads_a_page_canvas_state_and_fills_whats_missing() {
    let read = shown_page(
        &json!({"pageId": "pg_1", "entry": "a.html", "source": "/tmp/a.html", "files": 2, "bytes": 10, "shownAt": "t", "warnings": ["w", 3], "label": ""}),
    );
    assert_eq!((read.files, read.bytes, read.warnings.clone(), read.label.clone()), (2, 10, vec!["w".to_string()], None));
    assert_eq!(shown_page(&json!(null)), ShownPage::default());
    assert_eq!(shown_page(&json!({"label": "Walkthrough"})).label.as_deref(), Some("Walkthrough"));
}

// server-canvas.test.ts: "reads the choice a page waits on, and drops a malformed one"
#[test]
fn reads_the_choice_a_page_waits_on_and_drops_a_malformed_one() {
    let choice = json!({"callId": "call_1", "status": "answered", "value": "a", "summary": "Layout A", "at": "2026-10-16T12:41:00Z"});
    let read = shown_page(&json!({"choice": choice})).choice.unwrap();
    assert_eq!((read.call_id.as_str(), read.status), ("call_1", ChoiceStatus::Answered));
    assert_eq!((read.value.as_deref(), read.summary.as_deref()), (Some("a"), Some("Layout A")));
    assert_eq!((read.reason, read.at.as_str()), (None, "2026-10-16T12:41:00Z"));
    let mut maybe = choice.clone();
    maybe["status"] = json!("maybe");
    assert_eq!(shown_page(&json!({"choice": maybe})).choice, None);
    assert_eq!(shown_page(&json!({"choice": {"status": "waiting"}})).choice, None);
}

#[test]
fn names_the_file_from_either_kind_of_slash_else_the_entry() {
    let mut p = page("pg", "index.html");
    p.source = "C:\\work\\notes\\layouts.html".into();
    assert_eq!(p.file_name(), "layouts.html");
    p.source = "/a/b/c.html".into();
    assert_eq!(p.file_name(), "c.html");
    p.source = String::new();
    assert_eq!(p.file_name(), "index.html");
}

fn waiting() -> PageChoice {
    read_page_choice(&json!({"callId": "c1", "status": "waiting", "at": "2026-10-16T12:00:00Z"})).unwrap()
}

fn pick(value: &str, summary: &str) -> (String, String) {
    (value.to_string(), summary.to_string())
}

fn with(mut c: PageChoice, status: ChoiceStatus) -> PageChoice {
    c.status = status;
    c
}

// page-choice.test.ts, "pageChoiceBar": "waits, then shows the proposal"
#[test]
fn bar_waits_then_shows_the_proposal() {
    let bar = page_choice_bar(&waiting(), true, None, "");
    assert_eq!((bar.kind, bar.text.as_str()), (BarKind::Waiting, "The agent is waiting for your pick."));
    let bar = page_choice_bar(&waiting(), true, Some(&pick("a", "Layout A")), "");
    assert_eq!((bar.kind, bar.text.as_str(), bar.strong.as_deref()), (BarKind::Proposed, "You picked ", Some("Layout A")));
}

// "says why the page's pick was refused, and keeps Send off"
#[test]
fn bar_says_why_the_pages_pick_was_refused() {
    let reason = "The page's pick was too long to send: 18,345 characters, and the most is 16,000.";
    let bar = page_choice_bar(&waiting(), true, None, reason);
    assert_eq!((bar.kind, bar.text.as_str(), bar.strong), (BarKind::Refused, reason, None));
    // A pick the page sends afterwards wins; so does the end of the wait.
    let bar = page_choice_bar(&waiting(), true, Some(&pick("a", "Layout A")), reason);
    assert_eq!((bar.kind, bar.strong.as_deref()), (BarKind::Proposed, Some("Layout A")));
    assert_eq!(page_choice_bar(&waiting(), false, None, reason).kind, BarKind::NotAnswered);
    let mut answered = with(waiting(), ChoiceStatus::Answered);
    answered.value = Some("b".into());
    assert_eq!(page_choice_bar(&answered, false, None, reason).kind, BarKind::Answered);
}

// "shows an answer"
#[test]
fn bar_shows_an_answer() {
    let mut c = with(waiting(), ChoiceStatus::Answered);
    c.summary = Some("Layout B".into());
    c.value = Some("b".into());
    let bar = page_choice_bar(&c, false, None, "");
    assert_eq!((bar.kind, bar.strong.as_deref(), bar.at.as_deref()), (BarKind::Answered, Some("Layout B"), Some("2026-10-16T12:00:00Z")));
    c.summary = None;
    assert_eq!(page_choice_bar(&c, false, None, "").strong.as_deref(), Some("b"), "the value when there is no summary");
}

// "adds no full stop after a summary shortened with an ellipsis"
#[test]
fn bar_adds_no_full_stop_after_a_summary_shortened_with_an_ellipsis() {
    let short = pick("{}", "Framework: SvelteKit; Licence: Ap\u{2026}");
    assert_eq!(page_choice_bar(&waiting(), true, Some(&pick("a", "Layout A")), "").after, ".");
    let bar = page_choice_bar(&waiting(), true, Some(&short), "");
    assert_eq!((bar.strong.as_deref(), bar.after.as_str()), (Some(short.1.as_str()), ""));
    let mut answered = with(waiting(), ChoiceStatus::Answered);
    answered.summary = Some(short.1.clone());
    assert_eq!(page_choice_bar(&answered, false, None, "").after, "");
}

// "says why it wasn't answered"
#[test]
fn bar_says_why_it_wasnt_answered() {
    let text = |reason: Option<&str>| {
        let mut c = with(waiting(), ChoiceStatus::NotAnswered);
        c.reason = reason.map(str::to_string);
        page_choice_bar(&c, false, None, "").text
    };
    assert_eq!(text(Some("wrote instead")), "Not answered. You wrote instead.");
    assert_eq!(text(Some("skipped")), "Not answered. You skipped it.");
    assert_eq!(text(Some("stopped")), "Not answered. The turn stopped.");
    assert_eq!(text(Some("Fleet restarted")), "Not answered. Fleet restarted.");
    assert_eq!(text(Some("timed out")), "Not answered after 24 hours.");
    assert_eq!(text(None), "Not answered.");
}

// "reads a waiting choice on an idle session as not answered"
#[test]
fn bar_reads_a_waiting_choice_on_an_idle_session_as_not_answered() {
    let bar = page_choice_bar(&waiting(), false, Some(&pick("a", "Layout A")), "");
    assert_eq!((bar.kind, bar.text.as_str()), (BarKind::NotAnswered, "Not answered."));
}

// "pageChoiceRowTitle": "leaves other calls alone" and "follows the call"
#[test]
fn row_title_leaves_other_calls_alone() {
    assert_eq!(page_choice_row_title(&json!({"title": "x"}), Some("running"), &json!(null)), None);
    assert_eq!(page_choice_row_title(&json!(null), Some("running"), &json!(null)), None);
}

#[test]
fn row_title_follows_the_call() {
    let input = json!({"waitForChoice": true, "title": "Layouts for pocket-notes"});
    let title = |status, meta| page_choice_row_title(&input, Some(status), &meta).unwrap();
    assert_eq!(title("running", json!(null)), "Waiting for your pick \u{b7} Layouts for pocket-notes");
    assert_eq!(title("completed", json!({"choice": {"answered": true, "value": "a", "summary": "Layout A"}})), "Picked Layout A");
    assert_eq!(title("completed", json!({"choice": {"answered": false, "reason": "wrote instead"}})), "Not picked \u{b7} wrote instead");
    assert_eq!(title("completed", json!({"choice": {"answered": false, "reason": "skipped"}})), "Not picked \u{b7} you skipped it");
    assert_eq!(title("error", json!(null)), "Not answered");
}

#[test]
fn puts_the_page_on_the_machine_it_was_shown_on_and_names_none_without_an_id() {
    assert_eq!(page_url("http://127.0.0.1:6411/", &page("pg_1", "a.html")).as_deref(), Some("http://127.0.0.1:6411/pages/pg_1/a.html"));
    assert_eq!(page_url("http://127.0.0.1:6411", &page("", "a.html")), None);
    assert_eq!(page_url("http://127.0.0.1:6411", &page("pg_1", "")), None);
}
