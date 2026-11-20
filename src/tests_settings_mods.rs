//! Component 16, piece AE: Plugins › Mods. Fleet's `ModsRuntimeRow.test.ts` (22), `mods-and-skills-plugins.test.ts` (6) and
//! `mods-runtime.test.ts` (4) cases under their names, over the fixed table of plugins, against a fake Fleet that answers
//! `GET /api/features/mods/runtime` and its three POSTs. A test counts only the requests it means (`called`).
use crate::fleet::events::{Event, Kind};
use crate::fleet::settings::mods_runtime as rt;
use crate::fleet::settings::plugins as p;
use crate::fleet::settings_nav::Page;
use crate::fleet::state::Topic;
use crate::tests::click;
use crate::tests_settings_general::{Rig, unread};
use serde_json::{Value, json};

const RUNTIME: &str = "/api/features/mods/runtime";
const NOW: i64 = 1_791_633_600_000; // 2026-10-10T12:00:00Z

fn release() -> Value {
    json!({ "version": "1.4.2", "size": 35_300_000, "hasBuild": true, "installFolder": "~/.weave/runtimes/bun/1.4.2", "source": "github.com/oven-sh/bun" })
}

fn make_view(bun: Value, job: Value, found: Value) -> Value {
    json!({ "bun": bun, "release": release(), "job": job, "found": found })
}

fn view() -> Value {
    make_view(Value::Null, Value::Null, json!([]))
}

fn installed(version: &str) -> Value {
    json!({ "path": format!("/u/bun/{version}/bun"), "displayPath": format!("~/.weave/runtimes/bun/{version}/bun"), "source": "installed", "version": version, "safe": true })
}

fn job(phase: &str) -> Value {
    json!({ "phase": phase, "version": "1.4.2", "bytesReceived": 12_000_000, "bytesTotal": 35_300_000, "startedAt": "2026-10-10T11:59:20Z" })
}

fn failed(reason: &str, message: &str) -> Value {
    json!({ "phase": "failed", "version": "1.4.2", "reason": reason, "message": message })
}

fn mine() -> Value {
    json!([{ "path": "/home/u/.bun/bin/bun", "displayPath": "~/.bun/bin/bun", "version": "1.4.2" }])
}

fn serve_route(r: &Rig, route: &str, code: u16, body: Value) {
    r.fake.lock().unwrap().routes.insert(route.to_string(), (code, body));
}

/// The Plugins page on a Fleet that answers with `view`; `mods` is the preference the server holds, if any.
fn mount(view: Value, mods: Option<&str>) -> Rig {
    mount_on("plugins", view, mods)
}

fn mount_on(page: &'static str, view: Value, mods: Option<&str>) -> Rig {
    let prefs: Vec<(&str, &str)> = mods.map(|m| ("Mods", m)).into_iter().collect();
    let mut r = unread(&prefs);
    serve_route(&r, &format!("GET {RUNTIME}"), 200, view);
    r.rt.update(move |s, _| {
        s.now_ms = Some(NOW);
        s.open_settings(page, None);
    });
    r.wait("the preferences and the runtime", |s| s.pickers.prefs.fetched && s.settings.mods_runtime.view.is_some());
    r.frames();
    r
}

fn called(r: &Rig, action: &str) -> usize {
    let path = format!("{RUNTIME}/{action}");
    r.fake.lock().unwrap().calls.iter().filter(|(m, p, _)| m == "POST" && *p == path).count()
}

fn gets(r: &Rig) -> usize {
    r.fake.lock().unwrap().calls.iter().filter(|(m, p, _)| m == "GET" && p == RUNTIME).count()
}

fn body_of(r: &Rig, action: &str) -> String {
    let path = format!("{RUNTIME}/{action}");
    r.fake.lock().unwrap().calls.iter().find(|(_, p, _)| *p == path).map(|c| c.2.clone()).unwrap_or_default()
}

fn mods_puts(r: &Rig) -> usize {
    r.puts().iter().filter(|(k, _)| k == "Mods").count()
}

