//! The pickers' data and the pick (docs/parity/05-composer.md, piece D). Ports `AgentModelSelectors.test.ts` (the
//! chips' names), `EffortToggle.test.ts` and the model and provider selection of `Composer.test.ts`; the rest are new:
//! the lists load on opening a session and again on a catalog event, a model that disappears falls back to Default
//! once, and the prompt body carries the pick (read from a fake server).
use super::super::catalog::{parse_agents, parse_harnesses, parse_models};
use super::super::events::{Event, Kind};
use super::super::http::{Pick, prompt_body};
use super::super::link::{Job, Link, Post};
use super::super::prefs::Prefs;
use super::super::state::Topic;
use super::reconnect::app;
use super::*;
use crate::State;
use crate::composer::choice::{ModelOption, create_model_selection_key};
use crate::composer::drafts::Choice;
use rune::Runtime;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const FRAME: f64 = 1.0 / 60.0;

fn click(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element keyed {key:?}"));
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(FRAME);
}

/// A server's session: the app's list from the fixture, its first session open, with a link that goes nowhere.
fn on_server() -> (Runtime<State>, String) {
    let mut rt = app();
    let post = Post::new(|_| {});
    rt.update(move |s, tx| {
        s.link = Some(Link::start("http://127.0.0.1:9".into(), post));
        s.on_list(sessions::parse_list(&serde_json::from_str(&fixture("api-sessions.json")).unwrap(), 0), tx);
    });
    let id = rt.state.open_id().unwrap();
    rt.frame(FRAME);
    (rt, id)
}

fn model(provider: &str, id: &str, name: &str, variants: &[&str]) -> ModelOption {
    let mut m = ModelOption::new(provider, provider, id, name);
    m.variants = variants.iter().map(|v| v.to_string()).collect();
    m
}

// ─── the wire shapes ────────────────────────────────────────────────────────

#[test]
fn agents_come_as_a_list_or_wrapped_and_hidden_ones_are_dropped() {
    let wrapped = json!({"agents": [
        {"name": "build", "description": "Edits files", "mode": "primary", "model": {"providerID": "p", "modelID": "m"}},
        {"name": "summary", "mode": "subagent", "hidden": true},
        {"name": "plan", "mode": "primary"},
    ]});
    let agents = parse_agents(&wrapped);
    assert_eq!(agents.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(), ["build", "plan"]);
    assert_eq!(agents[0].description, "Edits files");
    assert_eq!(agents[0].model.as_ref().map(|m| m.model_id.as_str()), Some("m"));
    assert_eq!(parse_agents(&wrapped["agents"]).len(), 2, "a bare list reads the same");
}

#[test]
fn models_come_as_a_list_of_providers_or_wrapped() {
    let providers = json!([{"id": "anthropic", "name": "Anthropic", "models": [
        {"id": "opus", "name": "Opus", "variants": ["low", "high"], "priceTier": "high", "capability": "powerful", "contextWindow": 200000},
        {"id": "haiku", "name": "Haiku"},
    ]}]);
    let models = parse_models(&providers);
    assert_eq!(models.len(), 2);
    assert_eq!(models[0].selection_key, create_model_selection_key("anthropic", "opus"));
    assert_eq!((models[0].variants.len(), models[0].price_tier.as_deref(), models[0].context_window), (2, Some("high"), Some(200_000)));
    assert!(models[1].variants.is_empty());
    assert_eq!(parse_models(&json!({ "providers": providers })).len(), 2);
}

#[test]
fn harnesses_say_what_they_can_do() {
    let list = parse_harnesses(&json!([
        {"type": "opencode2", "displayName": "OpenCode 2", "available": true, "capabilities": {"supportsSteering": true, "steersByDefault": true, "steerDelivery": "immediate", "supportsImageAttachments": true}},
        {"type": "claude-code", "capabilities": {}},
    ]));
    assert_eq!(list.len(), 2);
    assert!(list[0].supports_steering && list[0].steers_by_default && list[0].supports_image_attachments);
    assert!(!list[1].supports_steering);
}

