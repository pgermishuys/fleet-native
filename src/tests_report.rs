//! Report a problem, piece P of docs/parity/09-app-frame.md: `problem-report.test.ts` (5) under its names, and
//! `ProblemReportDialog.test.ts` cases 3 to 5 as request tests. Every request goes to a stand-in server on this machine
//! that answers as Fleet's report routes do; nothing here can reach a real inbox.
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::report::*;
use crate::tests::app;
use crate::{State, test_wait};
use rune::Runtime;
use serde_json::{Value, json};
use std::io::Write;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

fn replacement(label: &str, kind: &str, shown: &str) -> Replacement {
    Replacement { label: label.into(), kind: kind.into(), shown: shown.into() }
}

fn run(text: &str, label: bool) -> Run {
    Run { text: text.into(), label }
}

// ─── problem-report.test.ts ──────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn splits_text_into_plain_runs_and_labels() {
    assert_eq!(
        split_labels("in ‹folder-1›/x by ‹user›."),
        vec![run("in ", false), run("‹folder-1›", true), run("/x by ", false), run("‹user›", true), run(".", false)]
    );
    assert_eq!(count_labels(Some("‹secret-1› and ‹secret-2›")), 2);
    assert_eq!(count_labels(None), 0);
}

#[test]
fn sums_up_what_was_replaced_in_words() {
    let all = [
        replacement("‹folder-1›", "folder", "/a"),
        replacement("‹folder-2›", "folder", "/b"),
        replacement("‹secret-1›", "secret", "ghp_…"),
    ];
    assert_eq!(summarize_replacements(&all), "2 folders, 1 token");
}

#[test]
fn names_the_desktop_app() {
    // The browser names of the Vue test are not in a native app: it only ever says "Desktop app".
    assert_eq!(describe_app("0.39.0", "linux"), "Desktop app 0.39.0 · linux");
}

#[test]
fn names_the_screen_from_the_address() {
    assert_eq!(describe_screen("/"), "Sessions");
    assert_eq!(describe_screen("/sessions/abc"), "Sessions");
    assert_eq!(describe_screen("/settings"), "Settings");
    assert_eq!(describe_screen("/automations/a1"), "Automations");
    assert_eq!(describe_screen("/mission-control"), "Mission control");
}

#[test]
fn keeps_other_machines_names_and_addresses_and_fleets_own_address_out_of_the_report() {
    let values = private_values([("studio-mac", "http://100.64.90.72:2113")], "https://fleet.example.net");
    assert_eq!(
        values,
        vec![
            (PrivateKind::Machine, "studio-mac".to_string()),
            (PrivateKind::Url, "http://100.64.90.72:2113".to_string()),
            (PrivateKind::Url, "https://fleet.example.net".to_string()),
        ]
    );
}

#[test]
fn leaves_a_lookalike_alone_and_finds_labels_beside_each_other() {
    let runs = split_labels("‹a›‹b-2› ‹Upper› ‹x-› ‹›");
    assert_eq!(runs.iter().filter(|r| r.label).map(|r| r.text.as_str()).collect::<Vec<_>>(), ["‹a›", "‹b-2›"]);
}

#[test]
fn names_a_kind_that_has_no_words_by_itself() {
    assert_eq!(summarize_replacements(&[replacement("‹a-1›", "badge", "x"), replacement("‹a-2›", "badge", "y")]), "2 badges");
    assert_eq!(summarize_replacements(&[]), "");
}

// ─── the stand-in server ─────────────────────────────────────────────────────────────────────────────────────────────

pub(crate) type Heard = Arc<Mutex<Vec<(String, Value)>>>;

fn prepared() -> Value {
    json!({
        "title": "Stopped updating in ‹folder-1›",
        "body": "## What happened\nStopped updating in ‹folder-1›.\n",
        "log": "2026-09-27 14:02:03.911 [WRN] [SessionHub] aborted for ‹user›\n",
        "logEntries": 1,
        "labels": ["opencode2", "desktop"],
        "replacements": [
            {"label": "‹folder-1›", "kind": "folder", "shown": "/work/acme-payments"},
            {"label": "‹user›", "kind": "user", "shown": "sam"},
        ],
        "problems": [],
        "canSend": true,
    })
}

