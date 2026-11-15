//! Fleet's tests for the choice logic, under their names (the Vue sentence in the comment), and the ones native adds.
use super::choice::*;
use super::effort::*;
use super::format::*;
use super::groups::*;
use std::collections::HashMap;

fn s(items: &[&str]) -> Vec<String> {
    items.iter().map(|i| i.to_string()).collect()
}

fn catalog() -> Catalog {
    Catalog {
        agents: vec![
            CatalogAgent { name: "loom".into(), model: Some(ModelReference::new("github-copilot", "claude-opus-4.7")), hidden: false },
            CatalogAgent { name: "build".into(), model: None, hidden: false },
            CatalogAgent { name: "title".into(), model: None, hidden: true },
        ],
        providers: vec![CatalogProvider {
            id: "github-copilot".into(),
            name: "GitHub Copilot".into(),
            models: vec![("claude-opus-4.7".into(), "Claude Opus 4.7".into()), ("claude-haiku-4.5".into(), "Claude Haiku 4.5".into())],
        }],
        default_agent: Some("loom".into()),
        default_model: None,
    }
}

fn choice(agent: &str, model: &str) -> AgentModelChoice {
    AgentModelChoice { agent: agent.into(), model: model.into() }
}

// agent-model-choice: "turns a model selection key back into the model"
#[test]
fn turns_a_model_selection_key_back_into_the_model() {
    let m = ModelReference::new("openrouter", "anthropic/claude-haiku-4.5");
    let key = key_for_model(&m);
    assert_eq!(key, r#"["openrouter","anthropic/claude-haiku-4.5"]"#);
    assert_eq!(model_from_key(&key), Some(m));
    assert_eq!(model_from_key(""), None);
    assert_eq!(model_from_key("not json"), None);
    assert_eq!(model_from_key(r#"["only-one"]"#), None);
    assert_eq!(model_from_key(r#"["a",""]"#), None);
}

// "writes a model as provider/model and reads it back at the first slash"
#[test]
fn writes_a_model_as_provider_model_and_reads_it_back_at_the_first_slash() {
    let m = ModelReference::new("openrouter", "anthropic/claude-haiku-4.5");
    assert_eq!(model_to_path(Some(&m)).as_deref(), Some("openrouter/anthropic/claude-haiku-4.5"));
    assert_eq!(model_from_path(Some("openrouter/anthropic/claude-haiku-4.5")), Some(m));
    assert_eq!(model_from_path(Some("claude-haiku-4.5")), None);
    assert_eq!(model_from_path(Some("/x")), None);
    assert_eq!(model_from_path(Some("x/")), None);
    assert_eq!(model_from_path(None), None);
}

// "keeps what the catalog offers and drops what it doesn't"
#[test]
fn keeps_what_the_catalog_offers_and_drops_what_it_doesnt() {
    let haiku = key_for_model(&ModelReference::new("github-copilot", "claude-haiku-4.5"));
    let gone = key_for_model(&ModelReference::new("anthropic", "claude-haiku-4-5"));
    assert_eq!(keep_offered(&choice("build", &haiku), &catalog()), choice("build", &haiku));
    assert_eq!(keep_offered(&choice("retired", &gone), &catalog()), choice("", ""));
    assert_eq!(keep_offered(&choice("title", ""), &catalog()), choice("", ""));
}

// "says what Default resolves to"
#[test]
fn says_what_default_resolves_to() {
    let d = describe_defaults(Some(&catalog()), &choice("", ""));
    assert_eq!(d.agent_label, "Default (loom)");
    assert_eq!(d.model_label, "Default (Claude Opus 4.7)");
    assert_eq!(d.model_description, "loom's own model: Claude Opus 4.7");
}

// "says plain Default when only the harness knows the model"
#[test]
fn says_plain_default_when_only_the_harness_knows_the_model() {
    let d = describe_defaults(Some(&catalog()), &choice("build", ""));
    assert_eq!(d.model_label, "Default");
    assert_eq!(d.model_description, "Whatever the harness picks");
}

// "uses the folder's configured model for an agent without one"
#[test]
fn uses_the_folders_configured_model_for_an_agent_without_one() {
    let configured = Catalog { default_model: Some(ModelReference::new("github-copilot", "claude-haiku-4.5")), ..catalog() };
    let d = describe_defaults(Some(&configured), &choice("build", ""));
    assert_eq!(d.model_label, "Default (Claude Haiku 4.5)");
    assert_eq!(d.model_description, "This folder's model: Claude Haiku 4.5");
}

// "says plain Default before the catalog arrives"
#[test]
fn says_plain_default_before_the_catalog_arrives() {
    let d = describe_defaults(None, &choice("", ""));
    assert_eq!((d.agent_label.as_str(), d.model_label.as_str()), ("Default", "Default"));
}

fn session_lists() -> (Vec<AgentOption>, Vec<ModelOption>) {
    let agents = vec![
        AgentOption {
            id: "loom".into(),
            name: "loom".into(),
            model: Some(ModelReference::new("github-copilot", "claude-opus-4.7")),
            ..Default::default()
        },
        AgentOption { id: "build".into(), name: "build".into(), ..Default::default() },
    ];
    let models = vec![
        ModelOption::new("github-copilot", "GitHub Copilot", "claude-opus-4.7", "Claude Opus 4.7"),
        ModelOption::new("github-copilot", "GitHub Copilot", "claude-haiku-4.5", "Claude Haiku 4.5"),
    ];
    (agents, models)
}

// agent-model-choice, in a session: "Default is the agent and model the session was given"
#[test]
fn default_is_the_agent_and_model_the_session_was_given() {
    let (agents, models) = session_lists();
    let own = ModelReference::new("github-copilot", "claude-haiku-4.5");
    let d = describe_session_defaults(&SessionDefaultsInput {
        session_agent: Some("build"),
        session_model: Some(&own),
        default_agent: "loom",
        agents: &agents,
        models: &models,
        ..Default::default()
    });
    assert_eq!(
        d,
        DefaultLabels {
            agent_label: "Default (build)".into(),
            agent_description: "This session's agent: build".into(),
            model_label: "Default (Claude Haiku 4.5)".into(),
            model_description: "This session's model: Claude Haiku 4.5".into(),
        }
    );
}

// "without a choice of its own, Default is the harness's agent and that agent's model"
#[test]
fn without_a_choice_of_its_own_default_is_the_harnesss_agent_and_that_agents_model() {
    let (agents, models) = session_lists();
    let d =
        describe_session_defaults(&SessionDefaultsInput { default_agent: "loom", agents: &agents, models: &models, ..Default::default() });
    assert_eq!(d.agent_label, "Default (loom)");
    assert_eq!(d.agent_description, "Use the session default agent");
    assert_eq!(d.model_label, "Default (Claude Opus 4.7)");
    assert_eq!(d.model_description, "loom's own model: Claude Opus 4.7");
}

// "an agent picked for the next prompt brings its own model"
#[test]
fn an_agent_picked_for_the_next_prompt_brings_its_own_model() {
    let (agents, models) = session_lists();
    let d = describe_session_defaults(&SessionDefaultsInput {
        draft_agent: "build",
        default_agent: "loom",
        agents: &agents,
        models: &models,
        ..Default::default()
    });
    assert_eq!(d.model_label, "Default");
}

// "a session left to the harness's model names the model that answered last"
#[test]
fn a_session_left_to_the_harnesss_model_names_the_model_that_answered_last() {
    let (agents, models) = session_lists();
    let d = describe_session_defaults(&SessionDefaultsInput {
        draft_agent: "build",
        default_agent: "loom",
        agents: &agents,
        models: &models,
        last_model_id: Some("claude-haiku-4.5"),
        ..Default::default()
    });
    assert_eq!(d.model_label, "Default (Claude Haiku 4.5)");
    assert_eq!(d.model_description, "The model that answered last: Claude Haiku 4.5");
}

fn key_models() -> Vec<ModelOption> {
    vec![
        ModelOption::new("p", "P", "claude-opus-4.7", "Claude Opus 4.7"),
        ModelOption::new("p", "P", "claude-haiku-4.5", "Claude Haiku 4.5"),
    ]
}

// sessionDefaultModelKey: "follows the model that answered last, not the catalog's first entry"
#[test]
fn follows_the_model_that_answered_last_not_the_catalogs_first_entry() {
    let models = key_models();
    assert_eq!(session_default_model_key(None, None, Some("claude-haiku-4.5"), &models), Some(models[1].selection_key.clone()));
}

// "prefers the session's own model, then the agent's"
#[test]
fn prefers_the_sessions_own_model_then_the_agents() {
    let models = key_models();
    let own = ModelReference::new("p", "claude-haiku-4.5");
    let agents_own = ModelReference::new("p", "claude-opus-4.7");
    assert_eq!(session_default_model_key(Some(&own), Some(&agents_own), Some("x"), &models), Some(models[1].selection_key.clone()));
    assert_eq!(session_default_model_key(None, Some(&agents_own), None, &models), Some(models[0].selection_key.clone()));
}

// "is null when only the harness knows"
#[test]
fn is_null_when_only_the_harness_knows() {
    assert_eq!(session_default_model_key(None, None, None, &key_models()), None);
    // Native adds: an answer from a model the list doesn't have is also not guessed.
    assert_eq!(session_default_model_key(None, None, Some("unlisted"), &key_models()), None);
}

// plainDefaultLabel: "drops the Default wrapper and keeps everything else"
#[test]
fn drops_the_default_wrapper_and_keeps_everything_else() {
    assert_eq!(plain_default_label("Default (DeepSeek V4.1 Flash Free)"), "DeepSeek V4.1 Flash Free");
    assert_eq!(plain_default_label("Default (build)"), "build");
    assert_eq!(plain_default_label("Default"), "Default");
    assert_eq!(plain_default_label("Opus 5.5"), "Opus 5.5");
}

// effort-memory: "keys one level per harness and model, in one URL segment"
#[test]
fn keys_one_level_per_harness_and_model_in_one_url_segment() {
    assert_eq!(effort_preference_key(Some("opencode2"), "anthropic/opus-5.5"), "effort.opencode2.anthropic:opus-5.5");
    assert_eq!(
        effort_preference_key(Some("claude-code"), r#"["anthropic","claude-opus-5-5"]"#),
        "effort.claude-code.anthropic:claude-opus-5-5"
    );
    assert_eq!(effort_preference_key(None, "m"), "effort.unknown.m");
}

// "returns the remembered level only while the model still offers it"
#[test]
fn returns_the_remembered_level_only_while_the_model_still_offers_it() {
    let levels = s(&["low", "medium", "high"]);
    assert_eq!(remembered_effort(Some("high"), &levels), "high");
    assert_eq!(remembered_effort(Some("xhigh"), &levels), "");
    assert_eq!(remembered_effort(Some(""), &s(&["low"])), "");
    assert_eq!(remembered_effort(None, &s(&["low"])), "");
}

// "names an unset level Default"
#[test]
fn names_an_unset_level_default() {
    assert_eq!(effort_label(""), "Default");
    assert_eq!(effort_label("xhigh"), "Xhigh");
}

// effortToSend: "sends the explicit model default for Default on a model with levels, nothing on one without, the pick otherwise"
#[test]
fn sends_the_explicit_model_default_for_default_on_a_model_with_levels_nothing_on_one_without_the_pick_otherwise() {
    assert_eq!(effort_to_send("", Some(&s(&["low", "high"]))).as_deref(), Some(MODEL_DEFAULT_EFFORT));
    assert_eq!(effort_to_send("", Some(&[])), None);
    assert_eq!(effort_to_send("", None), None);
    assert_eq!(effort_to_send("high", Some(&s(&["low", "high"]))).as_deref(), Some("high"));
}

// the toggle's click (row 4.3): "low, medium, high, xhigh, Default, low"
#[test]
fn a_click_cycles_the_levels_then_default_and_wraps() {
    let levels = s(&["low", "medium", "high", "xhigh"]);
    let mut at = "low".to_string();
    let mut seen = vec![at.clone()];
    for _ in 0..5 {
        at = next_effort(&at, &levels);
        seen.push(at.clone());
    }
    assert_eq!(seen, ["low", "medium", "high", "xhigh", "", "low"]);
    // A level the model doesn't offer starts the cycle again.
    assert_eq!(next_effort("max", &levels), "low");
}

// use-effort-memory: the logic as a state machine
struct Rig {
    draft: String,
    model: String,
    variants: Vec<String>,
    prefs: HashMap<String, String>,
    memory: EffortMemory,
    fetched: bool,
}

impl Rig {
    fn new(stored: &[(&str, &str)], variants: &[&str]) -> Self {
        let mut rig = Self {
            draft: String::new(),
            model: "anthropic/opus".into(),
            variants: s(variants),
            prefs: stored.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
            memory: EffortMemory::new("anthropic/opus"),
            fetched: true,
        };
        rig.run(0.0);
        rig
    }
    fn run(&mut self, now: f64) {
        let name = if self.model == "anthropic/opus" { "Opus 5.5" } else { "Haiku" };
        let input = Input {
            fetched: self.fetched,
            harness_type: Some("opencode2"),
            model_key: &self.model,
            model_name: name,
            picked: &self.model,
            variants: &self.variants,
        };
        self.memory.update(&input, &mut self.draft, &self.prefs, now);
    }
    fn pick(&mut self, level: &str) -> (String, String) {
        let name = if self.model == "anthropic/opus" { "Opus 5.5" } else { "Haiku" };
        let input = Input {
            fetched: true,
            harness_type: Some("opencode2"),
            model_key: &self.model,
            model_name: name,
            picked: &self.model,
            variants: &self.variants,
        };
        self.memory.pick(&input, level, &mut self.draft, &mut self.prefs)
    }
}

const OPUS: &str = "effort.opencode2.anthropic:opus";
const LEVELS: [&str; 3] = ["low", "medium", "high"];

// "opens a draft on the remembered level, else Default"
#[test]
fn opens_a_draft_on_the_remembered_level_else_default() {
    assert_eq!(Rig::new(&[(OPUS, "high")], &LEVELS).draft, "high");
    assert_eq!(Rig::new(&[], &LEVELS).draft, "");
}

// "takes the remembered level once the model's levels load, when its key was known first"
#[test]
fn takes_the_remembered_level_once_the_models_levels_load_when_its_key_was_known_first() {
    let mut rig = Rig::new(&[(OPUS, "high")], &[]);
    assert_eq!(rig.draft, "");
    rig.variants = s(&LEVELS);
    rig.run(1.0);
    assert_eq!(rig.draft, "high");
}

// "ignores a remembered level the model no longer offers, without deleting it"
#[test]
fn ignores_a_remembered_level_the_model_no_longer_offers_without_deleting_it() {
    let rig = Rig::new(&[(OPUS, "max")], &LEVELS);
    assert_eq!(rig.draft, "");
    assert_eq!(rig.prefs.get(OPUS).map(String::as_str), Some("max"));
}

// "brings each model's level back on a switch and says so; a model without one resets to Default"
#[test]
fn brings_each_models_level_back_on_a_switch_and_says_so_a_model_without_one_resets_to_default() {
    let mut rig = Rig::new(&[(OPUS, "high"), ("effort.opencode2.anthropic:haiku", "low")], &LEVELS);
    rig.model = "anthropic/haiku".into();
    rig.run(10.0);
    assert_eq!(rig.draft, "low");
    assert_eq!(rig.memory.note(10.0), Some("Haiku: back on Low"));
    // The note times out after four seconds.
    assert_eq!(rig.memory.note(13.9), Some("Haiku: back on Low"));
    assert_eq!(rig.memory.note(14.0), None);
    rig.model = "anthropic/other".into();
    rig.run(11.0);
    assert_eq!(rig.draft, "");
}

// "writes only a composer pick; Default stores an empty value"
#[test]
fn writes_only_a_composer_pick_default_stores_an_empty_value() {
    let mut rig = Rig::new(&[], &LEVELS);
    // Opening the draft and running again wrote nothing.
    assert!(rig.prefs.is_empty());
    assert_eq!(rig.pick("medium"), (OPUS.to_string(), "medium".to_string()));
    assert_eq!(rig.draft, "medium");
    assert_eq!(rig.prefs.get(OPUS).map(String::as_str), Some("medium"));
    assert_eq!(rig.pick(""), (OPUS.to_string(), String::new()));
}

// Native adds: a draft that already holds a level keeps it on first run, and nothing runs before the preferences load.
#[test]
fn a_level_the_draft_already_has_survives_the_first_run_and_nothing_applies_before_the_preferences_load() {
    let mut memory = EffortMemory::new("");
    let mut draft = "low".to_string();
    let prefs: HashMap<String, String> = [(OPUS.to_string(), "high".to_string())].into();
    let levels = s(&LEVELS);
    let input =
        Input { fetched: false, harness_type: Some("opencode2"), model_key: "anthropic/opus", variants: &levels, ..Default::default() };
    memory.update(&input, &mut draft, &prefs, 0.0);
    assert_eq!(draft, "low");
    let input = Input { fetched: true, ..input };
    memory.update(&input, &mut draft, &prefs, 0.0);
    assert_eq!(draft, "low");
}

fn m(provider: &str, id: &str) -> ModelOption {
    ModelOption::new(provider, &provider.to_uppercase(), id, id)
}
fn key(provider: &str, id: &str, harness: &str) -> String {
    model_picker_key(harness, provider, id)
}
fn ids(items: &[ModelOption]) -> Vec<&str> {
    items.iter().map(|i| i.id.as_str()).collect()
}
fn titles(sections: &[ModelSection]) -> Vec<Option<&str>> {
    sections.iter().map(|s| s.title.as_deref()).collect()
}

fn few() -> Vec<ModelOption> {
    vec![m("a", "m1"), m("a", "m2"), m("b", "m3")]
}
fn many() -> Vec<ModelOption> {
    (0..6).map(|i| m("a", &format!("a{i}"))).chain((0..4).map(|i| m("b", &format!("b{i}")))).collect()
}

// model-picker-groups: "looks like today's list when nothing is starred or recent"
#[test]
fn looks_like_todays_list_when_nothing_is_starred_or_recent() {
    assert_eq!(group_models(&few(), "opencode", &[], &[], true), vec![ModelSection { title: None, level: Level::Group, items: few() }]);
}

// "lists favourites in starred order and keeps them in All models"
#[test]
fn lists_favourites_in_starred_order_and_keeps_them_in_all_models() {
    let sections = group_models(&few(), "opencode", &[key("b", "m3", "opencode"), key("a", "m1", "opencode")], &[], true);
    assert_eq!(titles(&sections), [Some("Favourites"), Some("All models")]);
    assert_eq!(ids(&sections[0].items), ["m3", "m1"]);
    assert_eq!(ids(&sections[1].items), ["m1", "m2", "m3"]);
}

// "ignores keys the list doesn't have"
#[test]
fn ignores_keys_the_list_doesnt_have() {
    let sections = group_models(&few(), "opencode", &[key("gone", "x", "opencode")], &[key("gone", "y", "opencode")], true);
    assert_eq!(sections.len(), 1);
}

// "lists this harness's recent minus favourites, at most five"
#[test]
fn lists_this_harnesss_recent_minus_favourites_at_most_five() {
    let list: Vec<ModelOption> = (0..8).map(|i| m("a", &format!("m{i}"))).collect();
    let mut recent: Vec<String> = list.iter().map(|x| key("a", &x.id, "opencode")).collect();
    recent.push(key("a", "m0", "claude"));
    let sections = group_models(&list, "opencode", &[key("a", "m0", "opencode")], &recent, true);
    let shown = sections.iter().find(|s| s.title.as_deref() == Some("Recent")).unwrap();
    assert_eq!(ids(&shown.items), ["m1", "m2", "m3", "m4", "m5"]);
    assert_eq!(group_models(&list, "claude", &[], &recent, true)[0].title.as_deref(), Some("Recent"));
}

// "leaves Recent out when asked"
#[test]
fn leaves_recent_out_when_asked() {
    assert_eq!(titles(&group_models(&few(), "opencode", &[], &[key("a", "m1", "opencode")], false)), [None]);
}

// "splits by provider only past eight models from several providers"
#[test]
fn splits_by_provider_only_past_eight_models_from_several_providers() {
    let sections = group_models(&many(), "opencode", &[], &[], true);
    assert_eq!(
        sections.iter().map(|s| (s.title.as_deref(), s.level)).collect::<Vec<_>>(),
        [(Some("A"), Level::Provider), (Some("B"), Level::Provider)]
    );
    let one: Vec<ModelOption> = many().into_iter().filter(|x| x.provider == "A").collect();
    assert_eq!(group_models(&one, "opencode", &[], &[], true).len(), 1);
}

// "puts All models above the provider headers when something is above it"
#[test]
fn puts_all_models_above_the_provider_headers_when_something_is_above_it() {
    let sections = group_models(&many(), "opencode", &[key("b", "b1", "opencode")], &[], true);
    assert_eq!(titles(&sections), [Some("Favourites"), Some("All models"), Some("A"), Some("B")]);
}

// stores/model-picker.ts (native adds): the preference's JSON, toggle and recent
#[test]
fn the_preference_reads_what_is_there_and_keeps_hidden_and_stale_keys() {
    let p = ModelPicker::parse(r#"{"favourites":["x:a/b",3],"hidden":["h"],"recent":"nope"}"#);
    assert_eq!((p.favourites.clone(), p.hidden.clone(), p.recent.clone()), (s(&["x:a/b"]), s(&["h"]), vec![]));
    assert_eq!(ModelPicker::parse("not json"), ModelPicker::default());
    let next = p.toggle_favourite("x:c/d");
    assert_eq!(next.favourites, s(&["x:a/b", "x:c/d"]));
    assert_eq!(next.hidden, s(&["h"]));
    assert_eq!(next.toggle_favourite("x:a/b").favourites, s(&["x:c/d"]));
    assert_eq!(ModelPicker::parse(&next.to_json()), next);
    assert!(next.is_favourite("x:c/d"));
}

#[test]
fn a_model_sent_with_moves_to_the_front_once_five_per_harness_and_others_keep_their_place() {
    let mut p = ModelPicker { recent: s(&["o:a/1", "c:a/9", "o:a/2", "o:a/3", "o:a/4", "o:a/5"]), ..Default::default() };
    p = p.record_recent("o:a/3");
    assert_eq!(p.recent, s(&["o:a/3", "o:a/1", "o:a/2", "o:a/4", "o:a/5", "c:a/9"]));
    p = p.record_recent("o:a/6");
    assert_eq!(p.recent, s(&["o:a/6", "o:a/3", "o:a/1", "o:a/2", "o:a/4", "c:a/9"]));
}

// format/numbers: "writes counts the way the meter shows them"
#[test]
fn writes_counts_the_way_the_meter_shows_them() {
    let t = |n: f64| format_tokens(Some(n));
    assert_eq!(t(950.0), "950");
    assert_eq!(t(17_148.0), "17.1k");
    assert_eq!(t(200_000.0), "200k");
    assert_eq!(t(1_000_000.0), "1M");
    assert_eq!(t(1_500_000.0), "1.5M");
}

// Fleet's numbers.test.ts, in en-US
#[test]
fn tokens_round_and_move_up_a_unit_as_fleets_do() {
    let cases = [
        (0.0, "0"),
        (999.0, "999"),
        (999.6, "1k"),
        (1_000.0, "1k"),
        (1_200.0, "1.2k"),
        (1_250.0, "1.3k"),
        (99_949.0, "99.9k"),
        (99_950.0, "100k"),
        (999_499.0, "999k"),
        (999_500.0, "1M"),
        (3_400_000.0, "3.4M"),
        (125_000_000.0, "125M"),
        (999_500_000.0, "1B"),
        (3_400_000_000.0, "3.4B"),
        (-1_200.0, "-1.2k"),
    ];
    for (n, want) in cases {
        assert_eq!(format_tokens(Some(n)), want, "{n}");
    }
    assert_eq!(format_tokens(Some(f64::NAN)), "");
    assert_eq!(format_tokens(None), "");
}

#[test]
fn counts_group_and_round_to_a_whole_number() {
    assert_eq!(format_count(Some(0.0)), "0");
    assert_eq!(format_count(Some(12_500.0)), "12,500");
    assert_eq!(format_count(Some(2.6)), "3");
    assert_eq!(format_count(Some(f64::INFINITY)), "");
    assert_eq!(format_count_digits(Some(1.25), 2), "1.25");
    assert_eq!(format_count_digits(Some(1234.5), 2), "1,234.5");
}

#[test]
fn money_has_two_decimals_and_a_floor_for_cheap_calls() {
    assert_eq!(format_money(Some(0.0)), "$0.00");
    assert_eq!(format_money(Some(1.5)), "$1.50");
    assert_eq!(format_money(Some(1234.5)), "$1,234.50");
    assert_eq!(format_money(Some(0.004)), "<$0.01");
    assert_eq!(format_money(Some(0.005)), "$0.01");
    assert_eq!(format_money(None), "");
}

// attached-files: "formats sizes"
#[test]
fn formats_sizes() {
    let b = |n: f64| format_bytes(Some(n));
    assert_eq!(b(900.0), "900 B");
    assert_eq!(b(188_416.0), "184 KB");
    assert_eq!(b(1.5 * 1024.0 * 1024.0), "1.5 MB");
    assert_eq!(b(0.0), "0 B");
    assert_eq!(b(1024.0), "1 KB");
    assert_eq!(b(1024.0 * 1024.0), "1.0 MB");
    assert_eq!(b(340.0 * 1024.0 * 1024.0), "340 MB");
    assert_eq!(b(2.1 * 1024.0 * 1024.0 * 1024.0), "2.1 GB");
    assert_eq!(format_bytes(Some(f64::NAN)), "");
}

// ModelSelector.vue (row 3.14, native adds): HydraFusion says what it costs, under its own row and under a Default that resolves to it
#[test]
fn hydrafusion_replaces_the_description_with_the_cost_note() {
    assert_eq!(model_description("copilot-HydraFusion-v2", "x"), HYDRAFUSION_NOTE);
    assert_eq!(model_description("claude-opus-4.7", "x"), "x");
    assert_eq!(default_model_description("Default (HydraFusion)", "d"), HYDRAFUSION_NOTE);
    assert_eq!(default_model_description("Default (Opus)", "d"), "d");
}