fn panel(r: &Rig) -> Option<rt::Panel> {
    let run = &r.rt.state.settings.mods_runtime;
    rt::panel_state(run.view.as_ref(), run.confirming, r.rt.state.plugin_enabled("mods"))
}

fn on(r: &Rig) -> bool {
    r.rt.state.plugin_enabled("mods")
}

fn has(r: &Rig, key: &str) -> bool {
    r.info(key).is_some()
}

/// The buttons of the open panel, in order.
fn buttons(r: &Rig) -> Vec<String> {
    ["mods-use", "mods-install", "mods-retry", "mods-cancel-confirm", "mods-cancel"]
        .iter()
        .filter_map(|k| r.info(k))
        .filter_map(|e| e.label)
        .collect()
}

/// The POST has reached Fleet and its answer has been taken into the page: the page asks for the preferences again right after
/// it takes the view, so a read of them after the POST means the view is in.
fn answered(r: &mut Rig, action: &'static str) {
    let fake = r.fake.clone();
    let path = format!("{RUNTIME}/{action}");
    r.wait(&format!("{action} to be answered and the preferences read again"), move |_| {
        let calls = fake.lock().unwrap().calls.clone();
        let post = calls.iter().rposition(|c| c.0 == "POST" && c.1 == path);
        post.is_some_and(|i| calls[i..].iter().any(|c| c.0 == "GET" && c.1 == "/api/preferences"))
    });
    r.frames();
}

fn event(r: &mut Rig, kind: Kind, payload: Value) {
    r.rt.update(move |s, _| s.dispatch(&Topic::Sessions, &[Event::Other(kind, payload)]));
    r.frames();
}

// ─── ModsRuntimeRow.test.ts ─────────────────────────────────────────────────────

#[test]
fn row_1_off_shows_nothing_under_the_description() {
    let r = mount(view(), None);
    assert!(!on(&r));
    assert!(!has(&r, "plugin-panel-mods"));
}

#[test]
fn row_2_confirm_switching_on_with_no_bun_says_what_fleet_will_download_and_the_switch_stays_off() {
    let mut r = mount(view(), None);
    click(&mut r.rt, "plugin-switch-mods");
    r.frames();
    assert_eq!(panel(&r), Some(rt::Panel::Confirm));
    let words = r.words();
    assert!(words.contains("Fleet downloads Bun 1.4.2 (about 35 MB) from github.com/oven-sh/bun"), "{words}");
    assert!(words.contains("installs it in ~/.weave/runtimes/bun/1.4.2. Nothing else on this machine changes."), "{words}");
    assert!(!on(&r));
    assert!(r.rt.state.settings.mods_runtime.confirming, "the switch rings while it asks");
    assert_eq!(mods_puts(&r), 0);
    assert_eq!(buttons(&r), ["Turn on and install", "Cancel"]);
}

#[test]
fn row_2_confirm_a_click_before_fleet_has_said_whether_bun_is_there_still_asks_first() {
    let mut r = mount(view(), None);
    // The view is not known yet: forget it, as if the first read had failed, and press.
    r.rt.update(|s, _| s.settings.mods_runtime.view = None);
    click(&mut r.rt, "plugin-switch-mods");
    r.wait("the question", |s| s.settings.mods_runtime.confirming);
    r.frames();
    assert_eq!(panel(&r), Some(rt::Panel::Confirm));
    assert_eq!(mods_puts(&r), 0);
}

#[test]
fn row_2_confirm_cancel_folds_the_row_back_to_step_1() {
    let mut r = mount(view(), None);
    click(&mut r.rt, "plugin-switch-mods");
    r.frames();
    click(&mut r.rt, "mods-cancel-confirm");
    r.frames();
    assert!(!has(&r, "plugin-panel-mods"));
    assert_eq!(called(&r, "install"), 0);
}

