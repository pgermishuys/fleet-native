//! Component 22, piece K: `ModsSafeModeBanner.test.ts` (the desktop cases), `StatusBar.mods.test.ts` (6) and `Composer.mods.test.ts`
//! (6) under Fleet's names, and the palette's "Start without mods" (`use-start-without-mods.ts`). Fleet's tests read the DOM; these
//! read the element tree of a headless app, a fake Fleet answering the Mods switch and safe mode, and the views a site holds.
use crate::fleet::events::{Event, Kind};
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::mods::points::{ComposerBandView, ModTreeView, OnAction, StatusChipView};
use crate::fleet::mods::types::{Action, ActionKind, Author, Surface};
use crate::fleet::settings::mods_kept::Switch;
use crate::fleet::state::Topic;
use crate::fleet::store::Route;
use crate::tests::{FRAME, app, click};
use crate::{Rail, State, test_http, test_wait};
use rune::Runtime;
use serde_json::{Value, json};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

type Rt = Runtime<State>;
type Go = Box<dyn Fn(&mut State)>;
type Heard = Arc<Mutex<Vec<(Action, Surface)>>>;

fn has(rt: &Rt, key: &str) -> bool {
    rt.elements().iter().any(|e| e.key.as_deref() == Some(key) && !e.exiting)
}

fn settle(rt: &mut Rt) {
    for _ in 0..4 {
        rt.frame(FRAME);
    }
}

/// Every word drawn inside the element at `key`.
fn words_in(rt: &Rt, key: &str) -> String {
    let all = rt.elements();
    let at = all.iter().position(|e| e.key.as_deref() == Some(key)).unwrap_or_else(|| panic!("no element keyed {key:?}"));
    let mut out = Vec::new();
    let mut stack = vec![at];
    while let Some(i) = stack.pop() {
        out.extend(all[i].text.clone());
        let mut kids: Vec<usize> = (0..all.len()).filter(|&j| all[j].parent == Some(i)).collect();
        kids.reverse();
        stack.extend(kids);
    }
    out.join(" ")
}

fn info(rt: &Rt, key: &str) -> rune::ElementInfo {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).unwrap_or_else(|| panic!("no element keyed {key:?}"))
}

fn text_tree(words: &str) -> Value {
    json!({ "type": "Text", "props": {}, "children": [words] })
}

fn author(draft: bool) -> Vec<Author> {
    vec![Author { name: "ci-mod".into(), draft }]
}

// ---- a fake Fleet ----

#[derive(Default)]
struct Fake {
    on: bool,
    safe_mode: bool,
    /// What `PUT /api/mods/safe-mode` answers with, when it refuses: the status line and the body.
    refuse: Option<(&'static str, Value)>,
    calls: Vec<(String, String, Value)>,
}

type Shared = Arc<Mutex<Fake>>;

fn serve(fake: Shared, hold: Arc<AtomicBool>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
    let base = format!("http://{}", listener.local_addr().expect("an address"));
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let (fake, hold) = (fake.clone(), hold.clone());
            std::thread::spawn(move || {
                let req = test_http::read(&mut stream);
                if req.method.is_empty() {
                    return;
                }
                let put = req.method == "PUT" && req.path == "/api/mods/safe-mode";
                fake.lock().unwrap().calls.push((req.method.clone(), req.path.clone(), req.json()));
                // A request the test holds waits here until it lets it go.
                while put && hold.load(Ordering::SeqCst) {
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                let mut f = fake.lock().unwrap();
                let (status, reply) = if req.path == "/api/features/mods" {
                    ("200 OK", json!({ "on": f.on, "safeMode": f.safe_mode }))
                } else if put {
                    match f.refuse.clone() {
                        Some((status, body)) => (status, body),
                        None => {
                            f.safe_mode = req.json()["on"].as_bool().unwrap_or(false);
                            ("200 OK", json!({ "safeMode": f.safe_mode, "mods": [] }))
                        }
                    }
                } else if req.path == "/api/mods" {
                    ("200 OK", json!({ "safeMode": f.safe_mode, "mods": [] }))
                } else {
                    ("200 OK", json!({}))
                };
                drop(f);
                test_http::respond_json(&mut stream, status, &reply.to_string());
            });
        }
    });
    base
}