/// Answers the three report routes as Fleet does and writes down what was asked. `refusal` is what `/send` answers with
/// instead of an id, with status 429.
pub(crate) fn standin(refusal: Option<&'static str>, can_send: bool) -> (String, Heard) {
    let listener = crate::test_http::bind().expect("a port");
    let base = format!("http://{}", listener.local_addr().unwrap());
    let heard: Heard = Default::default();
    let log = heard.clone();
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let log = log.clone();
            std::thread::spawn(move || {
                let req = crate::test_http::read(&mut stream);
                if req.method.is_empty() {
                    return;
                }
                let (head, body) = (req.head, req.body);
                let path = head.lines().next().unwrap_or("").split(' ').nth(1).unwrap_or("").to_string();
                log.lock().unwrap().push((path.clone(), serde_json::from_slice(&body).unwrap_or(Value::Null)));
                let (status, extra, reply) = match path.as_str() {
                    "/api/reports/prepare" => {
                        let mut p = prepared();
                        p["canSend"] = json!(can_send);
                        (200, "content-type: application/json\r\n", p.to_string().into_bytes())
                    }
                    "/api/reports/send" => match refusal {
                        Some(why) => (429, "content-type: application/json\r\n", json!({"error": why}).to_string().into_bytes()),
                        None => (200, "content-type: application/json\r\n", json!({"id": "r_abc234"}).to_string().into_bytes()),
                    },
                    "/api/reports/file" => {
                        (200, "content-disposition: attachment; filename=\"fleet-report-r1.zip\"\r\n", b"PK\x03\x04zip".to_vec())
                    }
                    _ => (404, "", Vec::new()),
                };
                let head = format!("HTTP/1.1 {status} X\r\n{extra}content-length: {}\r\nconnection: close\r\n\r\n", reply.len());
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(&reply);
            });
        }
    });
    (base, heard)
}

pub(crate) fn rig(base: String) -> (Runtime<State>, Receiver<Job>) {
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let post = Post::new(move |job| {
        let _ = jobs.lock().unwrap().send(job);
    });
    let mut rt = app();
    let link = Link::start(base, post);
    rt.update(move |s, _| {
        s.link = Some(link);
        s.report.show(Opening { from: Source::Help, session: Some("s1".into()), description: String::new() });
    });
    (rt, queue)
}

pub(crate) fn asked(heard: &Heard, path: &str) -> Vec<Value> {
    heard.lock().unwrap().iter().filter(|c| c.0 == path).map(|c| c.1.clone()).collect()
}

fn request(include: Includes) -> PrepareRequest {
    PrepareRequest {
        kind: Kind::Bug,
        description: "Stopped updating in /work/acme-payments.".into(),
        expected: Some("  ".into()),
        session: Some("s1".into()),
        include,
        client: Client {
            app: Some(describe_app("0.1.0", "linux")),
            screen: Some(describe_screen("/sessions/s1")),
            shown_status: Some("Idle".into()),
            place: where_facts(1360.0, 880.0, 2.0, true),
            connection: vec![Fact::new("Live updates", "Connected")],
            private_values: private_values([("studio-mac", "http://100.64.90.72:2113")], "http://127.0.0.1:6262"),
        },
    }
}

fn reviewed(s: &State) -> SendRequest {
    SendRequest::reviewed(
        Kind::Bug,
        s.report.prepared.as_ref().expect("prepared"),
        "Conversation stops after sleep",
        Some("data:image/png;base64,iVBORw0KGgo="),
        " @sam ",
    )
}

// ─── ProblemReportDialog.test.ts 3 to 5, as requests ─────────────────────────────────────────────────────────────────