#[test]
fn row_2_confirm_with_no_build_for_this_machine_it_says_so_and_only_offers_cancel() {
    let mut v = view();
    v["release"]["hasBuild"] = json!(false);
    let mut r = mount(v, None);
    click(&mut r.rt, "plugin-switch-mods");
    r.frames();
    assert!(r.words().contains("Fleet has no Bun build for this machine. Install Bun yourself and point Fleet at it."));
    assert_eq!(buttons(&r), ["Cancel"]);
}

#[test]
fn row_2_confirm_turn_on_and_install_posts_the_install_and_shows_its_progress() {
    let mut r = mount(view(), None);
    click(&mut r.rt, "plugin-switch-mods");
    r.frames();
    serve_route(&r, &format!("POST {RUNTIME}/install"), 200, make_view(Value::Null, job("downloading"), json!([])));
    click(&mut r.rt, "mods-install");
    answered(&mut r, "install");
    assert_eq!(called(&r, "install"), 1);
    assert_eq!(panel(&r), Some(rt::Panel::Progress));
    assert!(r.words().contains("This keeps going if you leave Settings. Mods start as soon as it's done."));
    assert!(!r.rt.state.settings.mods_runtime.confirming, "the ring is gone");
}

#[test]
fn row_2_confirm_with_a_bun_found_it_offers_it_first_then_fleet_s_own_then_cancel() {
    let mut r = mount(make_view(Value::Null, Value::Null, mine()), None);
    click(&mut r.rt, "plugin-switch-mods");
    r.frames();
    assert_eq!(panel(&r), Some(rt::Panel::Confirm));
    let words = r.words();
    assert!(words.contains("Mods need Bun"));
    assert!(
        words.contains("Fleet found Bun 1.4.2 at ~/.bun/bin/bun. Mods can run on it, and you keep it up to date with bun upgrade."),
        "{words}"
    );
    assert!(
        words.contains(
            "Or Fleet can download its own Bun 1.4.2 (about 35 MB) from github.com/oven-sh/bun, check it, install it in ~/.weave/runtimes/bun/1.4.2 and keep it up to date."
        ),
        "{words}"
    );
    assert_eq!(buttons(&r), ["Use my Bun 1.4.2", "Download Fleet's own", "Cancel"]);
    assert_eq!(mods_puts(&r), 0);
}

#[test]
fn row_2_confirm_use_my_bun_posts_the_path_shows_mods_on_with_the_user_s_bun_and_downloads_nothing() {
    let mut r = mount(make_view(Value::Null, Value::Null, mine()), None);
    click(&mut r.rt, "plugin-switch-mods");
    r.frames();
    let mine_bun = json!({ "path": "/home/u/.bun/bin/bun", "displayPath": "~/.bun/bin/bun", "source": "configured", "version": "1.4.2", "safe": true });
    serve_route(&r, &format!("POST {RUNTIME}/use"), 200, make_view(mine_bun, Value::Null, json!([])));
    // The server turned the switch on; the preferences are read again after the answer.
    r.fake.lock().unwrap().prefs.insert("Mods".into(), "true".into());
    click(&mut r.rt, "mods-use");
    answered(&mut r, "use");
    r.wait("Mods to show on", |s| s.plugin_enabled("mods"));
    r.frames();
    assert_eq!(called(&r, "use"), 1);
    assert_eq!(called(&r, "install"), 0);
    assert_eq!(body_of(&r, "use"), json!({ "path": "/home/u/.bun/bin/bun" }).to_string());
    assert_eq!(panel(&r), Some(rt::Panel::Ready));
    assert!(r.words().contains("Mods on \u{b7} your Bun 1.4.2"));
    assert!(!has(&r, "mods-progress-bar"));
}

#[test]
fn row_2_confirm_download_fleet_s_own_posts_the_install_even_when_a_bun_was_found() {
    let mut r = mount(make_view(Value::Null, Value::Null, mine()), None);
    click(&mut r.rt, "plugin-switch-mods");
    r.frames();
    serve_route(&r, &format!("POST {RUNTIME}/install"), 200, make_view(Value::Null, job("downloading"), json!([])));
    click(&mut r.rt, "mods-install");
    answered(&mut r, "install");
    assert_eq!(called(&r, "install"), 1);
    assert_eq!(called(&r, "use"), 0);
    assert_eq!(panel(&r), Some(rt::Panel::Progress));
}