struct Rig {
    rt: Rt,
    queue: Receiver<Job>,
    fake: Shared,
    hold: Arc<AtomicBool>,
}

/// An app on a fake Fleet whose Mods switch is `on` and whose safe mode is `safe_mode`; nothing has read the switch yet.
fn rig(on: bool, safe_mode: bool) -> Rig {
    let fake: Shared = Arc::new(Mutex::new(Fake { on, safe_mode, ..Default::default() }));
    let hold = Arc::new(AtomicBool::new(false));
    let base = serve(fake.clone(), hold.clone());
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let link = Link::start(
        base,
        Post::new(move |job| {
            let _ = jobs.lock().unwrap().send(job);
        }),
    );
    let mut rt = app();
    rt.update(move |s, _| s.link = Some(link));
    Rig { rt, queue, fake, hold }
}

impl Rig {
    fn wait(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        test_wait::drive(&mut self.rt, &self.queue, what, done, |_| String::new());
        settle(&mut self.rt);
    }

    fn calls(&self, method: &str, path: &str) -> Vec<Value> {
        self.fake.lock().unwrap().calls.iter().filter(|c| c.0 == method && c.1 == path).map(|c| c.2.clone()).collect()
    }
}

// ---- ModsSafeModeBanner.test.ts ----

fn scene(rt: &mut Rt, switch: Option<Switch>) {
    rt.update(move |s, _| s.mod_sites.switch = switch);
    settle(rt);
}

// "says mods are stopped, and why, while safe mode is on"
#[test]
fn says_mods_are_stopped_and_why_while_safe_mode_is_on() {
    let mut rt = app();
    scene(&mut rt, Some(Switch { on: true, safe_mode: true }));
    assert_eq!(info(&rt, "mods-safe-mode-banner").role, "status");
    let said = words_in(&rt, "mods-safe-mode-banner");
    assert!(said.contains("Mods are stopped for now"), "{said}");
    assert!(said.contains("Fleet started without mods. They start again when Fleet restarts or you turn them back on."), "{said}");
    assert_eq!(info(&rt, "mods-safe-mode-turn-on").label.as_deref(), Some("Turn mods back on"));
}

// "shows nothing for %j"
#[test]
fn shows_nothing_for_the_switch_on_with_safe_mode_off_and_for_the_switch_off_with_safe_mode_on() {
    let mut rt = app();
    for state in [Switch { on: true, safe_mode: false }, Switch { on: false, safe_mode: true }] {
        scene(&mut rt, Some(state));
        assert!(!has(&rt, "mods-safe-mode-banner"), "{state:?}");
    }
}

// "reads the switch when it hasn't been read, and not otherwise"
#[test]
fn reads_the_switch_when_it_hasnt_been_read_and_not_otherwise() {
    let mut r = rig(true, true);
    r.wait("the switch to be read", |s| s.mod_sites.switch.is_some());
    assert_eq!(r.calls("GET", "/api/features/mods").len(), 1);
    assert!(has(&r.rt, "mods-safe-mode-banner"), "what it read shows");
    // A second pass asks for nothing: it has been read.
    settle(&mut r.rt);
    assert_eq!(r.calls("GET", "/api/features/mods").len(), 1);

    let mut known = rig(true, true);
    known.rt.update(|s, _| s.mod_sites.switch = Some(Switch { on: true, safe_mode: true }));
    settle(&mut known.rt);
    known.rt.update(|_, _| {});
    assert!(known.calls("GET", "/api/features/mods").is_empty(), "a known switch is not asked for");
}

// "turns mods back on from the button, disabled while it waits"
#[test]
fn turns_mods_back_on_from_the_button_disabled_while_it_waits() {
    let mut r = rig(true, true);
    r.wait("the switch to be read", |s| s.mod_sites.switch.is_some());
    r.hold.store(true, Ordering::SeqCst);
    click(&mut r.rt, "mods-safe-mode-turn-on");
    r.wait("the request to be sent", |s| s.mod_sites.pending);
    // `pending` is set when the button is pressed; the fake hears the request a moment later.
    let fake = r.fake.clone();
    r.wait("the server to hear it", move |_| fake.lock().unwrap().calls.iter().any(|c| c.0 == "PUT"));
    assert_eq!(r.calls("PUT", "/api/mods/safe-mode"), [json!({ "on": false })]);
    let button = info(&r.rt, "mods-safe-mode-turn-on");
    assert_eq!(button.label.as_deref(), Some("Turning on\u{2026}"));
    assert!(button.opacity < 0.9, "the button is dimmed while it waits");
    // A second press while it waits sends nothing.
    r.rt.update(|s, _| s.set_mods_safe_mode(false, true));
    r.hold.store(false, Ordering::SeqCst);
    r.wait("the answer", |s| !s.mod_sites.pending);
    assert_eq!(r.calls("PUT", "/api/mods/safe-mode").len(), 1);
    assert!(!has(&r.rt, "mods-safe-mode-banner"), "the banner goes");
}

