//! `Composer.other-machine.test.ts` ("Composer @ sessions on another machine"): the open session's `@` list offers the sessions
//! of the machine the session is on, since that machine looks a picked one up when the prompt is sent. The session is the one
//! started on a second Fleet from the New session page; each Fleet answers `GET /api/sessions` with its own.
use crate::tests::typed;
use crate::tests_new_send::{Rig, Shared, rig_on_another_machine};
use rune::{Key, Mods};
use serde_json::{Value, json};

fn row(id: &str, title: &str) -> Value {
    json!({ "sessionStatus": "idle", "harnessType": "opencode", "workspaceDirectory": "/srv/orbit",
        "session": { "id": id, "title": title, "time": { "created": 1, "updated": 2 } } })
}

/// The reads the `@` list makes (`retentionStatus=all`), apart from the list's own polling.
fn mention_reads(fake: &Shared) -> usize {
    fake.lock().unwrap().session_reads.iter().filter(|p| p.contains("limit=200")).count()
}

/// A session open on the second Fleet, which lists two sessions of its own; home lists two others.
fn on_falcon() -> (Rig, Shared) {
    let (mut r, falcon) = rig_on_another_machine();
    falcon.lock().unwrap().sessions = vec![row("ses-new", "Falcon open session"), row("ses-falcon-cache", "Falcon cache warmup")];
    r.fake.lock().unwrap().sessions = vec![row("ses-home-docs", "Home docs rewrite"), row("ses-home-cli", "Home CLI flags")];
    r.send_and_wait();
    assert_eq!(r.rt.state.machine_key_of("ses-new"), "m-falcon");
    (r, falcon)
}

fn offered(r: &Rig) -> Vec<String> {
    r.rt.state.auto.popup.as_ref().map(|p| p.items.iter().map(|i| i.label.clone()).collect()).unwrap_or_default()
}

#[test]
fn asks_the_sessions_machine_for_its_sessions_and_offers_them_not_the_live_machines() {
    let (mut r, falcon) = on_falcon();
    typed(&mut r.rt, "carry on from @Falcon");
    r.wait("the @ list to offer the machine's sessions", |s| {
        s.auto.popup.as_ref().is_some_and(|p| p.items.iter().any(|i| i.label == "Falcon cache warmup"))
    });
    assert!(mention_reads(&falcon) > 0);
    assert_eq!(mention_reads(&r.fake), 0);
    assert!(offered(&r).iter().all(|l| !l.contains("Home")), "{:?}", offered(&r));
}

#[test]
fn sends_a_picked_sessions_id_to_the_sessions_machine() {
    let (mut r, falcon) = on_falcon();
    // The first turn is over: a prompt now goes straight to the machine, not into its queue.
    r.rt.update(|s, _| {
        s.optimistic = None;
        s.session_mut().expect("the open session").status = crate::mock::Status::Idle;
    });
    typed(&mut r.rt, "carry on from @cache");
    r.wait("the @ list", |s| s.auto.popup.as_ref().is_some_and(|p| p.items.iter().any(|i| i.label == "Falcon cache warmup")));
    r.press(Key::Enter, Mods::default());
    assert!(r.rt.state.draft.text.starts_with("carry on from @") && r.rt.state.draft.text.ends_with(' '), "{:?}", r.rt.state.draft.text);
    r.press(Key::Enter, Mods::default());
    r.wait("the prompt on the machine", |_| !falcon.lock().unwrap().prompts.is_empty());
    let sent = falcon.lock().unwrap().prompts[0].clone();
    assert_eq!(sent.0, "ses-new");
    assert_eq!(sent.1["sessionReferences"][0]["sessionId"], "ses-falcon-cache");
    assert!(r.fake.lock().unwrap().prompts.is_empty(), "home got none");
}

#[test]
fn offers_the_live_machines_sessions_for_a_session_on_the_live_machine_as_before() {
    let mut r = crate::tests_new_send::rig();
    r.fake.lock().unwrap().sessions = vec![row("ses-home-docs", "Home docs rewrite"), row("ses-home-cli", "Home CLI flags")];
    r.text("Fix the login redirect");
    r.send_and_wait();
    typed(&mut r.rt, "see @Home");
    r.wait("the @ list", |s| s.auto.popup.as_ref().is_some_and(|p| p.items.iter().any(|i| i.label == "Home CLI flags")));
    assert!(mention_reads(&r.fake) > 0);
    assert!(offered(&r).iter().all(|l| !l.contains("Falcon")));
}

/// A21.105: Fix, with the page on another machine, opens that machine's Harnesses (and makes it the one Settings shows).
#[test]
fn sends_a_harness_that_isnt_set_up_there_to_that_machines_harnesses() {
    let (mut r, _falcon) = rig_on_another_machine();
    assert_eq!(r.rt.state.target_key(), "m-falcon");
    r.rt.update(|s, tx| s.fix_no_harness(tx));
    r.rt.frame(crate::tests::FRAME);
    assert_eq!(r.rt.state.rail, crate::Rail::Settings);
    assert_eq!(r.rt.state.settings.page, crate::fleet::settings_nav::Page::Harnesses);
    assert_eq!(r.rt.state.machines.current_key(), "m-falcon");
    assert_eq!(r.rt.state.harness_machine(), "m-falcon");
}
