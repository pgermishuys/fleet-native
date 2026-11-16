//! Reading the machines (`fleet::machines`, `machine_cache`, `machine_poll`): the cases of weave-fleet's
//! `stores/__tests__/machines.test.ts`, `lib/__tests__/machine-feed.test.ts` and `machines-open-in-place.test.ts` that
//! belong to this app, under Fleet's names, plus ours. Servers here are tiny fakes on a loopback port.
use crate::fleet::machine::Machine;
use crate::fleet::machine_cache;
use crate::fleet::machine_poll::read_one;
use crate::fleet::machines::{Entry, Listed, Machines, Source, parse_machine_list, read_machine};
use crate::fleet::{auth, sessions};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

const FALCON_TOKEN: &str = "falcon-token-0123456789";
const FALCON_ID: &str = "ffffffffffffffffffffffffffffffff";

/// What a fake machine was asked: the path with its query, and the `Authorization` header.
type Seen = Arc<Mutex<Vec<(String, Option<String>)>>>;

/// A machine at a loopback port. `route` answers a path (no query) with a status and a body, or None for a 404.
fn serve(route: impl Fn(&str) -> Option<(u16, String)> + Send + Sync + 'static) -> (String, Seen) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let seen: Seen = Arc::default();
    let (log, route) = (seen.clone(), Arc::new(route));
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let (log, route) = (log.clone(), route.clone());
            std::thread::spawn(move || {
                let mut buf = [0u8; 8192];
                let n = stream.read(&mut buf).unwrap_or(0);
                let text = String::from_utf8_lossy(&buf[..n]).to_string();
                let target = text.lines().next().unwrap_or("").split(' ').nth(1).unwrap_or("").to_string();
                let bearer = text.lines().find_map(|l| {
                    l.to_ascii_lowercase().starts_with("authorization:").then(|| l.split_once(':').unwrap().1.trim().to_string())
                });
                log.lock().unwrap().push((target.clone(), bearer));
                let (status, body) = route(target.split('?').next().unwrap()).unwrap_or((404, "{}".into()));
                let reply = format!(
                    "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(reply.as_bytes());
            });
        }
    });
    (base, seen)
}

fn row(id: &str, title: &str) -> Value {
    json!({
        "session": { "id": id, "title": title, "time": { "created": 1_000, "updated": 2_000 } },
        "instanceId": format!("inst-{id}"), "sessionStatus": "idle", "activityStatus": "idle",
        "retentionStatus": "active", "parentSessionId": null,
    })
}

fn info(id: &str, name: &str) -> Value {
    json!({ "id": id, "name": name, "hostName": name, "os": "macos", "version": "0.72.0", "apiVersion": 1,
        "authMode": "token", "remoteReachable": true, "requiresToken": true })
}

/// A machine that wants `FALCON_TOKEN` for everything but its answers to `routes`.
fn falcon_server(sessions: Value, extra: impl Fn(&str) -> Option<(u16, String)> + Send + Sync + 'static) -> (String, Seen) {
    serve(move |path| match path {
        "/api/sessions" => Some((200, sessions.to_string())),
        "/api/machine" => Some((200, info(FALCON_ID, "falcon").to_string())),
        other => extra(other),
    })
}

fn machine_json(base: &str) -> Value {
    json!({ "id": FALCON_ID, "name": "falcon", "baseUrl": base, "os": "macos", "status": "online", "addedAt": "2026-09-26T00:00:00Z",
        "token": FALCON_TOKEN, "addresses": [{ "url": base, "kind": "lan", "learned": false }] })
}

fn listed(base: &str) -> Listed {
    parse_machine_list(200, &json!({ "machines": [machine_json(base)] }).to_string())
}

/// A store that knows falcon at `base`.
fn knows_falcon(base: &str) -> Machines {
    let mut m = Machines::default();
    m.apply_list(listed(base));
    m
}

fn titles(m: &Machines) -> Vec<String> {
    m.others[FALCON_ID].sessions.iter().map(|s| s.title.clone()).collect()
}

/// One read of falcon, applied: what `spawn_read` does, in this thread.
fn refresh(m: &mut Machines) -> Option<bool> {
    let job = m.begin_reads().into_iter().find(|j| j.key == FALCON_ID)?;
    Some(match read_one(&job) {
        Ok(read) => m.apply_read(FALCON_ID, read, 5_000),
        Err(why) => {
            m.mark_unreachable(FALCON_ID, why);
            false
        }
    })
}