#[test]
fn preferences_keep_text_and_a_refused_write_puts_the_old_value_back() {
    let mut prefs = Prefs::default();
    prefs.adopt(&json!({"effort.x.y": "high", "n": 3}));
    assert!(prefs.fetched);
    assert_eq!((prefs.get("effort.x.y"), prefs.get("n")), (Some("high"), Some("3")));
    let before = prefs.set("effort.x.y", "low");
    prefs.roll_back("effort.x.y", "low", before);
    assert_eq!(prefs.get("effort.x.y"), Some("high"));
    // A newer write is not undone by an older refusal.
    let before = prefs.set("k", "a");
    prefs.set("k", "b");
    prefs.roll_back("k", "a", before);
    assert_eq!(prefs.get("k"), Some("b"));
}

// ─── AgentModelSelectors.test.ts ────────────────────────────────────────────

#[test]
fn the_chips_name_a_listed_pick_and_default_as_before() {
    let mut rt = app();
    rt.update(|s, tx| s.pick_agent("plan", tx));
    assert_eq!(rt.state.agent_chip().label, "plan");
    rt.update(|s, tx| s.pick_agent("", tx));
    assert_eq!(rt.state.agent_chip().label, "Default (build)");
    let key = rt.state.lists().models[0].selection_key.clone();
    rt.update(move |s, tx| s.pick_model(&key, tx));
    assert_eq!(rt.state.model_chip().label, "Sonnet 5.5");
}

#[test]
fn the_chips_still_name_a_pick_the_harness_no_longer_lists() {
    let mut rt = app();
    rt.update(|s, tx| {
        s.pick_agent("removed-agent", tx);
        s.pick_model(&create_model_selection_key("gone", "gone-model"), tx);
    });
    assert_eq!(rt.state.agent_chip().label, "removed-agent");
    assert_eq!(rt.state.model_chip().label, "gone-model");
}

#[test]
fn default_names_what_it_resolves_to() {
    let mut rt = app();
    assert_eq!(rt.state.model_chip().label, "Default (Sonnet 5.5)", "the agent's own model");
    rt.update(|s, tx| s.pick_agent("plan", tx));
    assert_eq!(rt.state.model_chip().label, "Default (Opus 5.5)", "an agent picked brings its own model");
}

#[test]
fn a_narrow_composer_says_the_plain_name_and_keeps_the_whole_one_for_the_tooltip() {
    let mut rt = app();
    rt.update(|s, _| s.pickers.composer_w = 700.0);
    assert_eq!(rt.state.agent_chip().label, "Default (build)");
    rt.update(|s, _| s.pickers.composer_w = 500.0);
    let chip = rt.state.agent_chip();
    assert_eq!((chip.label.as_str(), chip.help.as_deref()), ("build", Some("Default (build)")));
    rt.update(|s, tx| s.pick_agent("plan", tx));
    assert_eq!(rt.state.agent_chip().help, None, "nothing was shortened");
}

// ─── model and provider selection (Composer.test.ts) ────────────────────────

#[test]
fn picking_from_the_menus_sets_the_drafts_choice_and_the_menu_closes() {
    let mut rt = app();
    click(&mut rt, "model");
    click(&mut rt, "model-pop-item2");
    assert_eq!(rt.state.choice().model, create_model_selection_key("anthropic", "opus-5-5"));
    assert_eq!(rt.state.model_chip().label, "Opus 5.5");
    assert!(rt.state.ui.open.is_none());
    click(&mut rt, "model");
    click(&mut rt, "model-pop-item0");
    assert_eq!(rt.state.choice().model, "", "Default is the empty key");
}

#[test]
fn the_pick_belongs_to_its_session() {
    let mut rt = app();
    rt.update(|s, tx| s.pick_agent("plan", tx));
    rt.update(|s, tx| s.open_session(0, 1, tx));
    assert_eq!(rt.state.choice().agent, "");
    rt.update(|s, tx| s.open_session(0, 0, tx));
    assert_eq!(rt.state.choice().agent, "plan");
}

