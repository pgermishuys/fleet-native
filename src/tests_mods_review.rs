//! Component 22, piece I: the Keep dialog. `lib/mods/__tests__/check-summary.test.ts` (12) and
//! `components/mods/review/__tests__/ModReviewDialog.test.ts` (8) under Fleet's names. The summary's cases run on the report's
//! JSON; the dialog's against a fake Fleet that answers the check, the keep and the discard.
use crate::fleet::mods::check_summary::{CheckRow, check_problems, summarize_check};
use crate::fleet::mods::drafts::Checked;
use crate::tests::FRAME;
use crate::tests_mods_drafts::{Rig, card, rig};
use rune::{Key, Mods};
use serde_json::{Value, json};

fn report(extra: Value) -> Value {
    let mut r = json!({
        "ok": true, "name": "test-chips", "version": "1", "description": "", "lines": 47, "sha256": "abc",
        "hooks": [], "calls": [], "state": [], "pages": [], "errors": [], "warnings": []
    });
    for (k, v) in extra.as_object().into_iter().flatten() {
        r[k] = v.clone();
    }
    r
}
fn render(matcher: Value) -> Value {
    json!({ "event": "ui.render", "matcher": matcher })
}
fn render_any() -> Value {
    json!({ "event": "ui.render" })
}
fn with_hooks(hooks: Value) -> Value {
    report(json!({ "hooks": hooks }))
}
fn keys(r: &Value) -> Vec<String> {
    summarize_check(r).into_iter().map(|row| row.key).collect()
}
fn text(r: &Value, key: &str) -> Option<String> {
    summarize_check(r).into_iter().find(|row| row.key == key).map(|row| row.text)
}
fn strs(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

// ---- check-summary.test.ts ----

// "describes the worked example: rows for bash and shell calls, then the muted rows"
#[test]
fn describes_the_worked_example_rows_for_bash_and_shell_calls_then_the_muted_rows() {
    let r = report(json!({
        "hooks": [render(json!({ "component": ["ToolUse", "ToolResult"], "props": { "tool": ["bash", "shell"] } }))],
        "calls": ["ui.resolve"]
    }));
    assert_eq!(keys(&r), strs(&["draws", "reads", "changes", "network", "files"]));
    assert_eq!(text(&r, "draws").unwrap(), "Rows for bash and shell calls (on the line, and the opened body)");
    assert_eq!(text(&r, "reads").unwrap(), "Tool input and output of those calls");
    let muted: Vec<String> = summarize_check(&r).into_iter().filter(|row| row.muted).map(|row| row.text).collect();
    assert_eq!(muted, strs(&["Nothing the agent sees", "None", "None"]));
}

// "names every site, one row each, and every place when there is no matcher"
#[test]
fn names_every_site_one_row_each_and_every_place_when_there_is_no_matcher() {
    let sites = summarize_check(&with_hooks(json!([render(json!({ "component": ["ComposerBand", "StatusChip", "Pane"] }))])));
    let draws: Vec<String> = sites.iter().filter(|r| r.key.starts_with("draws")).map(|r| r.text.clone()).collect();
    assert_eq!(draws, strs(&["A band above the composer", "A chip in the status bar", "A pane it opens"]));
    assert_eq!(text(&with_hooks(json!([render_any()])), "draws").unwrap(), "Every place a mod can draw");
}

// "narrows by tool with a regex, and falls back to code for a shape it doesn't know"
#[test]
fn narrows_by_tool_with_a_regex_and_falls_back_to_code_for_a_shape_it_doesn_t_know() {
    let regex = with_hooks(json!([render(json!({ "component": "ToolUse", "props": { "tool": { "$regex": "^git", "flags": "i" } } }))]));
    assert!(text(&regex, "draws").unwrap().contains("matching /^git/i"));
    let unknown = summarize_check(&with_hooks(json!([render(json!({ "component": "Mystery" }))])));
    let draws: &CheckRow = unknown.iter().find(|r| r.key == "draws").unwrap();
    assert_eq!(draws.code.as_deref(), Some(r#"{"component":"Mystery"}"#));
    assert!(!draws.text.contains('`'));
}

// "lists what it listens to in one row, and omits it when nothing"
#[test]
fn lists_what_it_listens_to_in_one_row_and_omits_it_when_nothing() {
    assert!(!keys(&report(json!({}))).contains(&"listens".to_string()));
    let r =
        with_hooks(json!([{ "event": "ui.press" }, { "event": "ui.input" }, { "event": "session.start" }, { "event": "turn.complete" }]));
    assert_eq!(text(&r, "listens").unwrap(), "Buttons and fields it draws, when a session opens, when a turn ends");
}

// "says it reads every tool call unless the render hook is narrowed by tool or to sites without tool rows"
#[test]
fn says_it_reads_every_tool_call_unless_the_render_hook_is_narrowed_by_tool_or_to_sites_without_tool_rows() {
    let every = "Tool input and output of every tool call";
    assert_eq!(text(&with_hooks(json!([render_any()])), "reads").unwrap(), every);
    assert_eq!(text(&with_hooks(json!([render(json!({ "props": { "status": "error" } }))])), "reads").unwrap(), every);
    assert_eq!(text(&with_hooks(json!([render(json!({ "component": "ToolUse" }))])), "reads").unwrap(), every);
    assert_eq!(
        text(&with_hooks(json!([render(json!({ "props": { "tool": "bash" } }))])), "reads").unwrap(),
        "Tool input and output of those calls"
    );
    assert_eq!(text(&with_hooks(json!([render(json!({ "component": ["ComposerBand", "StatusChip"] }))])), "reads"), None);
}

// "reads session details when it calls session.*"
#[test]
fn reads_session_details_when_it_calls_session() {
    assert_eq!(text(&report(json!({ "calls": ["session.title"] })), "reads").unwrap(), "Session details (title, folder, harness)");
}

// "lists state, saved data, pages and the other calls"
#[test]
fn lists_state_saved_data_pages_and_the_other_calls() {
    assert_eq!(text(&report(json!({ "state": ["count", "open"] })), "state").unwrap(), "Keeps per-session state: count, open");
    assert_eq!(text(&report(json!({ "calls": ["store.get"] })), "store").unwrap(), "Reads its saved data");
    assert_eq!(text(&report(json!({ "calls": ["store.get", "store.set"] })), "store").unwrap(), "Saves data for you on this machine");
    assert_eq!(text(&report(json!({ "pages": ["a.html", "b.html"] })), "pages").unwrap(), "Shows its own pages: a.html, b.html");
    assert_eq!(
        text(&report(json!({ "calls": ["ui.toast", "ui.open", "clock.every"] })), "also").unwrap(),
        "Shows notices, opens panes, runs timers"
    );
    assert_eq!(keys(&report(json!({}))), strs(&["changes", "network", "files"]));
}

// "keeps the order of the rows"
#[test]
fn keeps_the_order_of_the_rows() {
    let r = report(json!({
        "hooks": [render(json!({ "component": "Pane" })), { "event": "ui.press" }],
        "calls": ["store.set", "ui.toast", "session.title"], "state": ["x"], "pages": ["p.html"]
    }));
    assert_eq!(keys(&r), strs(&["draws", "listens", "reads", "state", "store", "pages", "also", "changes", "network", "files"]));
}

fn panes() -> Value {
    json!([{ "id": "board", "title": "Release board" }, { "id": "log", "title": "Log" }])
}

// "adds a row naming the panes in the + menu, after what it draws"
#[test]
fn declared_panes_adds_a_row_naming_the_panes_in_the_menu_after_what_it_draws() {
    let r = report(json!({ "hooks": [render(json!({ "component": ["ToolUse", "Pane"] }))], "panes": panes() }));
    assert_eq!(keys(&r)[..3], strs(&["draws", "adds", "reads"])[..]);
    assert_eq!(text(&r, "adds").unwrap(), "A pane in the + menu: Release board and Log");
}

// "drops 'A pane it opens' when panes are declared, and keeps it when they aren't"
#[test]
fn declared_panes_drops_a_pane_it_opens_when_panes_are_declared_and_keeps_it_when_they_aren_t() {
    let hooks = json!([render(json!({ "component": "Pane" }))]);
    assert_eq!(keys(&report(json!({ "hooks": hooks, "panes": panes() }))), strs(&["adds", "changes", "network", "files"]));
    assert_eq!(text(&report(json!({ "hooks": hooks })), "draws").unwrap(), "A pane it opens");
}

// "gives the first error with its position and the warning count"
#[test]
fn check_problems_gives_the_first_error_with_its_position_and_the_warning_count() {
    let r = report(json!({
        "errors": [{ "line": 12, "column": 3, "code": "x", "message": "Call on() inside register" }, { "code": "y", "message": "Other" }],
        "warnings": [{ "code": "w", "message": "w" }]
    }));
    let p = check_problems(Some(&r));
    let e = p.error.expect("an error");
    assert_eq!((e.message.as_str(), e.line, e.column, e.place.as_deref()), ("Call on() inside register", Some(12), Some(3), Some("12:3")));
    assert_eq!(p.warnings, 1);
}

// "copes with no report and no position"
#[test]
fn check_problems_copes_with_no_report_and_no_position() {
    assert_eq!(check_problems(None), Default::default());
    assert_eq!(check_problems(Some(&report(json!({ "errors": [{ "code": "x", "message": "Bad" }] })))).error.unwrap().place, None);
}

// ---- ModReviewDialog.test.ts ----

fn test_chips() -> Value {
    report(json!({
        "hooks": [render(json!({ "component": ["ToolUse", "ToolResult"], "props": { "tool": ["bash", "shell"] } }))],
        "calls": ["ui.resolve"]
    }))
}

/// A rig on a draft, with the Keep dialog opened by Keep… (or Review… when the agent asked).
fn open(extra: Value, check: Value) -> Rig {
    let mut r = rig(move |f| f.check = json!({ "check": check }));
    r.rt.update(|s, _| s.projects[0].sessions[0].title = "Make test output readable".into());
    card(&mut r, extra);
    r.press(if r.has("mod-draft-review") { "mod-draft-review" } else { "mod-draft-keep" });
    r
}

fn rows(r: &Rig) -> Vec<String> {
    let mut found: Vec<(usize, String)> = Vec::new();
    for e in r.rt.elements().iter().filter(|e| !e.exiting) {
        let Some(k) = e.key.as_deref().and_then(|k| k.strip_prefix("mod-review-row-")) else { continue };
        if let Some(i) = k.strip_suffix("-label").and_then(|i| i.parse::<usize>().ok()) {
            found.push((i, e.text.clone().unwrap_or_default() + &r.text_of(&format!("mod-review-row-{i}-text"))));
        }
    }
    found.sort();
    found.into_iter().map(|f| f.1).collect()
}

/// A disabled button is drawn at half opacity (`ui::button::disabled`).
fn disabled(r: &Rig, key: &str) -> bool {
    r.rt.elements().iter().find(|e| e.key.as_deref() == Some(key)).is_some_and(|e| e.opacity < 0.9)
}

fn type_note(r: &mut Rig, words: &str) {
    crate::tests::click(&mut r.rt, "mod-review-note");
    r.rt.key(rune::KeyEvent { key: Key::Text(words.into()), mods: Mods::default() });
    r.rt.frame(FRAME);
}

// "asks 'Keep test-chips?' and lists what the worked example touches"
#[test]
fn mod_review_dialog_asks_keep_test_chips_and_lists_what_the_worked_example_touches() {
    let mut r = open(json!({}), test_chips());
    assert!(r.has("mod-review-dialog"));
    r.settle();
    let w = r.words();
    assert!(w.contains("Keep ") && w.contains("test-chips") && w.contains("v1 · 47 lines · written in “Make test output readable”"), "{w}");
    assert_eq!(
        rows(&r),
        strs(&[
            "DrawsRows for bash and shell calls (on the line, and the opened body)",
            "ReadsTool input and output of those calls",
            "ChangesNothing the agent sees",
            "NetworkNone",
            "FilesNone",
        ])
    );
    assert!(!r.has("mod-review-loading"));
}

// "lists a mod with pages, state and several hooks"
#[test]
fn mod_review_dialog_lists_a_mod_with_pages_state_and_several_hooks() {
    let rich = report(json!({
        "hooks": [render(json!({ "component": "ComposerBand" })), { "event": "ui.press" }, { "event": "session.start" }],
        "calls": ["store.set", "ui.toast", "session.title"], "state": ["count"], "pages": ["panel.html"]
    }));
    let r = open(json!({}), rich);
    let w = r.words();
    for part in [
        "A band above the composer",
        "Buttons and fields it draws",
        "Keeps per-session state: count",
        "Saves data for you",
        "panel.html",
        "Shows notices",
    ] {
        assert!(w.contains(part), "{part}");
    }
}

// "says so when there is no report, and Keep stays possible"
#[test]
fn mod_review_dialog_says_so_when_there_is_no_report_and_keep_stays_possible() {
    let r = open(json!({}), Value::Null);
    assert!(r.text_of("mod-review-unchecked").contains("the mod runtime isn't running"));
    assert!(!disabled(&r, "mod-review-keep"));
}

// "blocks Keep on an error, and only warns on a warning"
#[test]
fn mod_review_dialog_blocks_keep_on_an_error_and_only_warns_on_a_warning() {
    let bad = report(json!({ "ok": false, "errors": [{ "line": 12, "column": 3, "code": "x", "message": "Call on() inside register" }] }));
    let r = open(json!({}), bad);
    assert_eq!(r.text_of("mod-review-error-text"), "Fix this first: Call on() inside register (line 12:3)");
    assert!(disabled(&r, "mod-review-keep"));

    let warned = report(json!({ "warnings": [{ "code": "w", "message": "Unused state key" }] }));
    let r = open(json!({}), warned);
    assert!(r.text_of("mod-review-warnings").contains("1 warning"));
    assert!(!disabled(&r, "mod-review-keep"));
}

// "keeps with the note, and closes"
#[test]
fn mod_review_dialog_keeps_with_the_note_and_closes() {
    let mut r = open(json!({}), test_chips());
    type_note(&mut r, "Counts are easier to scan.");
    r.press("mod-review-keep");
    let sent = r.calls("POST", "/api/sessions/s-1/mods/drafts/test-chips/keep");
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].1, json!({ "note": "Counts are easier to scan." }));
    assert!(!r.has("mod-review-dialog") && r.rt.state.mod_drafts.review.is_none());
}

// "shows the server's words when Keep is refused, and stays open"
#[test]
fn mod_review_dialog_shows_the_server_s_words_when_keep_is_refused_and_stays_open() {
    let mut r = open(json!({}), test_chips());
    r.with(|f| f.keep = ("400 Bad Request", json!({ "error": "The check found a problem: line 3" })));
    r.press("mod-review-keep");
    assert_eq!(r.text_of("mod-review-refused-text"), "The check found a problem: line 3");
    assert!(r.has("mod-review-dialog"));
    assert!(!disabled(&r, "mod-review-keep"), "free to try again");
}

// "without a keep request, Cancel only closes"
#[test]
fn mod_review_dialog_without_a_keep_request_cancel_only_closes() {
    let mut r = open(json!({}), test_chips());
    assert!(r.words().contains("Cancel") && !r.words().contains("Discard"));
    r.press("mod-review-decline");
    assert!(r.calls("DELETE", "/keep-request").is_empty());
    assert!(!r.has("mod-review-dialog"));
}

// "with the agent's request, prefills the note and Discard tells the agent"
#[test]
fn mod_review_dialog_with_the_agent_s_request_prefills_the_note_and_discard_tells_the_agent() {
    let mut r = open(json!({ "keepRequest": { "note": "Counts are easier to scan.", "at": "2026-10-10T10:00:00Z" } }), test_chips());
    assert_eq!(r.rt.state.mod_drafts.dialog.note.text, "Counts are easier to scan.");
    assert!(r.words().contains("Discard tells the agent you didn't keep it. The draft stays on in this session."));
    assert!(r.words().contains("Discard"));
    r.press("mod-review-decline");
    assert_eq!(r.calls("DELETE", "/api/sessions/s-1/mods/drafts/test-chips/keep-request").len(), 1);
    assert!(!r.has("mod-review-dialog"));
}

// ---- The rest of rows 10.2, 10.7 and 10.8 ----

// The check is asked once when the dialog opens, and not before.
#[test]
fn the_draft_is_checked_when_the_dialog_opens_and_not_before() {
    let mut r = rig(|f| f.check = json!({ "check": test_chips() }));
    card(&mut r, json!({}));
    assert!(r.calls("GET", "/check").is_empty());
    r.press("mod-draft-keep");
    assert_eq!(r.calls("GET", "/api/sessions/s-1/mods/drafts/test-chips/check").len(), 1);
    assert!(matches!(r.rt.state.mod_drafts.dialog.checked, Checked::Report(Some(_))));
}

// A failed check shows the request's words, and Keep stays possible.
#[test]
fn a_failed_check_shows_the_request_s_error_words() {
    let mut r = open(json!({}), test_chips());
    r.rt.update(|s, _| s.mod_drafts.dialog.checked = Checked::Failed("Fleet couldn't check this mod.".into()));
    r.settle();
    assert_eq!(r.text_of("mod-review-unchecked"), "Fleet couldn't check this mod.");
    assert!(!disabled(&r, "mod-review-keep"));
}

// Discard that fails says so and stays open; a 404 counts as done.
#[test]
fn discard_that_fails_says_so_and_a_404_counts_as_done() {
    let mut r = open(json!({ "keepRequest": { "note": null, "at": "2026-10-10T10:00:00Z" } }), test_chips());
    r.with(|f| f.dismiss = "500 Internal Server Error");
    r.press("mod-review-decline");
    assert!(r.has("mod-review-dialog") && r.has("mod-review-refused"));
    r.with(|f| f.dismiss = "404 Not Found");
    r.press("mod-review-decline");
    assert!(!r.has("mod-review-dialog"));
}

// Keep sends no body for an empty note, and leaves the draft's request gone.
#[test]
fn keep_sends_no_note_when_the_field_is_empty() {
    let mut r = open(json!({}), test_chips());
    r.press("mod-review-keep");
    assert_eq!(r.calls("POST", "/keep")[0].1, json!({}));
}