fn nobody_home() -> String {
    // A port nothing listens on any more.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    format!("http://{}", listener.local_addr().unwrap())
}

fn app_row(id: &str, title: &str) -> crate::mock::Session {
    sessions::parse_sessions(&json!([row(id, title)]), 0).remove(0)
}

// ─── the list on home ───────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn takes_home_s_list_over_the_cached_one() {
    let cached = {
        let mut old = knows_falcon("http://127.0.0.1:9");
        old.others.entry(FALCON_ID.into()).or_default().loaded_at = Some(1);
        let mut gone = Machines::default();
        gone.others.insert("stale".into(), Default::default());
        gone.others.get_mut("stale").unwrap().loaded_at = Some(1);
        old.others.extend(gone.others);
        machine_cache::parse(&machine_cache::to_json(&old))
    };
    let mut m = Machines::default();
    m.adopt_cache(cached);
    assert!(m.others.contains_key("stale"), "the cached list shows before home answers");

    let (base, _) = falcon_server(json!([]), |_| None);
    m.apply_list(listed(&base));

    assert_eq!(m.source, Source::Owner);
    assert_eq!(m.list.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(), [FALCON_ID]);
    assert!(!m.others.contains_key("stale"), "a machine home doesn't list is dropped");
    assert_eq!(auth::token_for(&base).map(|t| t.expose().to_string()).as_deref(), Some(FALCON_TOKEN));
}