#[test]
fn asks_the_server_to_prepare_only_the_ticked_parts_with_what_the_window_knows() {
    let (base, heard) = standin(None, true);
    let (mut rt, queue) = rig(base);
    let include = Includes { log: false, ..Includes::default() };
    rt.update(move |s, _| s.report_prepare(request(include)));
    test_wait::drive(&mut rt, &queue, "the prepared report", |s| s.report.stage == Stage::Review, test_wait::app_state);

    let body = asked(&heard, "/api/reports/prepare").remove(0);
    assert_eq!(body["kind"], "bug");
    assert_eq!(body["sessionId"], "s1");
    assert_eq!(body["expected"], Value::Null, "a blank 'expected' is sent as null");
    assert_eq!(body["include"], json!({"environment": true, "where": true, "connection": true, "log": false}));
    assert_eq!(body["client"]["screen"], "Sessions");
    assert_eq!(body["client"]["app"], "Desktop app 0.1.0 · linux");
    assert_eq!(body["client"]["where"][0], json!({"name": "Window", "value": "1360×880 at 2×"}));
    assert_eq!(body["client"]["connection"][0]["name"], "Live updates");
    assert_eq!(body["client"]["privateValues"][2], json!({"kind": "url", "value": "http://127.0.0.1:6262"}));
    rt.update(|s, _| {
        let p = s.report.prepared.as_ref().unwrap();
        assert_eq!(p.title, "Stopped updating in ‹folder-1›");
        assert_eq!(summarize_replacements(&p.replacements), "1 folder, 1 user name");
        assert_eq!(count_labels(Some(&p.body)), 1);
        assert!(p.can_send && p.problems.is_empty() && p.log_entries == 1);
    });
}

#[test]
fn sends_the_reviewed_report_with_the_screenshot_title_and_contact_then_has_its_id() {
    let (base, heard) = standin(None, true);
    let (mut rt, queue) = rig(base);
    rt.update(|s, _| s.report_prepare(request(Includes::default())));
    test_wait::drive(&mut rt, &queue, "the prepared report", |s| s.report.stage == Stage::Review, test_wait::app_state);
    rt.update(|s, _| {
        let r = reviewed(s);
        s.report_send(r);
    });
    test_wait::drive(&mut rt, &queue, "the report's id", |s| s.report.stage == Stage::Sent, test_wait::app_state);

    let p = prepared();
    assert_eq!(
        asked(&heard, "/api/reports/send")[0],
        json!({
            "kind": "bug",
            "title": "Conversation stops after sleep",
            "body": p["body"],
            "log": p["log"],
            "screenshot": "data:image/png;base64,iVBORw0KGgo=",
            "contact": "@sam",
            "labels": ["opencode2", "desktop"],
        })
    );
    rt.update(|s, _| assert_eq!(s.report.sent_id.as_deref(), Some("r_abc234")));
}

#[test]
fn leaves_the_screenshot_and_contact_out_when_there_are_none() {
    let prepared = Prepared::parse(&prepared()).unwrap();
    let body = SendRequest::reviewed(Kind::Idea, &prepared, "   ", None, "").body();
    assert_eq!(body["screenshot"], Value::Null);
    assert_eq!(body["contact"], Value::Null);
    assert_eq!(body["title"], "Stopped updating in ‹folder-1›", "a blank title falls back to the server's");
    assert_eq!(body["kind"], "idea");
}

#[test]
fn keeps_the_report_open_and_says_why_when_sending_fails() {
    let (base, _) = standin(Some("Too many reports from this address. Try again in a few minutes."), true);
    let (mut rt, queue) = rig(base);
    rt.update(|s, _| s.report_prepare(request(Includes::default())));
    test_wait::drive(&mut rt, &queue, "the prepared report", |s| s.report.stage == Stage::Review, test_wait::app_state);
    rt.update(|s, _| {
        let r = reviewed(s);
        s.report_send(r);
    });
    test_wait::drive(&mut rt, &queue, "the refusal", |s| s.report.error.is_some(), test_wait::app_state);
    rt.update(|s, _| {
        assert_eq!(s.report.error.as_deref(), Some("Too many reports from this address. Try again in a few minutes."));
        assert_eq!((s.report.stage, s.report.busy, s.report.open), (Stage::Review, None, true));
        assert!(s.report.sent_id.is_none());
    });
}

#[test]
fn turns_send_off_when_this_fleet_has_no_inbox() {
    let (base, _) = standin(None, false);
    let (mut rt, queue) = rig(base);
    rt.update(|s, _| s.report_prepare(request(Includes::default())));
    test_wait::drive(&mut rt, &queue, "the prepared report", |s| s.report.stage == Stage::Review, test_wait::app_state);
    rt.update(|s, _| assert!(!s.report.prepared.as_ref().unwrap().can_send));
}