#[test]
fn row_2_confirm_a_refused_bun_shows_the_server_s_reason() {
    let mut r = mount(make_view(Value::Null, Value::Null, mine()), None);
    click(&mut r.rt, "plugin-switch-mods");
    r.frames();
    serve_route(&r, &format!("POST {RUNTIME}/use"), 400, json!({ "error": "Bun 1.3.0 is older than 1.4.0." }));
    click(&mut r.rt, "mods-use");
    r.wait("the refusal", |s| s.settings.mods_runtime.action_error.is_some());
    r.frames();
    assert!(r.words().contains("Bun 1.3.0 is older than 1.4.0."));
}

#[test]
fn row_3a_downloading_heading_megabytes_bar_steps_and_the_announcement() {
    let r = mount(make_view(Value::Null, job("downloading"), json!([])), Some("true"));
    assert_eq!(panel(&r), Some(rt::Panel::Progress));
    let words = r.words();
    assert!(words.contains("Downloading Bun 1.4.2"));
    assert!(words.contains("12 of 35 MB"));
    assert_eq!(r.info("mods-progress-bar").unwrap().label.as_deref(), Some("Downloading Bun 1.4.2: 12 of 35 MB"));
    let steps: Vec<&str> = rt::steps(&rt::Job { phase: "downloading".into(), ..Default::default() }).iter().map(|s| s.2).collect();
    assert_eq!(steps, ["current", "todo", "todo"]);
    assert_eq!(r.info("mods-step-download").unwrap().label.as_deref(), Some("Download current"));
    assert_eq!(r.info("mods-step-check").unwrap().label.as_deref(), Some("Check todo"));
}

#[test]
fn row_3b_verifying_the_heading_and_steps_move_on() {
    let r = mount(make_view(Value::Null, job("verifying"), json!([])), Some("true"));
    assert!(r.words().contains("Checking the download"));
    let at = |id: &str| r.info(id).unwrap().label.unwrap();
    assert_eq!(
        [at("mods-step-download"), at("mods-step-check"), at("mods-step-unpack")],
        ["Download done", "Check current", "Unpack todo"]
    );
}

#[test]
fn row_3c_extracting_the_heading_and_steps_move_on() {
    let r = mount(make_view(Value::Null, job("extracting"), json!([])), Some("true"));
    assert!(r.words().contains("Unpacking"));
    let at = |id: &str| r.info(id).unwrap().label.unwrap();
    assert_eq!(
        [at("mods-step-download"), at("mods-step-check"), at("mods-step-unpack")],
        ["Download done", "Check done", "Unpack current"]
    );
}

#[test]
fn row_3d_came_back_an_install_already_running_when_the_row_loaded_says_how_long_it_has_been_going() {
    let mut j = job("downloading");
    j["startedAt"] = json!("2026-10-10T11:57:00Z");
    let r = mount(make_view(Value::Null, j, json!([])), Some("true"));
    assert!(r.words().contains("Started 3 minutes ago. It kept going while you were away."), "{}", r.words());
}

#[test]
fn row_3_cancel_posts_cancel() {
    let mut r = mount(make_view(Value::Null, job("downloading"), json!([])), Some("true"));
    serve_route(&r, &format!("POST {RUNTIME}/cancel"), 200, make_view(Value::Null, failed("cancelled", ""), json!([])));
    click(&mut r.rt, "mods-cancel");
    answered(&mut r, "cancel");
    assert_eq!(called(&r, "cancel"), 1);
    assert!(!has(&r, "plugin-panel-mods"));
}