// ─── EffortToggle.test.ts ───────────────────────────────────────────────────

fn opus(rt: &mut Runtime<State>) {
    let key = create_model_selection_key("anthropic", "opus-5-5");
    rt.update(move |s, tx| s.pick_model(&key, tx));
    rt.frame(FRAME);
}

#[test]
fn cycles_the_models_levels_then_default_then_back() {
    let mut rt = app();
    opus(&mut rt);
    rt.update(|s, _| {
        let owner = s.draft_owner().unwrap();
        let c = s.drafts.choice(&owner);
        s.drafts.set_choice(&owner, Choice { effort: "low".into(), ..c });
    });
    rt.frame(FRAME);
    let mut seen = Vec::new();
    for _ in 0..5 {
        click(&mut rt, "effort");
        seen.push(rt.state.choice().effort);
    }
    assert_eq!(seen, ["medium", "high", "xhigh", "", "low"]);
}

#[test]
fn says_default_when_unset_and_names_the_model_in_the_tooltip() {
    let mut rt = app();
    opus(&mut rt);
    let view = rt.state.effort_view().unwrap();
    assert_eq!(view.level, "");
    assert_eq!(view.help(), "Default: the harness's own level, nothing sent");
    click(&mut rt, "effort");
    assert_eq!(rt.state.effort_view().unwrap().help(), "Low, remembered for Opus 5.5");
}

#[test]
fn is_shown_only_for_a_model_with_levels() {
    let mut rt = app();
    // Default is Sonnet through the agent: it has levels.
    assert!(rt.find("effort").is_some());
    let key = create_model_selection_key("anthropic", "haiku-5-5");
    rt.update(move |s, tx| s.pick_model(&key, tx));
    rt.frame(FRAME);
    assert!(rt.find("effort").is_none(), "Haiku has no levels");
}

#[test]
fn a_click_is_remembered_for_the_model_and_a_model_switch_brings_it_back_with_a_note() {
    let mut rt = app();
    opus(&mut rt);
    click(&mut rt, "effort");
    assert_eq!(rt.state.pickers.prefs.get("effort.unknown.anthropic:opus-5-5"), Some("low"));
    let pear = create_model_selection_key("orchard", "pear-2");
    rt.update(move |s, tx| s.pick_model(&pear, tx));
    assert_eq!(rt.state.choice().effort, "", "a model with nothing remembered is on Default");
    let opus = create_model_selection_key("anthropic", "opus-5-5");
    rt.update(move |s, tx| s.pick_model(&opus, tx));
    assert_eq!(rt.state.choice().effort, "low");
    assert_eq!(rt.state.effort_view().unwrap().note.as_deref(), Some("Opus 5.5: back on Low"));
}

// ─── a server's lists ───────────────────────────────────────────────────────

#[test]
fn a_model_that_disappears_falls_back_to_default_once_when_the_list_first_loads() {
    let (mut rt, id) = on_server();
    let gone = create_model_selection_key("p", "old");
    let kept = model("p", "new", "New", &[]);
    rt.update({
        let (id, gone) = (id.clone(), gone.clone());
        move |s, _| s.drafts.set_choice(&id, Choice { model: gone, ..Choice::default() })
    });
    assert_eq!(rt.state.model_chip().label, "old", "named until the list arrives");
    rt.update({
        let (id, kept) = (id.clone(), kept.clone());
        move |s, tx| {
            s.on_agents(&id, Vec::new(), tx);
            s.on_models(&id, vec![kept], tx);
        }
    });
    assert_eq!(rt.state.choice().model, "", "the list lacks it: back on Default");
    // A list that changes later doesn't change the pick: the chip keeps naming it.
    let later = create_model_selection_key("p", "later");
    rt.update({
        let later = later.clone();
        move |s, tx| s.pick_model(&later, tx)
    });
    rt.update({
        let (id, kept) = (id.clone(), kept.clone());
        move |s, tx| s.on_models(&id, vec![kept], tx)
    });
    assert_eq!(rt.state.choice().model, later);
    assert_eq!(rt.state.model_chip().label, "later");
}

