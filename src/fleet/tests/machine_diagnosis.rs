//! `machine-diagnosis.test.ts` (the tunnel case and the text to paste) and `MachineSessionsGroup.test.ts` "says why an
//! unreachable machine isn't answering, from home's last look at it".
use crate::fleet::machine_diagnosis::{Diagnosis, StepState};
use crate::fleet::machines::{Machines, Source, parse_machine_list};
use serde_json::json;

#[test]
fn gives_a_down_tunnel_its_own_headline() {
    let d = Diagnosis::tunnel_down("osprey", Some("alice@osprey"));
    assert_eq!(d.headline, "The SSH tunnel to osprey is down");
    assert!(d.next.contains("ssh alice@osprey"));
    assert_eq!(d.steps[1].state, StepState::Failed);
}

#[test]
fn writes_the_details_as_text_to_paste() {
    let text = Diagnosis::tunnel_down("osprey", None).text();
    assert_eq!(text.lines().next(), Some("The SSH tunnel to osprey is down"));
    assert!(text.contains("- Machine answers: failed (The tunnel Fleet keeps open isn't up.)"));
    assert!(text.contains("- Key accepted: not checked"));
    assert!(text.ends_with("works in a terminal."), "{text}");
    assert!(text.contains("\nTry: Fleet opens it again by itself."));
}

#[test]
fn reads_what_home_says() {
    let said = json!({ "code": "refused", "headline": "macbook isn't answering", "try": "Wake it.",
        "steps": [{ "label": "Name found", "state": "ok" }, { "label": "Machine answers", "state": "failed", "detail": "refused" },
                  { "label": "Port answers", "state": "notChecked" }] });
    let d = Diagnosis::from_json(&said).unwrap();
    assert_eq!((d.code.as_str(), d.next.as_str()), ("refused", "Wake it."));
    assert_eq!(d.steps.iter().map(|s| s.state).collect::<Vec<_>>(), [StepState::Ok, StepState::Failed, StepState::NotChecked]);
    assert_eq!(d.steps[1].detail.as_deref(), Some("refused"));
    assert!(Diagnosis::from_json(&json!({ "headline": "" })).is_none(), "nothing to say is no diagnosis");
}

fn listed(diagnosis: serde_json::Value) -> Machines {
    let body = json!({ "machines": [{ "id": "macbook-id", "name": "macbook", "baseUrl": "http://127.0.0.43:9", "token": "macbook-token-0123456789",
        "addedAt": "2026-09-26T00:00:00Z", "diagnosis": diagnosis }] });
    let mut m = Machines::default();
    m.apply_list(parse_machine_list(200, &body.to_string()));
    m
}

#[test]
fn says_why_an_unreachable_machine_isn_t_answering_from_home_s_last_look_at_it() {
    let look = json!({ "code": "x", "headline": "macbook isn't answering", "try": "Wake it.", "steps": [] });
    let mut m = listed(look);
    assert_eq!(m.why("macbook-id"), None, "it answers: nothing to say");
    m.mark_unreachable("macbook-id", "Can't reach macbook.".into());
    assert_eq!(m.why("macbook-id").map(|d| d.headline.as_str()), Some("macbook isn't answering"));

    let mut none = listed(serde_json::Value::Null);
    none.mark_unreachable("macbook-id", "no answer".into());
    assert_eq!(none.why("macbook-id"), None, "home hasn't said why");
}

#[test]
fn this_app_s_own_check_wins_and_is_kept_only_while_the_error_stands() {
    let mut m = listed(json!({ "headline": "old", "try": "", "steps": [] }));
    assert!(!m.wants_diagnosis("macbook-id"), "it answers");
    m.set_diagnosis("macbook-id", Diagnosis::from_json(&json!({ "headline": "late", "try": "" })));
    assert!(m.diagnoses.is_empty(), "an answer that came after the machine did is dropped");

    m.mark_unreachable("macbook-id", "Can't reach macbook.".into());
    assert!(m.wants_diagnosis("macbook-id"));
    m.set_diagnosis("macbook-id", Diagnosis::from_json(&json!({ "headline": "new", "try": "" })));
    assert_eq!(m.why("macbook-id").map(|d| d.headline.as_str()), Some("new"));
    assert!(!m.wants_diagnosis("macbook-id"), "asked once while the error stands");

    let read = crate::fleet::machines::Read { sessions: vec![], projects: None, identity: None };
    m.apply_read("macbook-id", read, 1);
    assert!(m.diagnoses.is_empty(), "a machine that answers has none");
}

#[test]
fn a_down_tunnel_or_a_device_sign_in_asks_home_nothing() {
    let mut m = listed(serde_json::Value::Null);
    m.mark_unreachable("macbook-id", "x".into());
    m.source = Source::Device;
    assert!(!m.wants_diagnosis("macbook-id"), "only an owner can ask home to look");
}