// "shows the server's refusal inline and keeps the button"
#[test]
fn shows_the_servers_refusal_inline_and_keeps_the_button() {
    let mut r = rig(true, true);
    r.wait("the switch to be read", |s| s.mod_sites.switch.is_some());
    r.fake.lock().unwrap().refuse = Some(("409 Conflict", json!({ "error": "Mods couldn't start." })));
    click(&mut r.rt, "mods-safe-mode-turn-on");
    r.wait("the refusal", |s| s.mod_sites.error.is_some());
    assert_eq!(words_in(&r.rt, "mods-safe-mode-error"), "Mods couldn't start.");
    assert_eq!(info(&r.rt, "mods-safe-mode-error").role, "alert");
    let button = info(&r.rt, "mods-safe-mode-turn-on");
    assert_eq!(button.label.as_deref(), Some("Turn mods back on"));
    assert!(button.opacity > 0.9, "the button is back");
}

// The server's words come as they are, and the banner follows a `mods.changed` for safe mode (`stores/mods.ts`'s listener).
#[test]
fn follows_safe_mode_events() {
    let mut r = rig(true, false);
    r.wait("the switch to be read", |s| s.mod_sites.switch.is_some());
    assert!(!has(&r.rt, "mods-safe-mode-banner"));
    r.fake.lock().unwrap().safe_mode = true;
    r.rt.update(|s, _| s.dispatch(&Topic::Sessions, &[Event::Other(Kind::ModsChanged, json!({ "reason": "safe-mode" }))]));
    r.wait("the banner", |s| s.mods_banner_shown());
    assert!(has(&r.rt, "mods-safe-mode-banner"));
}

// ---- "Start without mods" (use-start-without-mods.ts, use-commands.ts) ----

fn palette_commands(rt: &Rt) -> Vec<(String, String, String)> {
    crate::ui::commands::all(&rt.state)
        .into_iter()
        .filter(|c| c.id == "start-without-mods")
        .map(|c| (c.label, c.description, c.id))
        .collect()
}

#[test]
fn the_palette_command_follows_safe_mode_and_is_hidden_while_mods_are_off() {
    let mut rt = app();
    assert!(palette_commands(&rt).is_empty(), "nothing is known about the switch");
    scene(&mut rt, Some(Switch { on: false, safe_mode: false }));
    assert!(palette_commands(&rt).is_empty(), "the switch is off");
    scene(&mut rt, Some(Switch { on: true, safe_mode: false }));
    assert_eq!(
        palette_commands(&rt)[0].0.clone() + "|" + &palette_commands(&rt)[0].1,
        "Start without mods|Stop your mods until Fleet restarts or you turn them back on."
    );
    scene(&mut rt, Some(Switch { on: true, safe_mode: true }));
    assert_eq!(palette_commands(&rt)[0].0.clone() + "|" + &palette_commands(&rt)[0].1, "Turn mods back on|Start your mods again.");
    // The user's own choice wins as soon as it is made.
    rt.update(|s, _| {
        s.pickers.prefs.set("Mods", "false");
    });
    assert!(palette_commands(&rt).is_empty());
}

#[test]
fn the_palette_command_flips_safe_mode_on_the_server() {
    let mut r = rig(true, false);
    r.wait("the switch to be read", |s| s.mod_sites.switch.is_some());
    r.rt.update(|s, tx| crate::ui::commands::run(s, tx, "start-without-mods"));
    r.wait("safe mode", |s| s.mods_safe_mode());
    assert_eq!(r.calls("PUT", "/api/mods/safe-mode"), [json!({ "on": true })]);
    assert!(has(&r.rt, "mods-safe-mode-banner"));
    r.rt.update(|s, tx| crate::ui::commands::run(s, tx, "start-without-mods"));
    r.wait("mods back on", |s| !s.mods_safe_mode());
    assert_eq!(r.calls("PUT", "/api/mods/safe-mode")[1], json!({ "on": false }));
}