#[test]
fn row_4_ready_mods_on_with_the_version_and_the_path() {
    let r = mount(make_view(installed("1.4.2"), Value::Null, json!([])), Some("true"));
    assert_eq!(panel(&r), Some(rt::Panel::Ready));
    assert_eq!(r.text("mods-ready-text"), "Mods on \u{b7} Bun 1.4.2");
    assert_eq!(r.text("mods-ready-path"), "~/.weave/runtimes/bun/1.4.2/bun");
}

#[test]
fn row_4_ready_points_to_the_showcase_in_mods_instead_of_listing_the_samples() {
    let mut r = mount(make_view(installed("1.4.2"), Value::Null, json!([])), Some("true"));
    assert!(r.words().contains("To see what mods can do, add one from the"));
    click(&mut r.rt, "mods-showcase-link");
    r.frames();
    assert_eq!(r.rt.state.settings.page, Page::Skills);
    assert_eq!(r.rt.state.settings.anchor.as_deref(), Some("mods"));
}

#[test]
fn row_3_downloading_doesn_t_point_to_the_showcase_until_bun_is_there() {
    let r = mount(make_view(Value::Null, job("downloading"), json!([])), Some("true"));
    assert_eq!(panel(&r), Some(rt::Panel::Progress));
    assert!(!has(&r, "mods-showcase-link"));
}

#[test]
fn row_4_ready_with_a_bun_from_config_it_says_your_bun() {
    let mut bun = installed("1.4.5");
    bun["source"] = json!("configured");
    bun["displayPath"] = json!("/opt/bun/bin/bun");
    let mut v = make_view(bun, Value::Null, json!([]));
    v["configuredPath"] = json!("/u/bun/1.4.5/bun");
    let r = mount(v, Some("true"));
    assert_eq!(r.text("mods-ready-text"), "Mods on \u{b7} your Bun 1.4.5");
    assert_eq!(r.text("mods-ready-path"), "/opt/bun/bin/bun");
}

fn failed_row(reason: &str, message: &str, hint: &str) {
    let r = mount(make_view(Value::Null, failed(reason, message), json!([])), Some("false"));
    assert_eq!(panel(&r), Some(rt::Panel::Failed));
    assert_eq!(r.text("mods-failed-heading"), "\u{26a0} Couldn't download Bun");
    let words = r.text("mods-failed-text");
    assert!(words.contains(message) && words.contains(hint), "{words}");
    assert!(has(&r, "mods-retry"));
    assert!(!on(&r));
}

#[test]
fn row_5a_failed_offline_red_heading_the_message_the_hint_and_try_again_the_switch_is_off() {
    failed_row(
        "offline",
        "Fleet couldn't reach github.com: the connection timed out.",
        "If GitHub is blocked here, install Bun yourself and point Fleet at it.",
    );
}

#[test]
fn row_5b_failed_blocked_red_heading_the_message_the_hint_and_try_again_the_switch_is_off() {
    failed_row("blocked", "A proxy answered 403.", "Ask whoever runs your network to allow github.com/oven-sh/bun");
}

#[test]
fn row_5_failed_with_no_build_shows_the_message_and_hides_try_again() {
    let r = mount(make_view(Value::Null, failed("no-build", "Fleet has no Bun build for this machine."), json!([])), Some("false"));
    assert_eq!(panel(&r), Some(rt::Panel::Failed));
    assert!(!has(&r, "mods-retry"));
}

#[test]
fn row_5_failed_try_again_posts_the_install_again() {
    let mut r = mount(make_view(Value::Null, failed("other", "The download stopped."), json!([])), Some("false"));
    serve_route(&r, &format!("POST {RUNTIME}/install"), 200, make_view(Value::Null, job("downloading"), json!([])));
    click(&mut r.rt, "mods-retry");
    answered(&mut r, "install");
    assert_eq!(called(&r, "install"), 1);
    assert_eq!(panel(&r), Some(rt::Panel::Progress));
}

