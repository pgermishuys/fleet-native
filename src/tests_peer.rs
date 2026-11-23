//! Component 26, piece N: the chip on a message another session sent (`ActivityStream.peer-machine.test.ts` under its
//! names), and what rides with it in the same piece (a folded turn's counts, a screenshot of another machine).
use crate::tests::{FRAME, app, click};
use rune::Runtime;

fn scene() -> Runtime<crate::State> {
    let mut rt = app();
    rt.update(|s, _| s.show_peer_messages());
    for _ in 0..4 {
        rt.frame(FRAME);
    }
    rt
}

fn has(rt: &Runtime<crate::State>, key: &str) -> bool {
    rt.find(key).is_some()
}

fn words(rt: &Runtime<crate::State>, key: &str) -> String {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).map(|e| e.text.unwrap_or_default()).unwrap_or_default()
}

#[test]
fn draws_the_chip_with_the_sender_its_machine_and_how_an_update_ended() {
    let rt = scene();
    for chip in ["peer-from-u1", "peer-from-u2", "peer-from-u3"] {
        assert!(has(&rt, chip), "{chip} is drawn");
    }
    assert_eq!(words(&rt, "peer-from-machine-u1"), "\u{b7} falcon");
    assert!(!has(&rt, "peer-from-machine-u2"), "a sender on this machine names none");
    // The sender's words are the bubble's, not the tag.
    assert!(rt.state.session().messages[0].text().starts_with("The installer test fails"));
}

#[test]
fn opens_a_sender_on_another_machine_in_place() {
    let mut rt = scene();
    click(&mut rt, "peer-from-u1");
    assert_eq!(rt.state.open_id().as_deref(), Some("ses_peer1"));
    assert_eq!(rt.state.machine_key_of("ses_peer1"), "m-falcon");
}

#[test]
fn finds_a_sender_with_no_machine_of_its_own_on_this_sessions_machine_opened_in_place() {
    let mut rt = scene();
    // Open a session that lives on falcon, then press the chip of a sender that names no machine.
    click(&mut rt, "peer-from-u1");
    assert_eq!(rt.state.machine_key_of("ses_peer1"), "m-falcon");
    let sender = crate::conv::peer::PeerSender { session_id: "ses_peer2".into(), title: "Update the docs".into(), ..Default::default() };
    assert_eq!(rt.state.peer_machine_key(&sender).as_deref(), Some("m-falcon"));
    let mut tx = rune::Tx::default();
    rt.update(|s, _| s.open_peer(&sender, &mut tx));
    assert_eq!(rt.state.open_id().as_deref(), Some("ses_peer2"));
    assert_eq!(rt.state.machine_key_of("ses_peer2"), "m-falcon");
}

#[test]
fn names_a_sender_on_a_machine_that_isnt_listed_and_does_not_open_it() {
    let mut rt = scene();
    let before = rt.state.open_id();
    click(&mut rt, "peer-from-u3");
    assert_eq!(rt.state.open_id(), before, "the machine isn't in the list, so the chip is no link");
}

// Row 2.18: a screenshot of a session on another machine is fetched from that machine, with its token.
#[test]
fn a_picture_is_fetched_through_the_machine_of_the_session_it_belongs_to() {
    use crate::fleet::images::session_of_path;
    assert_eq!(session_of_path("/api/sessions/ses_9/screenshots/shot_1"), Some("ses_9"));
    assert_eq!(session_of_path("/api/sessions/ses_9/agent-browser/tabs/t1/frame?n=3"), Some("ses_9"));
    assert_eq!(session_of_path("/api/machine"), None);
    assert_eq!(session_of_path("/api/sessions//x"), None);
}

// Row 2.23: only so many decoded pictures are kept; the ones asked for longest ago go first.
#[test]
fn decoded_pictures_are_let_go_oldest_first() {
    let mut rt = app();
    let png = include_bytes!("../assets/weave_logo.png");
    rt.update(|s, _| {
        for n in 0..70 {
            s.images.want(&format!("k{n}"), crate::fleet::images::Source::Data(String::new()));
            s.supply_image(&format!("k{n}"), png);
        }
        s.images.want("k0", crate::fleet::images::Source::Data(String::new()));
        s.keep_images(&mut rune::Tx::default());
    });
    assert!(rt.state.images.ready("k0").is_some(), "the one asked for again stays");
    assert!(rt.state.images.ready("k1").is_none(), "the oldest unasked one went");
    assert!(rt.state.images.ready("k69").is_some());
}

// Row 2.11: the line "<machine> isn't answering" comes from the machines list, not from a scene.
#[test]
fn says_which_machine_isnt_answering_when_the_open_sessions_machine_stops() {
    let mut rt = app();
    rt.update(|s, _| s.show_machines_only(crate::mock::machines::Case::Down));
    let down = rt.state.machines.list.iter().find(|m| !rt.state.machines.is_answering(&m.id)).map(|m| (m.id.clone(), m.name.clone()));
    let (key, name) = down.expect("the scene has a machine that isn't answering");
    rt.update(|s, _| {
        s.machines.session_machines.insert("ses_far".into(), key.clone());
    });
    assert_eq!(rt.state.machine_key_of("ses_far"), key);
    let mut tx = rune::Tx::default();
    rt.update(|s, _| {
        let session = crate::mock::machines::listed("ses_far", "Far away", crate::mock::Status::Idle, "1m", None, |_| {});
        s.store.upsert_elsewhere(&key, session);
        s.go(crate::fleet::store::Route::Session("ses_far".into()));
        s.after_open(&mut tx);
    });
    assert_eq!(rt.state.not_answering_machine().as_deref(), Some(name.as_str()));
}
