//! `lib/harness-sign-in.ts`'s summary, the search and the order, and the panel's own rules, with no server.
use super::*;
use serde_json::json;

fn list() -> SignIns {
    SignIns::parse(
        &json!({"note": "", "providers": [
            {"id": "anthropic", "name": "Anthropic", "methods": [], "connections": [
                {"kind": "credential", "id": "c1", "label": "Work", "active": true}]},
            {"id": "azure", "name": "Azure OpenAI", "methods": [{"type": "key"}], "connections": []},
            {"id": "gh-models", "name": "Models", "methods": [], "connections": [
                {"kind": "env", "id": "EXAMPLE_TOKEN", "label": "EXAMPLE_TOKEN", "active": true}]},
            {"id": "openai", "name": "OpenAI", "methods": [], "connections": []},
        ]})
        .to_string(),
    )
    .expect("a list")
}

fn provider(connections: Vec<Connection>) -> Provider {
    Provider { id: "p".into(), name: "P".into(), methods: Vec::new(), connections }
}

fn connection(kind: &str, id: &str, active: bool) -> Connection {
    Connection { kind: kind.into(), id: id.into(), label: id.into(), active }
}

#[test]
fn says_which_sign_in_a_provider_uses() {
    assert_eq!(provider(vec![connection("credential", "c", true)]).summary().as_deref(), Some("Signed in"));
    assert_eq!(provider(vec![connection("env", "EXAMPLE_KEY", true)]).summary().as_deref(), Some("From EXAMPLE_KEY"));
    assert_eq!(provider(vec![connection("credential", "c", false)]).summary(), None, "signed in, but none in use");
    assert_eq!(provider(vec![]).summary(), None);
}

#[test]
fn reads_the_providers_in_the_order_the_harness_lists_them_and_an_empty_note_as_none() {
    let l = list();
    assert_eq!(l.providers.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), ["anthropic", "azure", "gh-models", "openai"]);
    assert_eq!(l.note, None);
    assert_eq!(l.signed_in().iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), ["anthropic", "gh-models"]);
    assert_eq!(l.providers[0].connections[0], connection("credential", "c1", true).with_label("Work"));
    assert!(SignIns::parse("{}").is_none() && SignIns::parse("nope").is_none());
}

impl Connection {
    fn with_label(mut self, label: &str) -> Connection {
        self.label = label.into();
        self
    }
}

#[test]
fn searches_the_providers_still_to_sign_in_to_by_name_or_id_whatever_the_case() {
    let l = list();
    let ids = |q: &str| l.others(q).iter().map(|p| p.id.clone()).collect::<Vec<_>>();
    assert_eq!(ids(""), ["azure", "openai"]);
    assert_eq!(ids("  "), ["azure", "openai"]);
    assert_eq!(ids("OPEN"), ["azure", "openai"], "Azure OpenAI by its name, OpenAI by both");
    assert_eq!(ids("azure"), ["azure"]);
    assert_eq!(ids("ope"), ["azure", "openai"]);
    assert_eq!(ids("anthropic"), Vec::<String>::new(), "a signed-in provider isn't offered again");
}

#[test]
fn one_provider_is_a_single_harness() {
    assert!(SignIns { providers: vec![provider(vec![])], note: None }.single());
    assert!(!list().single());
    assert!(!SignIns::default().single());
}

#[test]
fn the_server_s_words_come_first() {
    assert_eq!(error_words(500, r#"{"error":"Locked."}"#), "Locked.");
    assert_eq!(error_words(500, r#"{"detail":"Slow down."}"#), "Slow down.");
    assert_eq!(error_words(502, "not json"), "Fleet couldn't reach the harness (HTTP 502).");
    assert_eq!(error_words(400, r#"{"error":""}"#), "Fleet couldn't reach the harness (HTTP 400).");
}

#[test]
fn a_finished_sign_in_says_new_sessions_can_use_the_models() {
    assert_eq!(done_words("Signed in to Azure."), "Signed in to Azure. New sessions can use its models.");
}

#[test]
fn only_a_credential_can_be_switched_to_or_signed_out_of() {
    assert!(connection("credential", "c", false).is_credential());
    assert!(!connection("env", "E", false).is_credential());
}
