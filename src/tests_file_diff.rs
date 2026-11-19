//! Diff, the stripe and a deleted file (component 11, piece J): `FileCanvas.test.ts` "Diff shows the git base against the buffer" (the
//! read-only half) on the invented `src/paginate.py` the scenes draw, a deleted file's last version, the folds, and the stripe.
//! Which lines differ and how they fold are `src/conv/tests/file_diff.rs`'s and `changed_lines.rs`'s.
use crate::State;
use crate::fleet::canvases::FileView;
use crate::mock::file_tab::{Case, PATH};
use crate::tests::{FRAME, app, click};
use rune::Runtime;

fn open(case: Case) -> Runtime<State> {
    let mut rt = app();
    rt.update(move |s, _| {
        s.sheet_w = 1400.0;
        s.show_file(case, true)
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

fn has(rt: &Runtime<State>, key: &str) -> bool {
    rt.find(key).is_some()
}

fn said(rt: &Runtime<State>, key: &str) -> String {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).and_then(|e| e.text).unwrap_or_default()
}

fn texts(rt: &Runtime<State>, prefix: &str) -> Vec<String> {
    rt.elements().into_iter().filter(|e| e.key.as_deref().is_some_and(|k| k.starts_with(prefix))).filter_map(|e| e.text).collect()
}

/// The numbers drawn in the gutters, in order (a removed line's is empty).
fn numbers(rt: &Runtime<State>, prefix: &str) -> Vec<String> {
    rt.elements()
        .into_iter()
        .filter(|e| e.key.as_deref().is_some_and(|k| k.starts_with(prefix) && k.ends_with("-t")))
        .map(|e| e.text.unwrap_or_default())
        .collect()
}

fn keys(rt: &Runtime<State>, prefix: &str) -> Vec<String> {
    rt.elements().into_iter().filter_map(|e| e.key).filter(|k| k.starts_with(prefix)).collect()
}

// FileCanvas.test.ts
#[test]
fn diff_shows_the_git_base_against_the_buffer() {
    let rt = open(Case::Diff);
    assert_eq!(rt.state.open_canvases().active().unwrap().file.as_ref().unwrap().view, FileView::Diff);
    let lines = texts(&rt, "diff-text-");
    assert!(lines.iter().any(|l| l == "    last = total // size"), "the base's line is there, removed: {lines:?}");
    assert!(lines.iter().any(|l| l == "    last = (total + size - 1) // size - 1"), "and the line that replaced it");
    assert!(!has(&rt, "file-code"), "the plain code view gives way to the diff");
    // Read-only: Revert is editing's (slice 2).
    assert!(!rt.elements().iter().any(|e| e.text.as_deref() == Some("Revert")));
}

#[test]
fn a_removed_line_comes_above_the_line_that_replaced_it_and_has_no_number() {
    let rt = open(Case::Diff);
    let order: Vec<String> = rt
        .elements()
        .into_iter()
        .filter(|e| e.key.as_deref().is_some_and(|k| k.starts_with("diff-text-")))
        .filter_map(|e| e.text)
        .filter(|t| t.starts_with("    last"))
        .collect();
    assert_eq!(order, ["    last = total // size", "    last = (total + size - 1) // size - 1"]);
    // The numbers count the new text: the removed lines show none.
    let numbers = numbers(&rt, "diff-no-");
    assert!(numbers.iter().any(|n| n == "26") && numbers.iter().filter(|n| n.is_empty()).count() >= 3);
}

#[test]
fn unchanged_runs_fold_to_a_bar_that_says_how_many_and_opens_on_a_click() {
    let mut rt = open(Case::Diff);
    // 22 rows come before the first change (23 less its margin of 3), so the first 22 fold.
    assert_eq!(rt.find("diff-fold-0").map(|_| ()), Some(()));
    let bar = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("diff-fold-0")).unwrap();
    assert_eq!(bar.label.as_deref(), Some("22 unchanged lines"));
    assert!(!keys(&rt, "diff-text-").iter().any(|k| k == "diff-text-0"), "the folded rows aren't built");
    click(&mut rt, "diff-fold-0");
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(!has(&rt, "diff-fold-0"), "the bar is gone once it is opened");
    assert_eq!(said(&rt, "diff-text-0"), "\"\"\"Splits a list of rows into pages.\"\"\"");
}

#[test]
fn a_file_with_no_git_base_is_compared_with_the_text_as_it_was_read() {
    let mut rt = open(Case::Diff);
    rt.update(|s, _| s.file_bases.clear());
    rt.frame(FRAME);
    rt.frame(FRAME);
    // Nothing differs from the text as read, so nothing folds and nothing is removed.
    assert!(keys(&rt, "diff-fold-").is_empty());
    assert_eq!(texts(&rt, "diff-text-").iter().filter(|l| l.as_str() == "    last = total // size").count(), 0);
}

#[test]
fn diff_has_no_stripe_of_its_own_in_the_gutter() {
    let rt = open(Case::Diff);
    assert!(keys(&rt, "diff-no-").iter().all(|k| !k.ends_with("-stripe")));
    assert!(keys(&rt, "code-no-").is_empty());
}

#[test]
fn a_deleted_file_shows_its_last_version_as_removed_lines() {
    let rt = open(Case::Deleted);
    assert_eq!(said(&rt, "file-deleted-note"), "This file was deleted in this session. Its last version:");
    assert!(!has(&rt, "file-unavailable"), "the failed read's own note gives way");
    let lines = texts(&rt, "diff-text-");
    assert!(lines.iter().any(|l| l == "    last = total // size"));
    assert!(!lines.iter().any(|l| l == "    last = (total + size - 1) // size - 1"), "only the last version, not the one on disk");
    // Every line keeps its number, and the view is the bare code: no switch, no Wrap.
    assert!(numbers(&rt, "diff-no-").iter().any(|n| n == "1") && numbers(&rt, "diff-no-").iter().any(|n| n == "26"));
    assert!(!has(&rt, "file-view-switch") && !has(&rt, "file-wrap"));
}

#[test]
fn a_file_that_failed_to_read_but_was_not_deleted_keeps_its_own_note() {
    let mut rt = open(Case::Deleted);
    rt.update(|s, _| {
        let id = s.canvas_session();
        s.diffs.apply(
            &serde_json::json!({"diffs": [{"file": PATH, "status": "modified", "additions": 1, "deletions": 1}], "available": true}),
        );
        let _ = id;
    });
    rt.frame(FRAME);
    assert!(!has(&rt, "file-deleted-note"));
    assert!(has(&rt, "file-unavailable"));
}

// markStripe: which lines get the bar
#[test]
fn edit_marks_the_lines_that_differ_from_the_git_base_in_a_gutter_of_its_own() {
    let rt = open(Case::Stripe);
    let marked: Vec<String> = keys(&rt, "code-no-").into_iter().filter(|k| k.ends_with("-stripe")).collect();
    assert_eq!(marked, ["code-no-26-stripe", "code-no-27-stripe", "code-no-28-stripe"]);
    // The gutter is there for every row, so the code starts at the same place; the others just have no bar in it.
    assert!(has(&rt, "code-text-1"));
}

#[test]
fn a_file_that_is_not_among_the_changes_has_no_stripe() {
    let mut rt = open(Case::Stripe);
    rt.update(|s, _| {
        let id = s.canvas_session();
        s.diffs.reset(&id);
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(keys(&rt, "code-no-").iter().all(|k| !k.ends_with("-stripe")));
}

// cm-agent-flash
#[test]
fn lines_the_agent_changed_flash_from_the_clock_and_stop_after_a_second_and_a_half() {
    let mut rt = open(Case::Code);
    let id = rt.state.canvas_session();
    let machine = rt.state.machine_key_of(&id).to_string();
    rt.update(move |s, _| {
        let now = s.clock;
        s.file_live.flash(&machine, &id, PATH, (3, 3), now);
    });
    let mut woke = false;
    rt.update(|s, tx| woke = s.keep_file_flash(tx));
    assert!(woke, "frames keep coming while a line flashes");
    rt.update(|s, _| s.clock += crate::fleet::file_live::FLASH_SECONDS + 0.1);
    rt.update(|s, tx| woke = s.keep_file_flash(tx));
    assert!(!woke, "and stop once it has faded");
    let t = crate::theme::Theme::of(true);
    assert_eq!(crate::view::code_view::flash_tint(&t, 0.0).a, 0.28);
    assert_eq!(crate::view::code_view::flash_tint(&t, 1.0).a, 0.0);
    assert!(crate::view::code_view::flash_tint(&t, 0.5).a < 0.28 * 0.5, "it eases out: past half its strength by the middle");
}