#[test]
fn a_refusal_from_the_palette_command_posts_a_notice() {
    let mut r = rig(true, false);
    r.wait("the switch to be read", |s| s.mod_sites.switch.is_some());
    r.fake.lock().unwrap().refuse = Some(("409 Conflict", json!({ "error": "Mods couldn't stop." })));
    r.rt.update(|s, tx| crate::ui::commands::run(s, tx, "start-without-mods"));
    r.wait("the notice", |s| !s.notices.notices().is_empty());
    let n = r.rt.state.notices.notices()[0].clone();
    assert_eq!(
        (n.title.as_str(), n.body.as_deref(), n.tone, n.expires_secs),
        ("Couldn't change mods", Some("Mods couldn't stop."), crate::fleet::notices::Tone::Warn, Some(20.0))
    );
    assert!(!r.rt.state.mods_safe_mode(), "a refusal changes nothing");

    // With no words from the server it says so.
    r.fake.lock().unwrap().refuse = Some(("500 Internal Server Error", json!({})));
    r.rt.update(|s, tx| crate::ui::commands::run(s, tx, "start-without-mods"));
    r.wait("the second notice", |s| s.notices.notices().len() == 2);
    assert_eq!(r.rt.state.notices.notices()[1].body.as_deref(), Some("Couldn't reach Mods (500)."));
}

// ---- StatusBar.mods.test.ts ----

fn chip(session: &str, tree: Value, draft: bool, on_action: Option<OnAction>) -> StatusChipView {
    let mut view = ModTreeView::new(tree, author(draft));
    view.on_action = on_action;
    StatusChipView { session_id: session.into(), view }
}

fn band(session: &str, tree: Value, draft: bool, on_action: Option<OnAction>) -> ComposerBandView {
    let mut view = ModTreeView::new(tree, author(draft));
    view.on_action = on_action;
    ComposerBandView { session_id: session.into(), view }
}

/// An app whose open session is `s1`.
fn on_session() -> Rt {
    let mut rt = app();
    rt.update(|s, _| {
        s.projects[0].sessions[0].id = "s1".into();
        s.route = Route::Session("s1".into());
    });
    settle(&mut rt);
    rt
}

fn contribute_chip(rt: &mut Rt, view: StatusChipView) {
    rt.update(move |s, _| {
        s.mod_sites.chips.contribute("t", vec![view]);
    });
    settle(rt);
}

// "draws nothing for a session that is not on screen, nor with no session"
#[test]
fn draws_nothing_for_a_session_that_is_not_on_screen_nor_with_no_session() {
    let mut rt = on_session();
    contribute_chip(&mut rt, chip("other", text_tree("build ok"), false, None));
    assert!(!has(&rt, "mod-status-chip"));
    // No session at all: the route has none.
    rt.update(|s, _| {
        s.mod_sites.chips.clear();
        s.mod_sites.chips.contribute("t", vec![chip("s1", text_tree("build ok"), false, None)]);
        s.route = Route::None;
    });
    settle(&mut rt);
    assert!(!has(&rt, "mod-status-chip"));
}

// "draws no chip on %s, though a session is still the active one"
#[test]
fn draws_no_chip_on_settings_workflows_automations_the_dashboard_or_a_new_session_though_a_session_is_still_the_active_one() {
    let mut rt = on_session();
    contribute_chip(&mut rt, chip("s1", text_tree("build ok"), false, None));
    assert!(has(&rt, "mod-status-chip"));
    let elsewhere: [(&str, Go); 5] = [
        ("Settings", Box::new(|s| s.rail = Rail::Settings)),
        ("Workflows", Box::new(|s| s.rail = Rail::Workflows)),
        ("Automations", Box::new(|s| s.rail = Rail::Automations)),
        ("the dashboard", Box::new(|s| s.route = Route::Home)),
        ("a new session", Box::new(|s| s.route = Route::NewSession { project_id: None })),
    ];
    for (name, go) in elsewhere {
        rt.update(|s, _| {
            s.rail = Rail::Sessions;
            s.route = Route::Session("s1".into());
        });
        rt.update(|s, _| go(s));
        settle(&mut rt);
        assert!(!has(&rt, "mod-status-chip"), "no chip on {name}");
        // The model and tokens still show: only the chip is gated on the route.
        assert!(has(&rt, "status-bar"), "the bar is still there on {name}");
    }
}