// ─── the rest of the requests ────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn saves_the_zip_to_the_downloads_folder_under_the_name_the_server_gave_and_never_over_a_file() {
    let (base, heard) = standin(None, true);
    let dir = std::env::temp_dir().join(format!("fleet-native-report-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (mut rt, queue) = rig(base);
    let to = dir.clone();
    rt.update(move |s, _| {
        s.report.downloads = Some(to);
        s.report_prepare(request(Includes::default()));
    });
    test_wait::drive(&mut rt, &queue, "the prepared report", |s| s.report.stage == Stage::Review, test_wait::app_state);
    for _ in 0..2 {
        rt.update(|s, tx| {
            let r = reviewed(s);
            s.report_save(r, tx);
        });
        test_wait::drive(&mut rt, &queue, "the saved file", |s| s.report.saved.is_some() && s.report.busy.is_none(), test_wait::app_state);
        rt.update(|s, _| s.report.saved = None);
    }
    assert_eq!(std::fs::read(dir.join("fleet-report-r1.zip")).unwrap(), b"PK\x03\x04zip");
    assert!(dir.join("fleet-report-r1 (1).zip").exists());
    assert_eq!(asked(&heard, "/api/reports/file").len(), 2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn names_the_file_from_the_header_and_cannot_be_sent_outside_the_folder() {
    assert_eq!(file_name("attachment; filename=\"fleet-report-r1.zip\""), "fleet-report-r1.zip");
    assert_eq!(file_name("attachment; filename=plain.zip; size=3"), "plain.zip");
    assert_eq!(file_name(""), "fleet-report.zip");
    assert_eq!(file_name("attachment; filename=\"../../etc/x.zip\""), "x.zip");
    assert_eq!(file_name("attachment; filename=\"..\""), "fleet-report.zip");
}

#[test]
fn says_what_the_server_said_else_the_fallback() {
    assert_eq!(error_words(429, "{\"error\":\"Slow down.\"}", SEND_FAILED), "Slow down.");
    assert_eq!(error_words(400, "{\"error\":{\"message\":\"Too long.\"}}", PREPARE_FAILED), "Too long.");
    assert_eq!(error_words(500, "<html>", SEND_FAILED), SEND_FAILED);
    assert_eq!(error_words(500, "{\"error\":\"  \"}", SAVE_FAILED), SAVE_FAILED);
    assert_eq!(error_words(500, "", PREPARE_FAILED), PREPARE_FAILED);
}

#[test]
fn a_server_that_does_not_answer_gets_the_footers_words() {
    // Nothing listens on this port.
    let base = crate::test_http::dead_address();
    assert_eq!(crate::fleet::http::report_prepare(&base, &json!({})).unwrap_err(), PREPARE_FAILED);
    assert_eq!(crate::fleet::http::report_send(&base, &json!({})).unwrap_err(), SEND_FAILED);
    assert_eq!(crate::fleet::http::report_file(&base, &json!({})).unwrap_err(), SAVE_FAILED);
}

// ─── the store (stores/problem-report.ts, close) ─────────────────────────────────────────────────────────────────────

#[test]
fn is_a_no_op_to_show_while_open_and_hide_forgets_everything() {
    let mut r = ReportState::default();
    let first = Opening { from: Source::Palette, session: None, description: "x".into() };
    assert!(r.show(first.clone()));
    assert!(!r.show(Opening { from: Source::Help, ..first.clone() }));
    assert_eq!(r.opening, Some(first));
    r.prepared = Some(Prepared::default());
    r.stage = Stage::Review;
    assert!(r.hide());
    assert!(!r.open && r.opening.is_none() && r.prepared.is_none() && r.stage == Stage::Describe);
}

#[test]
fn will_not_close_while_sending_and_drops_an_answer_that_comes_after_closing() {
    let (base, _) = standin(None, true);
    let (mut rt, queue) = rig(base);
    rt.update(|s, _| s.report_prepare(request(Includes::default())));
    // Closed before the answer: it must not bring the report back.
    rt.update(|s, _| assert!(s.report.hide()));
    let job = test_wait::recv("the late answer", &queue);
    rt.update(job);
    rt.update(|s, _| assert!(!s.report.open && s.report.prepared.is_none()));
    rt.update(|s, _| {
        s.report.show(Opening { from: Source::Help, session: None, description: String::new() });
        s.report.busy = Some(Busy::Send);
        assert!(!s.report.hide(), "Esc and the overlay do nothing while sending");
    });
}

#[test]
fn does_not_prepare_a_blank_description_or_twice_at_once() {
    let (base, heard) = standin(None, true);
    let (mut rt, queue) = rig(base);
    rt.update(|s, _| {
        let mut blank = request(Includes::default());
        blank.description = "  \n".into();
        s.report_prepare(blank);
        assert!(s.report.busy.is_none());
        s.report_prepare(request(Includes::default()));
        s.report_prepare(request(Includes::default()));
    });
    test_wait::drive(&mut rt, &queue, "the prepared report", |s| s.report.stage == Stage::Review, test_wait::app_state);
    assert_eq!(asked(&heard, "/api/reports/prepare").len(), 1);
}

// ─── against a scratch Fleet ─────────────────────────────────────────────────────────────────────────────────────────

/// Asks a scratch Fleet to prepare a report and to write its zip, and prints what came back. Never sends: a Fleet's
/// `/send` reaches the maintainers' real inbox.
///
///     FLEET_LIVE_URL=http://127.0.0.1:6451 cargo test live_report -- --ignored --nocapture
#[test]
#[ignore = "needs a scratch Fleet"]
fn live_report() {
    let Ok(url) = std::env::var("FLEET_LIVE_URL") else { return };
    assert!(url.contains("127.0.0.1") || url.contains("localhost"), "only a scratch Fleet on this machine");
    let (mut rt, queue) = rig(url);
    let dir = std::env::temp_dir().join(format!("fleet-native-report-live-{}", std::process::id()));
    let to = dir.clone();
    rt.update(move |s, _| {
        s.report.downloads = Some(to);
        let mut r = request(Includes::default());
        r.description = "Stopped updating in /tmp/fleet-native-scratch-9P/proj/demo after the token ghp_0123456789abcdefghijklmnopqrstuvwxyz1234 was pasted. Mail sam@example.com.".into();
        s.report_prepare(r);
    });
    test_wait::drive(
        &mut rt,
        &queue,
        "the prepared report",
        |s| s.report.stage == Stage::Review || s.report.error.is_some(),
        test_wait::app_state,
    );
    rt.update(|s, _| {
        let p = s.report.prepared.as_ref().expect("prepared");
        println!("title: {}\nlabels: {:?}\ncan_send: {}", p.title, p.labels, p.can_send);
        println!("replaced: {} · {}", p.replacements.len(), summarize_replacements(&p.replacements));
        println!("body labels: {}, log labels: {}", count_labels(Some(&p.body)), count_labels(p.log.as_deref()));
        for r in &p.replacements {
            println!("  {} ({}): {}", r.label, r.kind, r.shown);
        }
        for (item, why) in &p.problems {
            println!("left out: {item}. {why}");
        }
        println!("--- body ---\n{}", p.body);
    });
    rt.update(|s, tx| {
        let r = reviewed(s);
        s.report_save(r, tx);
    });
    test_wait::drive(&mut rt, &queue, "the saved zip", |s| s.report.saved.is_some() || s.report.error.is_some(), test_wait::app_state);
    rt.update(|s, _| {
        println!("saved: {:?} error: {:?}", s.report.saved.as_ref().map(|p| p.file_name().unwrap().to_owned()), s.report.error)
    });
    let _ = std::fs::remove_dir_all(&dir);
}

// `maxlength` stops typing at the limit; here what is typed past it is cut on the next poll.
#[test]
fn the_fields_are_cut_to_their_limits_as_they_are_typed() {
    use crate::fleet::text_limit::limit;
    let mut e = rune::TextEdit::new("abcdéf");
    assert!(!limit(&mut e, 6));
    assert!(limit(&mut e, 4));
    assert_eq!(e.text, "abcd");
    let mut rt = app();
    rt.update(|s, _| {
        s.report.open = true;
        s.report_form.contact.replace_text("x".repeat(250));
        s.report_form.description.replace_text("y".repeat(10_001));
    });
    assert!(rt.state.keep_text_limits());
    assert_eq!((rt.state.report_form.contact.text.len(), rt.state.report_form.description.text.len()), (200, 10_000));
    assert!(!rt.state.keep_text_limits());
}
