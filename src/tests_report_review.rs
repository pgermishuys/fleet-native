//! Report a problem, piece R of docs/parity/09-app-frame.md: `ProblemReportDialog.test.ts` cases 6 to 10 under their names,
//! driven through the dialog's own buttons against a stand-in server on this machine (nothing here can reach a real inbox),
//! and the rows Fleet's tests don't name (Back, the files, the screenshot view, Copy report id).
use crate::fleet::report::{Source, Stage};
use crate::fleet::report_form::{ReportForm, ReviewFile};
use crate::mock::report::shot;
use crate::tests::{FRAME, click};
use crate::tests_report::{Heard, asked, rig, standin};
use crate::{State, test_wait};
use rune::{ElementInfo, Key, KeyEvent, Mods, Runtime};
use std::sync::mpsc::Receiver;

type Rt = Runtime<State>;

fn info(rt: &Rt, key: &str) -> ElementInfo {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).unwrap_or_else(|| panic!("no element keyed {key:?}"))
}

fn all_text(rt: &Rt) -> String {
    rt.elements().into_iter().filter_map(|e| e.text).collect::<Vec<_>>().join("\n")
}

fn type_into(rt: &mut Rt, key: &str, words: &str) {
    click(rt, key);
    rt.key(KeyEvent { key: Key::Text(words.into()), mods: Mods::default() });
    rt.frame(FRAME);
    rt.frame(FRAME);
}