// "draws the chip on a session route"
#[test]
fn draws_the_chip_on_a_session_route() {
    let mut rt = on_session();
    contribute_chip(&mut rt, chip("s1", text_tree("build ok"), false, None));
    assert!(has(&rt, "mod-status-chip"));
}

// "draws the chip just before the session's model and tokens"
#[test]
fn draws_the_chip_just_before_the_sessions_model_and_tokens() {
    let mut rt = on_session();
    contribute_chip(&mut rt, chip("s1", text_tree("build ok"), false, None));
    assert!(words_in(&rt, "mod-status-chip").contains("build ok"));
    let all = rt.elements();
    let at = |key: &str| all.iter().position(|e| e.key.as_deref() == Some(key)).unwrap();
    let chip_at = at("mod-status-chip");
    let parent = all[chip_at].parent.expect("the chip is in the end of the bar");
    let next = (chip_at + 1..all.len()).find(|&i| all[i].parent == Some(parent)).expect("something after the chip");
    // What follows the chip is the session's part: the harness, the bar and the tokens.
    let after = all.iter().enumerate().filter(|(_, e)| e.parent == Some(parent)).map(|(i, _)| i).collect::<Vec<_>>();
    assert_eq!(after.last().copied(), Some(next), "the session's own part is the last child");
    let session_words = words_in_index(&all, next);
    assert!(session_words.contains("tokens"), "{session_words}");
    assert!(!has(&rt, "mod-draft-mark"), "a kept mod has no mark");
}

fn words_in_index(all: &[rune::ElementInfo], at: usize) -> String {
    let mut out = Vec::new();
    let mut stack = vec![at];
    while let Some(i) = stack.pop() {
        out.extend(all[i].text.clone());
        stack.extend((0..all.len()).filter(|&j| all[j].parent == Some(i)));
    }
    out.join(" ")
}

// "marks a draft"
#[test]
fn marks_a_draft() {
    let mut rt = on_session();
    contribute_chip(&mut rt, chip("s1", text_tree("build ok"), true, None));
    let all = rt.elements();
    let chip_at = all.iter().position(|e| e.key.as_deref() == Some("mod-status-chip")).unwrap();
    let mark = all.iter().position(|e| e.key.as_deref() == Some("mod-draft-mark")).expect("the mark");
    let mut up = all[mark].parent;
    while let Some(p) = up {
        if p == chip_at {
            return;
        }
        up = all[p].parent;
    }
    panic!("the Draft mark is not inside the chip");
}

// "draws nothing for a null or invalid tree, and removal restores the bar"
#[test]
fn draws_nothing_for_a_null_or_invalid_tree_and_removal_restores_the_bar() {
    let mut rt = on_session();
    let before = rt.elements().iter().filter_map(|e| e.key.clone()).collect::<Vec<_>>();
    contribute_chip(&mut rt, chip("s1", Value::Null, false, None));
    assert!(!has(&rt, "mod-status-chip"));
    contribute_chip(&mut rt, chip("s1", json!({ "type": "Nope" }), false, None));
    assert!(!has(&rt, "mod-status-chip"));
    contribute_chip(&mut rt, chip("s1", text_tree("build ok"), false, None));
    assert!(has(&rt, "mod-status-chip"));
    rt.update(|s, _| s.mod_sites.chips.remove_by_owner("t"));
    settle(&mut rt);
    let after = rt.elements().iter().filter_map(|e| e.key.clone()).collect::<Vec<_>>();
    assert_eq!(after, before);
}

fn recorder() -> (OnAction, Heard) {
    let heard: Heard = Arc::default();
    let sink = heard.clone();
    (Arc::new(move |a: &Action, s: Surface| sink.lock().unwrap().push((a.clone(), s))), heard)
}

fn button_tree() -> Value {
    json!({ "type": "Button", "props": { "key": "go", "label": "Go" }, "handles": { "onPress": "h1" } })
}