#[test]
fn the_prompt_body_carries_only_what_is_picked() {
    let none = prompt_body("hi", "prompt-1", &Pick::default());
    assert_eq!(none, json!({"text": "hi", "correlationId": "prompt-1"}));
    let picked = Pick {
        agent: Some("plan".into()),
        model: Some(("anthropic".into(), "opus-5-5".into())),
        effort: Some("high".into()),
        user_message_id: Some("user-abc".into()),
    };
    let body = prompt_body("hi", "prompt-1", &picked);
    assert_eq!(body["agent"], "plan");
    assert_eq!(body["model"], json!({"providerID": "anthropic", "modelID": "opus-5-5"}));
    assert_eq!((body["effort"].as_str(), body["userMessageId"].as_str()), (Some("high"), Some("user-abc")));
}

#[test]
fn a_model_without_levels_sends_no_effort_and_one_with_them_sends_the_explicit_default() {
    let (mut rt, id) = on_server();
    rt.update({
        let id = id.clone();
        move |s, tx| {
            s.on_agents(&id, Vec::new(), tx);
            s.on_models(&id, vec![model("p", "plain", "Plain", &[]), model("p", "deep", "Deep", &["low", "high"])], tx);
        }
    });
    let plain = create_model_selection_key("p", "plain");
    let deep = create_model_selection_key("p", "deep");
    rt.update(move |s, tx| s.pick_model(&plain, tx));
    assert_eq!(rt.state.prompt_pick(None).effort, None);
    rt.update(move |s, tx| s.pick_model(&deep, tx));
    assert_eq!(rt.state.prompt_pick(None).effort.as_deref(), Some("fleet:default"));
    assert_eq!(rt.state.prompt_pick(None).model, Some(("p".into(), "deep".into())));
}

// ─── against a fake server ──────────────────────────────────────────────────

#[derive(Default)]
struct Seen {
    agents: usize,
    prompts: Vec<Value>,
    puts: Vec<(String, Value)>,
}

fn serve(mut stream: TcpStream, seen: Arc<Mutex<Seen>>) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let (mut head, mut length) = (String::new(), 0);
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            return;
        }
        if line == "\r\n" {
            break;
        }
        if let Some(n) = line.to_ascii_lowercase().strip_prefix("content-length:") {
            length = n.trim().parse().unwrap_or(0);
        }
        head.push_str(&line);
    }
    let mut body = vec![0; length];
    let _ = reader.read_exact(&mut body);
    let request = head.lines().next().unwrap_or("").to_string();
    let reply = |stream: &mut TcpStream, body: &str| {
        let _ = stream.write_all(
            format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            )
            .as_bytes(),
        );
    };
    let mut seen = seen.lock().unwrap();
    if request.starts_with("POST /hubs/session-events/negotiate") {
        reply(&mut stream, "{\"negotiateVersion\":1,\"connectionToken\":\"tok\"}");
    } else if request.starts_with("GET /api/sessions ") || request.starts_with("GET /api/sessions?") {
        reply(&mut stream, &fixture("api-sessions.json"));
    } else if request.contains("/agents ") {
        seen.agents += 1;
        let name = if seen.agents == 1 { "build" } else { "added" };
        reply(&mut stream, &json!({"agents": [{"name": name, "mode": "primary"}]}).to_string());
    } else if request.contains("/models ") {
        reply(
            &mut stream,
            &json!([{"id": "p", "name": "P", "models": [{"id": "m", "name": "M", "variants": ["low", "high"]}]}]).to_string(),
        );
    } else if request.starts_with("GET /api/harnesses") {
        reply(&mut stream, &json!([{"type": "opencode2", "capabilities": {"supportsSteering": true}}]).to_string());
    } else if request.starts_with("GET /api/preferences") {
        reply(&mut stream, "{}");
    } else if request.starts_with("PUT /api/preferences/") {
        let key = request.split(' ').nth(1).unwrap_or("").trim_start_matches("/api/preferences/").to_string();
        seen.puts.push((key, serde_json::from_slice(&body).unwrap_or(Value::Null)));
        reply(&mut stream, "{}");
    } else if request.contains("/prompt ") {
        seen.prompts.push(serde_json::from_slice(&body).unwrap_or(Value::Null));
        reply(&mut stream, "{}");
    }
}

