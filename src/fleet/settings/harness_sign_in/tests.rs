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

// ─── lib/__tests__/harness-sign-in.test.ts: the forms ─────────────────────────────────────────────────────────────────

mod forms {
    use super::super::Provider;
    use super::super::flow::*;
    use serde_json::{Value, json};

    fn field(key: &str, extra: Value) -> Field {
        let mut v = json!({"key": key, "type": "string", "required": false, "hidden": false});
        v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        Field::parse(&v).expect("a field")
    }

    /// GitHub Copilot's device sign-in on OpenCode 2.0.9: an enterprise URL only for GitHub Enterprise.
    fn copilot() -> Vec<Field> {
        vec![
            field(
                "deploymentType",
                json!({"title": "Select GitHub deployment type", "required": true, "options": [
                    {"value": "github.com", "label": "GitHub.com"}, {"value": "enterprise", "label": "GitHub Enterprise"}]}),
            ),
            field(
                "enterpriseUrl",
                json!({"title": "Enter your GitHub Enterprise URL or domain", "required": true,
                    "when": [{"key": "deploymentType", "op": "eq", "value": "enterprise"}]}),
            ),
        ]
    }

    fn answers(v: Value) -> Answers {
        v.as_object().unwrap().clone()
    }

    fn keys(fields: Vec<&Field>) -> Vec<&str> {
        fields.into_iter().map(|f| f.key.as_str()).collect()
    }

    #[test]
    fn starts_a_choice_on_its_first_option_and_a_hidden_field_on_its_default() {
        let mut fields = copilot();
        fields.push(field("server", json!({"hidden": true, "default": "https://example.test/console"})));
        assert_eq!(
            Value::Object(initial_answers(&fields)),
            json!({"deploymentType": "github.com", "enterpriseUrl": "", "server": "https://example.test/console"})
        );
        // A tick starts off unless it says otherwise, a many-choice empty, a number empty or at its default, a link not at all.
        let more = [
            field("on", json!({"type": "boolean"})),
            field("yes", json!({"type": "boolean", "default": true})),
            field("many", json!({"type": "multiselect", "default": ["a"]})),
            field("n", json!({"type": "number"})),
            field("m", json!({"type": "integer", "default": 3})),
            field("see", json!({"type": "external", "url": "https://example.test"})),
        ];
        assert_eq!(Value::Object(initial_answers(&more)), json!({"on": false, "yes": true, "many": ["a"], "n": "", "m": 3}));
    }

    #[test]
    fn asks_a_field_only_while_its_condition_holds() {
        let f = copilot();
        assert_eq!(keys(visible_fields(&f, &answers(json!({"deploymentType": "github.com"})))), ["deploymentType"]);
        assert_eq!(keys(visible_fields(&f, &answers(json!({"deploymentType": "enterprise"})))), ["deploymentType", "enterpriseUrl"]);
        let not_a = [field("x", json!({"when": [{"key": "mode", "op": "neq", "value": "a"}]}))];
        assert_eq!(visible_fields(&not_a, &answers(json!({"mode": "b"}))).len(), 1);
        assert_eq!(visible_fields(&not_a, &answers(json!({"mode": "a"}))).len(), 0);
    }

    #[test]
    fn names_the_first_needed_field_without_an_answer() {
        let f = copilot();
        assert_eq!(
            missing_answer(&f, &answers(json!({"deploymentType": "enterprise", "enterpriseUrl": ""}))).as_deref(),
            Some("Enter your GitHub Enterprise URL or domain")
        );
        assert_eq!(missing_answer(&f, &answers(json!({"deploymentType": "github.com", "enterpriseUrl": ""}))), None);
    }

    #[test]
    fn sends_what_was_asked_and_the_hidden_defaults_and_numbers_as_numbers() {
        let mut fields = copilot();
        fields.push(field("server", json!({"hidden": true, "default": "https://example.test/console"})));
        fields.push(field("port", json!({"type": "integer"})));
        let sent =
            answers_to_send(&fields, &answers(json!({"deploymentType": "github.com", "enterpriseUrl": "left over", "port": "8080"})));
        assert_eq!(Value::Object(sent), json!({"deploymentType": "github.com", "server": "https://example.test/console", "port": 8080}));
    }

    #[test]
    fn offers_the_environment_last() {
        let provider = Provider {
            id: "openai".into(),
            name: "OpenAI".into(),
            connections: Vec::new(),
            methods: vec![
                json!({"type": "key", "label": "API key", "fields": []}),
                json!({"type": "env", "label": "Environment variable", "fields": [], "environmentVariables": ["EXAMPLE_API_KEY"]}),
                json!({"type": "oauth", "id": "chatgpt-browser", "label": "ChatGPT (browser)", "fields": []}),
            ],
        };
        let kinds: Vec<String> = methods_to_offer(&provider).into_iter().map(|m| m.kind).collect();
        assert_eq!(kinds, ["key", "oauth", "env"]);
    }

    #[test]
    fn finds_the_code_a_device_sign_in_shows() {
        assert_eq!(code_to_enter("Enter code: ABCD-1234").as_deref(), Some("ABCD-1234"));
        assert_eq!(code_to_enter("Open https://example.test/device on any device and enter code: W1Z2-QQ").as_deref(), Some("W1Z2-QQ"));
        assert_eq!(code_to_enter("Complete authorization in your browser. This window will close automatically."), None);
        assert_eq!(code_to_enter("the code: ab"), None, "too short to be one");
    }

    #[test]
    fn tells_this_computer_from_another_machine() {
        assert!(!is_remote(crate::fleet::settings::HOME_MACHINE_KEY));
        assert!(is_remote("falcon"));
    }

    #[test]
    fn a_number_that_isnt_one_goes_as_null_and_a_tick_or_choice_goes_as_it_is() {
        let fields = [
            field("port", json!({"type": "number"})),
            field("on", json!({"type": "boolean"})),
            field("many", json!({"type": "multiselect"})),
        ];
        let sent = answers_to_send(&fields, &answers(json!({"port": "x", "on": true, "many": ["a", "b"]})));
        assert_eq!(Value::Object(sent), json!({"port": null, "on": true, "many": ["a", "b"]}));
    }
}