#[test]
fn row_6_on_at_once_with_a_bun_already_there_switching_on_just_writes_the_preference() {
    let mut r = mount(make_view(installed("1.4.2"), Value::Null, json!([])), Some("false"));
    click(&mut r.rt, "plugin-switch-mods");
    r.wait_put("Mods", "true");
    assert_eq!(mods_puts(&r), 1);
    assert_eq!(called(&r, "install"), 0);
    assert_ne!(panel(&r), Some(rt::Panel::Confirm));
}

// ─── mods-and-skills-plugins.test.ts ────────────────────────────────────────────

#[test]
fn both_are_in_the_registry_with_a_settings_page_and_built_in_skills_has_no_switch_of_its_own() {
    let ids: Vec<&str> = p::CORE.iter().map(|x| x.id).collect();
    assert_eq!(ids, ["mods", "built-in-skills"]);
    assert_eq!(p::find(&crate::plugins::register(Default::default()), "mods").unwrap().page_title, "Mods");
    assert_eq!(p::find(&crate::plugins::register(Default::default()), "built-in-skills").unwrap().page_title, "Built-in skills");
    assert!(p::find(&crate::plugins::register(Default::default()), "built-in-skills").unwrap().switchless);
    assert!(p::find(&crate::plugins::register(Default::default()), "built-in-skills").unwrap().enabled(&Default::default()));
}

#[test]
fn lists_both_in_settings_plugins_only_mods_has_a_switch() {
    let r = mount(view(), None);
    assert!(has(&r, "plugin-row-mods") && has(&r, "plugin-row-built-in-skills"));
    assert!(has(&r, "plugin-switch-mods"));
    assert!(!has(&r, "plugin-switch-built-in-skills"));
    assert!(has(&r, "plugin-open-built-in-skills"), "an Open button stands in for the switch");
}

#[test]
fn the_mods_switch_is_the_mods_feature_preference_not_the_list_of_plugins_that_are_off() {
    let mut r = mount(make_view(installed("1.4.2"), Value::Null, json!([])), None);
    assert!(!on(&r));
    click(&mut r.rt, "plugin-switch-mods");
    r.wait_put("Mods", "true");
    assert_eq!(mods_puts(&r), 1);
    assert!(on(&r));
    assert!(r.puts().iter().all(|(k, _)| k != "plugins.disabled"));
}

#[test]
fn an_existing_mods_preference_decides_the_switch_whatever_plugins_disabled_says() {
    let mut r = unread(&[("Mods", "true"), ("plugins.disabled", "[\"mods\"]")]);
    r.rt.update(|s, _| s.open_settings("plugins", None));
    r.wait("the preferences", |s| s.pickers.prefs.fetched);
    assert!(on(&r));
    assert!(r.rt.state.plugin_enabled("mods"));
}

#[test]
fn turning_mods_on_with_no_bun_asks_first_says_what_will_be_installed_and_writes_nothing_yet() {
    let mut r = mount(view(), None);
    click(&mut r.rt, "plugin-switch-mods");
    r.frames();
    assert!(r.rt.state.settings.mods_runtime.confirming);
    assert_eq!(panel(&r), Some(rt::Panel::Confirm));
    let words = r.words();
    assert!(words.contains("Fleet downloads Bun 1.4.2 (about 35 MB) from github.com/oven-sh/bun"));
    assert!(words.contains("installs it in ~/.weave/runtimes/bun/1.4.2"));
    assert!(!on(&r));
    assert_eq!(mods_puts(&r), 0);
}

#[test]
fn set_plugin_enabled_everywhere_for_mods_goes_through_the_same_switch() {
    let mut r = mount(make_view(installed("1.4.2"), Value::Null, json!([])), None);
    r.rt.update(|s, _| s.set_plugin_enabled("mods", true));
    r.wait_put("Mods", "true");
    assert_eq!(mods_puts(&r), 1);
    assert!(r.puts().iter().all(|(k, _)| k != "plugins.disabled"));
}

// ─── mods-runtime.test.ts ───────────────────────────────────────────────────────