#[test]
fn the_lists_load_on_opening_a_session_and_again_on_a_catalog_event_and_the_prompt_carries_the_pick() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Seen::default()));
    let shared = seen.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let seen = shared.clone();
            std::thread::spawn(move || serve(stream, seen));
        }
    });
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let post = Post::new(move |job| {
        let _ = jobs.lock().unwrap().send(job);
    });
    let mut rt = app();
    let link = Link::start(base, post);
    // The fake serves no hub, so the list is handed over here, as the hub thread would.
    rt.update(move |s, tx| {
        s.link = Some(link);
        s.projects.clear();
        s.load_pickers();
        s.on_list(sessions::parse_list(&serde_json::from_str(&fixture("api-sessions.json")).unwrap(), 0), tx);
    });
    let deadline = Instant::now() + Duration::from_secs(20);
    let step = |rt: &mut Runtime<State>, until: &dyn Fn(&State) -> bool, what: &str| {
        while !until(&rt.state) {
            assert!(Instant::now() < deadline, "{what}");
            while let Ok(job) = queue.try_recv() {
                rt.update(job);
            }
            rt.frame(FRAME);
            std::thread::sleep(Duration::from_millis(10));
        }
    };
    let loaded =
        |s: &State| s.lists().agents_loaded && s.lists().models_loaded && !s.pickers.harnesses.is_empty() && s.pickers.prefs.fetched;
    step(&mut rt, &loaded, "the lists, harnesses and preferences on opening");
    assert_eq!(rt.state.lists().agents[0].id, "build");
    assert_eq!(rt.state.agent_chip().label, "Default (build)");
    assert!(rt.state.caps().can_steer || rt.state.session().row.harness_type.as_deref() != Some("opencode2"));

    // The harness says what it offers changed: ask again, the old list stays until the new one is here.
    let id = rt.state.open_id().unwrap();
    let event =
        Event::Other(Kind::HarnessCatalogChanged, json!({"harnessType": "opencode2", "directory": "/x", "sessionIds": [id.clone()]}));
    rt.update(move |s, _| s.dispatch(&Topic::Sessions, &[event]));
    step(&mut rt, &|s| s.lists().agents.first().is_some_and(|a| a.id == "added"), "the list asked for again");
    // An event naming other sessions asks for nothing.
    let other = Event::Other(Kind::HarnessCatalogChanged, json!({"sessionIds": ["someone-else"]}));
    rt.update(move |s, _| s.dispatch(&Topic::Sessions, &[other]));
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(seen.lock().unwrap().agents, 2);

    // The pick travels with the prompt, and the effort click is written as a preference.
    let m = create_model_selection_key("p", "m");
    rt.update(move |s, tx| {
        s.pick_agent("added", tx);
        s.pick_model(&m, tx);
        s.cycle_effort();
    });
    step(&mut rt, &|_| !seen.lock().unwrap().puts.is_empty(), "the preference write");
    {
        let seen = seen.lock().unwrap();
        let (key, body) = &seen.puts[0];
        assert!(key.starts_with("effort."), "{key}");
        assert_eq!(body["value"], "low");
    }
    rt.update(move |s, _| s.send_to_server(&id, "hello"));
    step(&mut rt, &|_| !seen.lock().unwrap().prompts.is_empty(), "the prompt");
    let body = seen.lock().unwrap().prompts[0].clone();
    assert_eq!(body["text"], "hello");
    assert_eq!(body["agent"], "added");
    assert_eq!(body["model"], json!({"providerID": "p", "modelID": "m"}));
    assert_eq!(body["effort"], "low");
    assert!(body["userMessageId"].as_str().unwrap().starts_with("user-"), "{body}");
    assert_eq!(rt.state.session().row.selected_agent.as_deref(), Some("added"), "the row says what was named");
}