/// `opened()` and the description typed: the dialog open on the first session, a screenshot taken, the stand-in at the other end.
fn opened(can_send: bool, refusal: Option<&'static str>) -> (Rt, Receiver<crate::fleet::link::Job>, Heard) {
    let (base, heard) = standin(refusal, can_send);
    let (mut rt, queue) = rig(base);
    rt.update(|s, _| {
        s.projects[0].sessions[0].id = "s1".into();
        s.projects[0].sessions[0].title = "Fix invoice rounding".into();
        s.route = crate::fleet::store::Route::Session("s1".into());
        s.report.hide();
        s.report_open(Source::Help, None, "");
        s.report_set_screenshot(Some(shot()));
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    (rt, queue, heard)
}

/// Types the description and presses Review what's sent, then waits for the Review step.
fn reviewed(rt: &mut Rt, queue: &Receiver<crate::fleet::link::Job>) {
    type_into(rt, "report-description", "Stopped updating.");
    click(rt, "report-review");
    test_wait::drive(rt, queue, "the Review step", |s| s.report.stage == Stage::Review, test_wait::app_state);
    rt.frame(FRAME);
    rt.frame(FRAME);
}

fn press(rt: &mut Rt, queue: &Receiver<crate::fleet::link::Job>, key: &str, what: &str, done: impl FnMut(&State) -> bool) {
    click(rt, key);
    test_wait::drive(rt, queue, what, done, test_wait::app_state);
    rt.frame(FRAME);
    rt.frame(FRAME);
}

// ---- ProblemReportDialog.test.ts -------------------------------------------------------------------------------------------

#[test]
fn shows_the_prepared_report_with_labels_and_what_they_replaced_on_request() {
    let (mut rt, queue, _) = opened(true, None);
    reviewed(&mut rt, &queue);
    let text = all_text(&rt);
    assert!(text.contains("2 private details replaced"));
    assert!(text.contains("1 folder, 1 user name"));
    let preview = info(&rt, "report-preview").text.unwrap();
    assert_eq!(preview, "## What happened\nStopped updating in ‹folder-1›.\n");

    click(&mut rt, "report-show-removed");
    let preview = info(&rt, "report-preview").text.unwrap();
    assert_eq!(preview, "## What happened\nStopped updating in /work/acme-payments ‹folder-1›.\n");

    click(&mut rt, "report-file-log");
    let preview = info(&rt, "report-preview").text.unwrap();
    assert!(preview.contains("aborted for sam ‹user›"), "{preview}");
}

#[test]
fn sends_the_reviewed_report_with_the_screenshot_title_and_contact_then_shows_its_id() {
    let (mut rt, queue, heard) = opened(true, None);
    type_into(&mut rt, "report-contact", "@sam");
    reviewed(&mut rt, &queue);
    rt.update(|s, _| s.report_form.title = rune::TextEdit::new(""));
    rt.frame(FRAME);
    type_into(&mut rt, "report-title", "Conversation stops after sleep");
    press(&mut rt, &queue, "report-send", "the Sent step", |s| s.report.stage == Stage::Sent);

    let sent = &asked(&heard, "/api/reports/send")[0];
    assert_eq!(sent["kind"], "bug");
    assert_eq!(sent["title"], "Conversation stops after sleep");
    assert_eq!(sent["body"], "## What happened\nStopped updating in ‹folder-1›.\n");
    assert_eq!(sent["contact"], "@sam");
    assert_eq!(sent["labels"], serde_json::json!(["opencode2", "desktop"]));
    assert!(sent["screenshot"].as_str().is_some_and(|d| d.starts_with("data:image/png;base64,")));
    assert!(info(&rt, "report-sent-id").text.unwrap().contains("r_abc234"));
    assert!(all_text(&rt).contains("a reply goes to @sam"));
}

#[test]
fn leaves_the_screenshot_out_once_its_removed_on_the_review_screen() {
    let (mut rt, queue, heard) = opened(true, None);
    reviewed(&mut rt, &queue);
    click(&mut rt, "report-file-screenshot");
    assert_eq!(rt.state.report_form.file, ReviewFile::Screenshot);
    // The dialog is shorter here and settles to its new place before the next press.
    for _ in 0..30 {
        rt.frame(FRAME);
    }
    click(&mut rt, "report-remove-screenshot");
    assert!(rt.find("report-file-screenshot").is_none());
    assert_eq!((rt.state.report.open, rt.state.report.stage), (true, Stage::Review));
    assert_eq!(rt.state.report_form.file, ReviewFile::Report);
    rt.frame(FRAME);
    press(&mut rt, &queue, "report-send", "the Sent step", |s| s.report.stage == Stage::Sent);
    assert!(asked(&heard, "/api/reports/send")[0]["screenshot"].is_null());
}

#[test]
fn keeps_the_report_open_and_says_why_when_sending_fails() {
    let why = "Too many reports from this address. Try again in a few minutes.";
    let (mut rt, queue, _) = opened(true, Some(why));
    reviewed(&mut rt, &queue);
    press(&mut rt, &queue, "report-send", "the answer", |s| s.report.error.is_some());
    assert_eq!(info(&rt, "report-error").text.as_deref(), Some(why));
    assert_eq!(info(&rt, "report-error").role, "alert");
    assert!(rt.find("report-send").is_some(), "still on Review, Send still there");
}

#[test]
fn turns_send_off_when_this_fleet_has_no_inbox() {
    let (mut rt, queue, _) = opened(false, None);
    reviewed(&mut rt, &queue);
    assert!(info(&rt, "report-send").opacity < 1.0);
    assert!(all_text(&rt).contains("Save it as a file instead"));
    rt.update(|s, _| s.report_send_reviewed());
    assert_eq!(rt.state.report.busy, None, "Send does nothing while it's off");
}

// ---- the rows Fleet's tests don't name ---------------------------------------------------------------------------------------

#[test]
fn back_returns_to_describe_with_what_was_typed() {
    let (mut rt, queue, _) = opened(true, None);
    reviewed(&mut rt, &queue);
    click(&mut rt, "report-back");
    assert_eq!(rt.state.report.stage, Stage::Describe);
    assert_eq!(rt.state.report_form.description.text, "Stopped updating.");
    assert!(rt.find("report-review").is_some());
}

#[test]
fn a_blank_title_sends_the_servers_own_and_a_report_with_no_private_details_says_so() {
    let (mut rt, queue, heard) = opened(true, None);
    reviewed(&mut rt, &queue);
    rt.update(|s, _| {
        s.report_form.title = rune::TextEdit::new("   ");
        s.report.prepared.as_mut().unwrap().replacements.clear();
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert_eq!(info(&rt, "report-replaced").text.as_deref(), Some("No private details found."));
    assert!(rt.find("report-show-removed").is_none(), "nothing was removed, so nothing to show");
    press(&mut rt, &queue, "report-send", "the Sent step", |s| s.report.stage == Stage::Sent);
    assert_eq!(asked(&heard, "/api/reports/send")[0]["title"], "Stopped updating in ‹folder-1›");
}

#[test]
fn lists_what_was_left_out_and_hides_the_log_when_there_is_none() {
    let (mut rt, queue, _) = opened(true, None);
    reviewed(&mut rt, &queue);
    rt.update(|s, _| {
        let p = s.report.prepared.as_mut().unwrap();
        p.log = None;
        p.problems = vec![("the log".into(), "It could not be read.".into())];
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(rt.find("report-file-log").is_none());
    assert!(all_text(&rt).contains("Left out: the log. It could not be read."));
}

#[test]
fn copy_report_id_puts_the_id_on_the_clipboard_and_says_copied_and_done_closes() {
    let (mut rt, queue, _) = opened(true, None);
    reviewed(&mut rt, &queue);
    press(&mut rt, &queue, "report-send", "the Sent step", |s| s.report.stage == Stage::Sent);
    assert!(rt.find("report-steps").is_none(), "no steps once it is sent");
    click(&mut rt, "report-copy-id");
    assert!(rt.state.report_form.copied);
    assert_eq!(info(&rt, "report-copy-id").label.as_deref(), Some("Copied"));
    click(&mut rt, "report-done");
    assert!(!rt.state.report.open);
    assert_eq!(rt.state.report_form.copied, ReportForm::default().copied);
}

#[test]
fn save_as_file_writes_the_zip_and_stays_on_review() {
    let (mut rt, queue, heard) = opened(true, None);
    let dir = std::env::temp_dir().join(format!("fleet-native-9r-{}", std::process::id()));
    let to = dir.clone();
    rt.update(move |s, _| s.report.downloads = Some(to));
    reviewed(&mut rt, &queue);
    press(&mut rt, &queue, "report-save", "the saved zip", |s| s.report.saved.is_some());
    assert_eq!(rt.state.report.stage, Stage::Review);
    assert!(dir.join("fleet-report-r1.zip").exists());
    assert_eq!(asked(&heard, "/api/reports/file").len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}