#[test]
fn merges_a_downloading_event_s_job_into_the_view_without_a_request() {
    let mut r = mount(make_view(Value::Null, job("downloading"), json!([])), Some("true"));
    let before = gets(&r);
    let mut j = job("downloading");
    j["bytesReceived"] = json!(20_000_000);
    event(&mut r, Kind::ModsRuntime, json!({ "job": j }));
    assert_eq!(r.rt.state.settings.mods_runtime.view.as_ref().unwrap().job.as_ref().unwrap().bytes_received, Some(20_000_000));
    assert!(r.words().contains("20 of 35 MB"));
    assert_eq!(gets(&r), before, "no request for progress");
}

#[test]
fn refetches_the_view_when_the_job_changes_phase() {
    let mut r = mount(make_view(Value::Null, job("downloading"), json!([])), Some("true"));
    let before = gets(&r);
    serve_route(&r, &format!("GET {RUNTIME}"), 200, make_view(Value::Null, job("verifying"), json!([])));
    event(&mut r, Kind::ModsRuntime, json!({ "job": job("verifying") }));
    // The event already shows the new phase in place; the proof of the re-read is the request.
    let fake = r.fake.clone();
    r.wait("the view to be read again", move |_| {
        fake.lock().unwrap().calls.iter().filter(|(m, p, _)| m == "GET" && p == RUNTIME).count() > before
    });
    assert_eq!(gets(&r), before + 1);
}

#[test]
fn reads_the_preferences_again_when_the_server_turned_the_switch_off_after_a_failed_install() {
    let mut r = mount(view(), Some("true"));
    assert!(on(&r));
    r.fake.lock().unwrap().prefs.insert("Mods".into(), "false".into());
    event(&mut r, Kind::ModsChanged, json!({ "reason": "switch" }));
    r.wait("the switch to show off", |s| !s.plugin_enabled("mods"));
}

#[test]
fn install_and_cancel_post_and_take_the_view_the_server_answers() {
    let mut r = mount(view(), None);
    serve_route(&r, &format!("POST {RUNTIME}/install"), 200, make_view(Value::Null, job("downloading"), json!([])));
    serve_route(&r, &format!("POST {RUNTIME}/cancel"), 200, make_view(Value::Null, job("downloading"), json!([])));
    r.rt.update(|s, _| s.mods_runtime_action(rt::Action::Install));
    answered(&mut r, "install");
    r.rt.update(|s, _| s.mods_runtime_action(rt::Action::Cancel));
    answered(&mut r, "cancel");
    let calls: Vec<String> =
        r.fake.lock().unwrap().calls.iter().filter(|c| c.1.starts_with(RUNTIME) && c.0 == "POST").map(|c| c.1.clone()).collect();
    assert_eq!(calls, [format!("{RUNTIME}/install"), format!("{RUNTIME}/cancel")]);
    assert_eq!(r.rt.state.settings.mods_runtime.view.as_ref().unwrap().job.as_ref().unwrap().phase, "downloading");
}

// ─── Mods' own page (ModsPluginPage.vue) ────────────────────────────────────────

#[test]
fn the_mods_page_says_what_mods_are_and_links_to_skills_and_tools() {
    let mut r = mount_on("plugin:mods", view(), Some("true"));
    assert!(has(&r, "plugin-config-mods"));
    assert!(r.words().contains("Agents can write small add-ons for Fleet"));
    click(&mut r.rt, "mods-plugin-open-mods");
    r.frames();
    assert_eq!(r.rt.state.settings.page, Page::Skills);
    assert_eq!(r.rt.state.settings.anchor.as_deref(), Some("mods"));
}

#[test]
fn the_mods_page_draws_the_same_panel_above_its_settings() {
    let r = mount_on("plugin:mods", make_view(installed("1.4.2"), Value::Null, json!([])), Some("true"));
    assert_eq!(r.text("mods-ready-text"), "Mods on \u{b7} Bun 1.4.2");
    assert!(has(&r, "plugin-page-panel"));
}