// "sends an action from the chip as desktop"
#[test]
fn sends_an_action_from_the_chip_as_desktop() {
    let mut rt = on_session();
    let (sink, heard) = recorder();
    contribute_chip(&mut rt, chip("s1", button_tree(), false, Some(sink)));
    click(&mut rt, "mod-status-chip-tree.0");
    settle(&mut rt);
    let heard = heard.lock().unwrap();
    assert_eq!(heard.len(), 1);
    assert_eq!(heard[0].0, Action { handle: "h1".into(), kind: ActionKind::Press, value: None });
    assert_eq!(heard[0].1, Surface::Desktop);
}

// ---- Composer.mods.test.ts ----

fn contribute_band(rt: &mut Rt, view: ComposerBandView) {
    rt.update(move |s, _| {
        s.mod_sites.bands.contribute("test", vec![view]);
    });
    settle(rt);
}

// "draws the band after the background strip and before the composer frame"
#[test]
fn draws_the_band_after_the_background_strip_and_before_the_composer_frame() {
    let mut rt = on_session();
    contribute_band(&mut rt, band("s1", text_tree("Lint clean"), false, None));
    assert!(has(&rt, "mod-composer-band"));
    assert!(words_in(&rt, "mod-composer-band").contains("Lint clean"));
    let all = rt.elements();
    let at = |key: &str| all.iter().position(|e| e.key.as_deref() == Some(key)).unwrap_or_else(|| panic!("no {key}"));
    let (band_at, field_at) = (at("mod-composer-band"), at("composer"));
    assert!(band_at < field_at, "the band comes before the field");
    // Siblings: the band is a child of the composer section, ahead of the frame the field is in.
    assert_eq!(all[band_at].parent, Some(at("composer-section")));
    let rect = |i: usize| all[i].rect;
    assert!(rect(band_at)[1] + rect(band_at)[3] <= rect(field_at)[1], "the band is above the field");
}

// "draws only its own session's band"
#[test]
fn draws_only_its_own_sessions_band() {
    let mut rt = on_session();
    contribute_band(&mut rt, band("session-2", text_tree("Other"), false, None));
    assert!(!has(&rt, "mod-composer-band"));
}

// "restores today's DOM when the band is removed"
#[test]
fn restores_todays_dom_when_the_band_is_removed() {
    let mut rt = on_session();
    let before = rt.elements().iter().filter_map(|e| e.key.clone()).collect::<Vec<_>>();
    rt.update(|s, _| {
        s.mod_sites.bands.contribute("test", vec![band("s1", text_tree("Here"), false, None)]);
    });
    settle(&mut rt);
    assert!(has(&rt, "mod-composer-band"));
    rt.update(|s, _| s.mod_sites.bands.remove_by_owner("test"));
    settle(&mut rt);
    let after = rt.elements().iter().filter_map(|e| e.key.clone()).collect::<Vec<_>>();
    assert_eq!(after, before);
}

// "draws nothing for a null tree or an invalid one"
#[test]
fn draws_nothing_for_a_null_tree_or_an_invalid_one() {
    let mut rt = on_session();
    contribute_band(&mut rt, band("s1", Value::Null, false, None));
    assert!(!has(&rt, "mod-composer-band"));
    rt.update(|s, _| s.mod_sites.bands.clear());
    contribute_band(&mut rt, band("s1", json!({ "type": "Nope", "props": {} }), false, None));
    assert!(!has(&rt, "mod-composer-band"));
}

// "marks a draft mod's band and sends actions from the desktop"
#[test]
fn marks_a_draft_mods_band_and_sends_actions_from_the_desktop() {
    let mut rt = on_session();
    let (sink, heard) = recorder();
    contribute_band(&mut rt, band("s1", button_tree(), true, Some(sink)));
    assert!(has(&rt, "mod-draft-mark"));
    click(&mut rt, "mod-composer-band-tree.0");
    settle(&mut rt);
    let heard = heard.lock().unwrap();
    assert_eq!(heard.len(), 1);
    assert_eq!(heard[0].1, Surface::Desktop);
}

// "has no draft mark for a kept mod"
#[test]
fn has_no_draft_mark_for_a_kept_mod() {
    let mut rt = on_session();
    contribute_band(&mut rt, band("s1", text_tree("Kept"), false, None));
    assert!(has(&rt, "mod-composer-band"));
    assert!(!has(&rt, "mod-draft-mark"));
}