#[test]
fn a_fleet_that_can_t_list_machines_has_none_and_shows_no_error() {
    for (status, body) in [(404, "{}"), (500, "boom"), (200, "not json"), (200, r#"{"machines":"x"}"#), (200, "{}")] {
        let l = parse_machine_list(status, body);
        assert_eq!((l.source, l.machines.len()), (Source::Cannot, 0), "{status} {body}");
    }
    let mut m = knows_falcon("http://127.0.0.1:9");
    m.apply_list(parse_machine_list(404, ""));
    assert!(!m.has_machines());
}

#[test]
fn every_token_present_is_an_owner_and_any_missing_is_a_device() {
    let with = machine_json("http://127.0.0.1:1");
    let mut without = machine_json("http://127.0.0.1:2");
    without["id"] = json!("aaaa");
    without["token"] = Value::Null;
    let owner = parse_machine_list(200, &json!({ "machines": [with.clone()] }).to_string());
    assert_eq!(owner.source, Source::Owner);
    let device = parse_machine_list(200, &json!({ "machines": [with, without] }).to_string());
    assert_eq!(device.source, Source::Device);
    assert_eq!(device.machines.len(), 1, "a machine with no token can't be reached, so it isn't listed");
    // Nothing listed is still the owner's list: an empty one.
    assert_eq!(parse_machine_list(200, r#"{"machines":[]}"#).source, Source::Owner);
}

#[test]
fn a_machine_that_is_no_longer_listed_lets_go_of_its_token() {
    let (base, _) = falcon_server(json!([]), |_| None);
    let mut m = knows_falcon(&base);
    assert!(auth::token_for(&base).is_some());
    m.apply_list(parse_machine_list(200, r#"{"machines":[]}"#));
    assert!(auth::token_for(&base).is_none());
}

// ─── reading one machine ────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn lists_another_machine_s_sessions_with_its_token_and_remembers_where_they_live() {
    let (base, seen) = falcon_server(json!([row("remote-1", "Notarisation retry loop")]), |_| None);
    let mut m = knows_falcon(&base);

    refresh(&mut m);

    assert_eq!(titles(&m), ["Notarisation retry loop"]);
    assert_eq!(m.others[FALCON_ID].error, None);
    assert_eq!(m.session_machines.get("remote-1").map(String::as_str), Some(FALCON_ID));
    assert_eq!(m.others[FALCON_ID].loaded_at, Some(5_000));
    // Three calls, each with the machine's token (and nothing else that would identify the app).
    let seen = seen.lock().unwrap();
    let paths: Vec<&str> = seen.iter().map(|(p, _)| p.as_str()).collect();
    for want in ["/api/sessions?limit=100&offset=0", "/api/projects", "/api/machine"] {
        assert!(paths.contains(&want), "{paths:?}");
    }
    assert!(seen.iter().all(|(_, auth)| auth.as_deref() == Some(&*format!("Bearer {FALCON_TOKEN}"))), "{seen:?}");
}

#[test]
fn gives_the_read_a_request_with_the_machine_s_key_and_no_cookies() {
    let (base, seen) = falcon_server(json!([]), |_| None);
    let mut m = knows_falcon(&base);
    refresh(&mut m);
    // A request never carries a cookie or the CSRF header a browser would send to home.
    let (home, home_seen) = serve(|_| Some((200, "[]".into())));
    let _ = crate::fleet::http::get(&home, "/api/sessions");
    assert_eq!(seen.lock().unwrap().len(), 3);
    assert_eq!(home_seen.lock().unwrap()[0].1, None, "home's request never gets falcon's token");
}

#[test]
fn shows_a_session_just_started_on_it_in_its_list_at_once_before_its_next_list_says_so() {
    let (base, _) = falcon_server(json!([row("remote-1", "Older")]), |_| None);
    let mut m = knows_falcon(&base);
    refresh(&mut m);

    m.upsert_session(FALCON_ID, app_row("remote-2", "Just started"));
    // Newest first, and the older one stays.
    assert_eq!(titles(&m), ["Just started", "Older"]);
    assert_eq!(m.session_machines["remote-2"], FALCON_ID);

    // The same session again updates its row, never doubles it.
    m.upsert_session(FALCON_ID, app_row("remote-2", "Renamed"));
    assert_eq!(titles(&m), ["Renamed", "Older"]);

    // A patch changes one listed row at once; a session it doesn't list is left alone.
    m.patch_session(FALCON_ID, "remote-1", |s| s.title = "Patched".into());
    m.patch_session(FALCON_ID, "nobody", |_| panic!("not listed"));
    assert_eq!(titles(&m), ["Renamed", "Patched"]);
}

#[test]
fn keeps_the_last_list_and_says_why_when_a_machine_goes_away() {
    let (base, _) = falcon_server(json!([row("remote-1", "Kept")]), |_| None);
    let mut m = knows_falcon(&base);
    refresh(&mut m);
    assert!(m.is_answering(FALCON_ID));

    // The machine goes away: the same name at an address nothing answers.
    let mut gone = listed(&nobody_home());
    gone.machines[0].name = "falcon".into();
    m.apply_list(gone);
    refresh(&mut m);

    assert_eq!(titles(&m), ["Kept"]);
    assert_eq!(m.others[FALCON_ID].error.as_deref(), Some("Can't reach falcon."));
    assert!(!m.is_answering(FALCON_ID));
}

#[test]
fn still_lists_a_machine_s_sessions_after_a_reload_while_it_s_away() {
    let (base, _) = falcon_server(json!([row("remote-1", "Kept")]), |_| None);
    let mut m = knows_falcon(&base);
    refresh(&mut m);
    let file = machine_cache::to_json(&m);

    let mut reloaded = Machines::default();
    reloaded.adopt_cache(machine_cache::parse(&file));
    reloaded.apply_list(listed(&nobody_home()));
    refresh(&mut reloaded);

    assert_eq!(titles(&reloaded), ["Kept"]);
    assert_eq!(reloaded.others[FALCON_ID].error.as_deref(), Some("Can't reach falcon."));
    assert_eq!(reloaded.session_machines["remote-1"], FALCON_ID, "where a session lives survives the restart");
}

#[test]
fn keeps_each_session_s_project_pin_and_lineage_and_the_machine_s_projects_across_a_reload() {
    let projects = json!([
        { "id": "p-lighthouse", "name": "Lighthouse", "type": "user", "position": 1, "description": null },
        { "id": "p-scratch", "name": "Scratch", "type": "scratch", "position": 0, "description": null },
    ]);
    let mut pinned = row("pinned", "Spring planting");
    pinned["projectId"] = json!("p-lighthouse");
    pinned["projectName"] = json!("Lighthouse");
    pinned["pinOrder"] = json!(1);
    let mut fork = row("fork", "Fork of it");
    fork["projectId"] = json!("p-lighthouse");
    fork["projectName"] = json!("Lighthouse");
    fork["forkedFromSessionId"] = json!("pinned");
    fork["spawnKind"] = json!("fork");
    fork["runningWorkCount"] = json!(2);
    let (base, _) = falcon_server(json!([pinned, fork]), move |p| (p == "/api/projects").then(|| (200, projects.to_string())));
    let mut m = knows_falcon(&base);
    refresh(&mut m);

    let mut reloaded = Machines::default();
    reloaded.adopt_cache(machine_cache::parse(&machine_cache::to_json(&m)));
    reloaded.apply_list(listed(&nobody_home()));
    refresh(&mut reloaded);

    let [pinned, fork] = &reloaded.others[FALCON_ID].sessions[..] else { panic!("two rows") };
    assert_eq!((pinned.listing.project_id.as_deref(), pinned.listing.project_name.as_deref()), (Some("p-lighthouse"), Some("Lighthouse")));
    assert_eq!(pinned.listing.pin_order, Some(1.0));
    assert_eq!((fork.listing.forked_from.as_deref(), fork.listing.spawn_kind.as_deref()), (Some("pinned"), Some("fork")));
    assert_eq!(fork.row.running_work, 2);
    let p = &reloaded.others[FALCON_ID].projects;
    assert_eq!(
        p.iter().map(|x| (x.id.as_str(), x.name.as_str(), x.position)).collect::<Vec<_>>(),
        [("p-lighthouse", "Lighthouse", 1), ("p-scratch", "Scratch", 0)]
    );
    assert_eq!(p[1].kind, crate::fleet::groups::ProjectKind::Scratch);
}

#[test]
fn still_lists_the_sessions_when_the_machine_s_projects_don_t_answer() {
    let mut r = row("remote-1", "Kept");
    r["projectId"] = json!("p-lighthouse");
    let (base, _) = falcon_server(json!([r]), |p| (p == "/api/projects").then(|| (500, r#"{"error":"no"}"#.into())));
    let mut m = knows_falcon(&base);
    refresh(&mut m);
    assert_eq!(titles(&m), ["Kept"]);
    assert!(m.others[FALCON_ID].projects.is_empty());
    assert_eq!(m.others[FALCON_ID].error, None);
}

#[test]
fn keeps_what_each_machine_says_it_can_run_and_nothing_for_a_fleet_too_old_to_say() {
    let mut with = info(FALCON_ID, "falcon");
    with["capabilities"] = json!({
        "harnesses": [{ "type": "opencode", "name": "OpenCode", "available": true, "enabled": true, "version": "1.18.32" }],
        "sessions": { "working": 2, "needsYou": 1 },
    });
    let body = with.to_string();
    let (base, _) = serve(move |p| match p {
        "/api/sessions" => Some((200, "[]".into())),
        "/api/machine" => Some((200, body.clone())),
        _ => None,
    });
    let mut m = knows_falcon(&base);
    refresh(&mut m);
    let entries = m.entries();
    let caps = entries.iter().find(|e| e.key == FALCON_ID).and_then(Entry::capabilities).expect("capabilities");
    assert_eq!((caps.working, caps.needs_you), (2, 1));
    let harness = &caps.harnesses.as_ref().unwrap()[0];
    assert_eq!((harness.kind.as_str(), harness.version.as_deref()), ("opencode", Some("1.18.32")));

    // Home answers without the field: an older Fleet.
    let old = crate::fleet::machines::Info::from_json(&info("home-id", "atlas")).unwrap();
    assert_eq!(old.capabilities, None);
    m.home = Some(old);
    assert!(m.entries()[0].capabilities().is_none());
}

#[test]
fn forgets_a_machine_without_touching_its_sessions() {
    let (base, _) = falcon_server(json!([row("remote-1", "Kept")]), |_| None);
    let mut m = knows_falcon(&base);
    refresh(&mut m);

    assert!(m.forget(FALCON_ID));

    assert!(!m.has_machines());
    assert!(!m.others.contains_key(FALCON_ID) && m.identities.is_empty());
    assert!(auth::token_for(&base).is_none(), "its token goes with it");
    assert_eq!(m.session_machines.len(), 1, "the ids stay, so a link to one still says where it was");
    assert!(!m.forget(FALCON_ID), "forgetting twice is nothing");
}

#[test]
fn finds_a_machine_by_its_id_home_a_listed_one_or_none() {
    let mut m = knows_falcon("http://127.0.0.1:9");
    m.home = crate::fleet::machines::Info::from_json(&info("hhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhh", "atlas"));
    assert_eq!(m.key_of_machine("hhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhh").as_deref(), Some("home"));
    assert_eq!(m.key_of_machine(FALCON_ID).as_deref(), Some(FALCON_ID));
    assert_eq!(m.key_of_machine("cccccccccccccccccccccccccccccccc"), None);
}

#[test]
fn home_is_first_and_called_this_machine_until_it_answers() {
    let mut m = knows_falcon("http://127.0.0.1:9");
    let names: Vec<_> = m.entries().iter().map(|e| (e.key.clone(), e.name.clone(), e.is_home)).collect();
    assert_eq!(names, [("home".into(), "This machine".into(), true), (FALCON_ID.into(), "falcon".into(), false)]);
    m.home = crate::fleet::machines::Info::from_json(&info("h", "atlas"));
    assert_eq!(m.entries()[0].name, "atlas");
}

// ─── the machine feed's read, and the words of a failure ────────────────────────────────────────────────────────────

#[test]
fn marks_the_machine_unreachable_when_the_read_fails_keeping_the_last_read() {
    let (base, _) = falcon_server(json!([row("remote-1", "Kept")]), |_| None);
    let mut m = knows_falcon(&base);
    refresh(&mut m);

    m.mark_unreachable(FALCON_ID, "falcon answered 500.".into());
    assert_eq!(titles(&m), ["Kept"]);
    assert_eq!(m.others[FALCON_ID].error.as_deref(), Some("falcon answered 500."));
    assert!(!m.others[FALCON_ID].loading, "a failed read ends the loading");

    // A good read clears it, says it was down, and stamps the time.
    let back = refresh(&mut m);
    assert_eq!(back, Some(true), "back after an error");
    assert_eq!(m.others[FALCON_ID].error, None);
    assert_eq!(refresh(&mut m), Some(false), "not back: it never was away");
}

#[test]
fn says_a_machine_its_feed_can_t_read_isn_t_answering() {
    let mut m = knows_falcon("http://127.0.0.1:9");
    assert!(m.is_answering("home") && m.is_answering(FALCON_ID), "nothing wrong yet");
    m.mark_unreachable(FALCON_ID, "Can't reach falcon.".into());
    assert!(m.is_answering("home"), "home always answers");
    assert!(!m.is_answering(FALCON_ID));
}

#[test]
fn the_words_of_a_failure() {
    let ok = || Ok((200, "[]".to_string()));
    let said = |sessions| read_machine("falcon", sessions, ok(), ok(), 0).err();
    assert_eq!(said(Ok((401, String::new()))).as_deref(), Some("falcon didn't accept the token."));
    assert_eq!(said(Ok((503, String::new()))).as_deref(), Some("falcon answered 503."));
    assert_eq!(said(Ok((500, r#"{"error":"disk full"}"#.into()))).as_deref(), Some("disk full"));
    assert_eq!(said(Ok((500, r#"{"message":"later"}"#.into()))).as_deref(), Some("later"));
    assert_eq!(said(Err("connection refused".into())).as_deref(), Some("Can't reach falcon."));
    assert!(said(Ok((200, "not a list".into()))).is_some());
}

#[test]
fn a_wrong_token_says_it_didn_t_accept_it_and_keeps_the_list() {
    let (base, _) = serve(|_| Some((401, r#"{"error":"nope"}"#.into())));
    let mut m = knows_falcon(&base);
    refresh(&mut m);
    assert_eq!(m.others[FALCON_ID].error.as_deref(), Some("falcon didn't accept the token."));
}

#[test]
fn one_read_at_a_time_per_machine() {
    let mut m = knows_falcon("http://127.0.0.1:9");
    assert_eq!(m.begin_reads().len(), 1);
    assert!(m.begin_reads().is_empty(), "the first is still out");
    m.mark_unreachable(FALCON_ID, "x".into());
    assert_eq!(m.begin_reads().len(), 1);
}

// ─── the cache ──────────────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn the_cache_keeps_what_a_tree_draws_and_never_a_token() {
    let (base, _) = falcon_server(json!([row("remote-1", "Kept")]), |_| None);
    let mut m = knows_falcon(&base);
    refresh(&mut m);
    let text = machine_cache::to_json(&m);
    assert!(!text.contains(FALCON_TOKEN) && !text.contains(&base), "no token, no address: {text}");
    let v: Value = serde_json::from_str(&text).unwrap();
    let saved = &v["machines"][FALCON_ID]["sessions"][0];
    assert_eq!(saved["session"]["title"], "Kept");
    for kept in ["session", "sessionStatus", "activityStatus", "retentionStatus", "projectId", "pinOrder", "runningWorkCount"] {
        assert!(saved.get(kept).is_some(), "{kept}");
    }
    for dropped in ["instanceId", "workspaceDirectory", "totalTokens", "capabilities"] {
        assert!(saved.get(dropped).is_none(), "{dropped}");
    }
}

#[test]
fn a_corrupt_cache_reads_as_empty() {
    for text in ["", "not json", "[]", r#"{"machines":3}"#, r#"{"machines":{"x":{"sessions":"no"}}}"#] {
        let cached = machine_cache::parse(text);
        assert!(cached.others.values().all(|o| o.sessions.is_empty()), "{text}");
        assert!(cached.session_machines.is_empty());
    }
}

#[test]
fn a_machine_that_was_never_read_is_not_in_the_cache() {
    let m = knows_falcon("http://127.0.0.1:9");
    let mut m = m;
    m.begin_reads();
    assert!(machine_cache::parse(&machine_cache::to_json(&m)).others.is_empty());
}

#[test]
fn the_cache_file_is_written_whole_and_read_back() {
    let dir = std::env::temp_dir().join(format!("fleet-native-machine-cache-{}", std::process::id()));
    let mut m = knows_falcon("http://127.0.0.1:9");
    let job = m.begin_reads().remove(0);
    let read = read_machine(&job.name, Ok((200, json!([row("a", "A")]).to_string())), Err("x".into()), Err("x".into()), 0).unwrap();
    m.apply_read(FALCON_ID, read, 7);
    machine_cache::write(&dir, &machine_cache::to_json(&m)).unwrap();
    let back = machine_cache::load(&dir);
    assert_eq!(back.others[FALCON_ID].sessions[0].title, "A");
    assert_eq!(back.others[FALCON_ID].loaded_at, Some(7));
    assert_eq!(machine_cache::load(&dir.join("missing")).others.len(), 0);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_machine_a_listed_address_uses_shows_its_scheme_and_nothing_else() {
    let mut l = listed("https://falcon.example.ts.net");
    l.machines[0]
        .addresses
        .push(crate::fleet::machine::Machine::from_json(&machine_json("http://100.64.0.9:2113")).unwrap().addresses.remove(0));
    let mut m = Machines::default();
    m.apply_list(l);
    assert_eq!(m.address_schemes(), ["http", "https"]);
    let _: &Machine = &m.list[0];
}

// ─── the poll, end to end ───────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn the_poll_reads_home_s_list_and_identity_then_each_machine_and_hands_them_to_the_state() {
    let (falcon, _) = falcon_server(json!([row("remote-1", "Harbour lights")]), |_| None);
    let machines = json!({ "machines": [machine_json(&falcon)] }).to_string();
    let home_info = info("hhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhh", "atlas").to_string();
    let (home, home_seen) = serve(move |p| match p {
        "/api/machines" => Some((200, machines.clone())),
        "/api/machine" => Some((200, home_info.clone())),
        _ => None,
    });
    let (jobs, queue) = std::sync::mpsc::channel::<crate::fleet::link::Job>();
    let jobs = Mutex::new(jobs);
    let post = crate::fleet::link::Post::new(move |job| {
        let _ = jobs.lock().unwrap().send(job);
    });
    let mut rt = crate::tests::app();
    let _poll = crate::fleet::machine_poll::start(home, post);

    let end = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while std::time::Instant::now() < end && rt.state.machines.others.get(FALCON_ID).is_none_or(|o| o.loaded_at.is_none()) {
        while let Ok(job) = queue.try_recv() {
            rt.update(job);
        }
        rt.frame(1.0 / 60.0);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    let m = &rt.state.machines;
    assert_eq!(m.source, Source::Owner);
    assert_eq!(m.home.as_ref().map(|h| h.name.as_str()), Some("atlas"));
    assert_eq!(titles(m), ["Harbour lights"]);
    assert!(rt.state.machine_schemes_told);
    // Home was asked for its list and identity, and nothing is imported.
    let asked: Vec<String> = home_seen.lock().unwrap().iter().map(|(p, _)| p.clone()).collect();
    assert!(asked.contains(&"/api/machines".to_string()) && asked.contains(&"/api/machine".to_string()), "{asked:?}");
    assert!(!asked.iter().any(|p| p.contains("import")), "{asked:?}");
}
