//! Component 16, piece AG: Settings → Skills & tools, the page, the install target and Installed skills. Fleet's
//! `install-target.test.ts`, `use-skills.spec.ts` (fetch, check, update, remove) and `SkillsToolsPage.test.ts` under its names,
//! then the card's words, the confirm-remove flow and that every request goes to the machine Settings shows.
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::settings::install_target::{InstallTarget, Kind, to_install_target};
use crate::fleet::settings::skills::{self, Inner, InstalledSkill, Source, Tab, parse_github_url, parse_skills};
use crate::tests::{FRAME, app, click};
use crate::{Rail, State, test_wait};
use rune::Runtime;
use serde_json::{Value, json};
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

const GLOBAL: InstallTarget = InstallTarget::Global;

fn project(path: &str) -> InstallTarget {
    InstallTarget::Project(path.into())
}

/// `createSkillEntry`.
pub(crate) fn entry(name: &str, over: Value) -> Value {
    let mut v = json!({
        "name": name, "source": 1, "repoUrl": format!("https://github.com/example/{name}"), "ref": "main", "subPath": null,
        "localPath": null, "targetHarnesses": ["opencode"], "scope": "global", "projectPath": null,
        "installedPaths": [format!("/home/user/.config/opencode/skills/{name}")],
        "installedAt": "2026-01-01T00:00:00Z", "updatedAt": "2026-01-01T00:00:00Z",
    });
    for (k, x) in over.as_object().into_iter().flatten() {
        v[k] = x.clone();
    }
    v
}

// ─── install-target.test.ts ─────────────────────────────────────────────────────

#[test]
fn reads_the_api_s_scope_fields_treating_anything_but_a_project_with_a_path_as_global() {
    assert_eq!(to_install_target(Some("project"), Some("/src/app")), project("/src/app"));
    assert_eq!(to_install_target(Some("project"), None), GLOBAL);
    assert_eq!(to_install_target(Some("global"), Some("/src/app")), GLOBAL);
    assert_eq!(to_install_target(None, None), GLOBAL);
}

#[test]
fn compares_targets_by_scope_and_repository() {
    let app = project("/src/app");
    assert!(GLOBAL.is_same(&InstallTarget::Global));
    assert!(app.is_same(&project("/src/app")));
    assert!(!app.is_same(&project("/src/other")));
    assert!(!app.is_same(&GLOBAL));
}

#[test]
fn round_trips_through_a_select_key_including_windows_paths() {
    let windows = project("C:\\src\\app");
    assert_eq!(InstallTarget::from_key(&windows.key()), windows);
    assert_eq!(InstallTarget::from_key(&GLOBAL.key()), GLOBAL);
}

#[test]
fn builds_request_fields() {
    assert_eq!(GLOBAL.body(), json!({ "scope": "global", "projectPath": null }));
    assert_eq!(project("/src/app").body(), json!({ "scope": "project", "projectPath": "/src/app" }));
    assert_eq!(project("/src/app").query(), "scope=project&projectPath=%2Fsrc%2Fapp");
    assert_eq!(GLOBAL.query(), "scope=global");
}

#[test]
fn labels_a_project_by_its_folder_name_on_any_os() {
    assert_eq!(GLOBAL.label(), "Global");
    assert_eq!(project("/home/me/src/weave-fleet/").label(), "weave-fleet");
    assert_eq!(project("C:\\src\\weave-fleet").label(), "weave-fleet");
}

#[test]
fn says_where_each_install_lands() {
    assert_eq!(GLOBAL.destination(Kind::Skills), "~/.config/opencode/skills/");
    assert_eq!(GLOBAL.destination(Kind::Tools), "~/.config/opencode/tools/");
    assert_eq!(GLOBAL.destination(Kind::Config), "~/.config/opencode/opencode.json");
    assert_eq!(project("/src/app").destination(Kind::Skills), "/src/app/.opencode/skills/");
    assert_eq!(project("/src/app").destination(Kind::Config), "/src/app/opencode.json");
    assert_eq!(GLOBAL.audience(), "Every session");
    assert_eq!(project("/src/app").audience(), "Sessions in this repository");
}

// ─── use-skills.spec.ts: the reading, which needs no server ─────────────────────

#[test]
fn fetches_and_transforms_skills_from_the_skills_endpoint() {
    let body = json!({ "skills": [
        entry("skill-one", json!({})),
        entry("skill-two", json!({ "source": 2, "localPath": "/path/to/skill", "repoUrl": null })),
    ]});
    let list = parse_skills(&body).unwrap();
    assert_eq!(list.len(), 2);
    let one = &list[0];
    assert_eq!((one.name.as_str(), one.source, &one.target), ("skill-one", Source::GitHub, &GLOBAL));
    assert_eq!(one.repo_url.as_deref(), Some("https://github.com/example/skill-one"));
    assert_eq!(one.installed_paths, ["/home/user/.config/opencode/skills/skill-one"]);
    // Backward compatibility fields: a GitHub install's address is its description and its path.
    assert_eq!(one.description.as_deref(), Some("https://github.com/example/skill-one"));
    assert_eq!(one.path.as_deref(), Some("https://github.com/example/skill-one"));
    let two = &list[1];
    assert_eq!(
        (two.source, two.local_path.as_deref(), two.path.as_deref()),
        (Source::Local, Some("/path/to/skill"), Some("/path/to/skill"))
    );
    assert_eq!(two.description, None);
}

#[test]
fn handles_empty_skills_list() {
    assert_eq!(parse_skills(&json!({ "skills": [] })).unwrap(), vec![]);
}

#[test]
fn handles_missing_data_response() {
    assert_eq!(parse_skills(&Value::Null).unwrap_err(), "No data returned");
    assert_eq!(skills::parse_update_check(&Value::Null).unwrap_err(), "No data returned");
}

#[test]
fn takes_the_reason_a_refusal_gives() {
    assert_eq!(skills::extract_error("Network error", "Failed to fetch skills"), "Network error");
    assert_eq!(skills::extract_error(r#"{"detail":"Not allowed","title":"x"}"#, "f"), "Not allowed");
    assert_eq!(skills::extract_error(r#"{"error":"Nope"}"#, "f"), "Nope");
    assert_eq!(skills::extract_error(r#"{"title":"Bad"}"#, "f"), "Bad");
    assert_eq!(skills::extract_error("", "Failed to fetch skills"), "Failed to fetch skills");
    assert_eq!(skills::extract_error("{}", "Failed to remove skill"), "Failed to remove skill");
}

#[test]
fn reads_the_three_sources_and_takes_a_number_it_does_not_know_for_local() {
    assert_eq!([0, 1, 2, 9].map(Source::from_number), [Source::Bundled, Source::GitHub, Source::Local, Source::Local]);
    assert_eq!([Source::Bundled, Source::GitHub, Source::Local].map(Source::number), [0, 1, 2]);
}

#[test]
fn reads_a_github_address_the_way_the_custom_tab_does() {
    let root = parse_github_url("https://github.com/acme/tidy.git").unwrap();
    assert_eq!(
        (root.repo_url.as_str(), root.ref_, root.sub_path, root.name.as_str()),
        ("https://github.com/acme/tidy.git", None, None, "tidy")
    );
    let deep = parse_github_url("http://github.com/acme/repo/tree/main/skills/my-skill/").unwrap();
    assert_eq!((deep.ref_.as_deref(), deep.sub_path.as_deref(), deep.name.as_str()), (Some("main"), Some("skills/my-skill"), "my-skill"));
    assert_eq!(parse_github_url("https://github.com/acme/repo/blob/dev").unwrap().ref_.as_deref(), Some("dev"));
    for bad in [
        "https://github.com/acme",
        "https://github.com/acme/repo/issues/3",
        "https://github.com/acme/repo/tree",
        "/work/skill",
        "https://example.com/a/b",
    ] {
        assert_eq!(parse_github_url(bad), None, "{bad}");
    }
}

// ─── SkillsToolsPage.test.ts and the page, over a fake Fleet ────────────────────

#[derive(Default)]
pub(crate) struct Fake {
    pub(crate) skills: Vec<Value>,
    /// Every call heard on /api/skills: method and path with its query.
    pub(crate) heard: Vec<(String, String)>,
    /// What `GET /api/skills` answers instead of the list.
    pub(crate) broken: Option<(u16, String)>,
    /// What the check, the update and the remove answer.
    pub(crate) check: Option<(u16, String)>,
    pub(crate) update: Option<(u16, String)>,
    pub(crate) remove: Option<(u16, String)>,
    pub(crate) mods_on: Option<bool>,
    pub(crate) repositories: Vec<Value>,
    /// Whether `GET /api/repositories` was asked.
    pub(crate) asked_repositories: bool,
    /// What `GET /api/skills/catalog` answers instead of the entries, the entries, and what the install answers.
    pub(crate) catalog: Option<(u16, String)>,
    pub(crate) entries: Vec<Value>,
    pub(crate) install: Option<(u16, String)>,
    /// The bodies of the installs heard.
    pub(crate) installs: Vec<Value>,
    /// Tools (`/api/tools`): the list, what it answers instead, the catalog entries and what they answer instead, what the
    /// install and the remove answer, and the bodies of the installs heard.
    pub(crate) tools: Vec<Value>,
    pub(crate) tools_broken: Option<(u16, String)>,
    pub(crate) tool_entries: Vec<Value>,
    pub(crate) tool_catalog: Option<(u16, String)>,
    pub(crate) tool_install: Option<(u16, String)>,
    pub(crate) tool_remove: Option<(u16, String)>,
    pub(crate) tool_installs: Vec<Value>,
}

type Shared = Arc<Mutex<Fake>>;

fn serve(fake: Shared) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
    let base = format!("http://{}", listener.local_addr().expect("an address"));
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let fake = fake.clone();
            std::thread::spawn(move || answer(stream, fake));
        }
    });
    base
}

fn answer(mut stream: std::net::TcpStream, fake: Shared) {
    let req = crate::test_http::read(&mut stream);
    let (method, path) = (req.method.clone(), req.path.clone());
    let mut f = fake.lock().unwrap();
    let route = path.split('?').next().unwrap_or("").to_string();
    if route.starts_with("/api/skills") || route.starts_with("/api/tools") {
        f.heard.push((method.clone(), path.clone()));
    }
    let (status, reply) = match (method.as_str(), route.as_str()) {
        ("GET", "/api/skills") => f.broken.clone().unwrap_or_else(|| (200, json!({ "skills": f.skills }).to_string())),
        ("GET", r) if r.ends_with("/update-check") => {
            f.check.clone().unwrap_or_else(|| (200, json!({ "updateAvailable": true }).to_string()))
        }
        ("POST", r) if r.ends_with("/update") => {
            f.update.clone().unwrap_or_else(|| (200, json!({ "name": "x", "syncResults": [] }).to_string()))
        }
        ("DELETE", r) if r.starts_with("/api/skills/") => {
            let answer = f.remove.clone().unwrap_or((204, String::new()));
            if answer.0 < 300 {
                let name = r.trim_start_matches("/api/skills/").to_string();
                let scope_project = path.contains("scope=project");
                f.skills.retain(|s| !(s["name"] == name && (s["scope"] == "project") == scope_project));
            }
            answer
        }
        ("GET", "/api/skills/catalog") => {
            f.catalog.clone().unwrap_or_else(|| (200, json!({ "entries": f.entries, "isStale": false }).to_string()))
        }
        ("POST", "/api/skills/install") => {
            let sent = req.json();
            f.installs.push(sent.clone());
            let answer = f.install.clone().unwrap_or_else(|| (200, json!({ "name": sent["name"], "syncResults": [] }).to_string()));
            if answer.0 < 300 {
                let (name, scope) = (sent["name"].as_str().unwrap_or("x").to_string(), sent["scope"].clone());
                let mut e = entry(&name, json!({ "scope": scope, "projectPath": sent["projectPath"] }));
                e["source"] = sent["source"].clone();
                f.skills.push(e);
            }
            answer
        }
        ("GET", "/api/tools") => f.tools_broken.clone().unwrap_or_else(|| (200, json!({ "tools": f.tools }).to_string())),
        ("GET", "/api/tools/catalog") => {
            f.tool_catalog.clone().unwrap_or_else(|| (200, json!({ "entries": f.tool_entries, "isStale": false }).to_string()))
        }
        ("POST", "/api/tools/install") => {
            let sent = req.json();
            f.tool_installs.push(sent.clone());
            let answer = f.tool_install.clone().unwrap_or_else(|| (200, json!({ "name": sent["name"] }).to_string()));
            if answer.0 < 300 {
                let name = sent["name"].as_str().unwrap_or("x").to_string();
                let mut t = crate::tests_settings_tools::tool(&name, json!({ "scope": sent["scope"], "projectPath": sent["projectPath"] }));
                t["toolType"] = sent["toolType"].clone();
                f.tools.push(t);
            }
            answer
        }
        ("DELETE", r) if r.starts_with("/api/tools/") => {
            let answer = f.tool_remove.clone().unwrap_or((204, String::new()));
            if answer.0 < 300 {
                let name = r.trim_start_matches("/api/tools/").to_string();
                let scope_project = path.contains("scope=project");
                f.tools.retain(|t| !(t["name"] == name && (t["scope"] == "project") == scope_project));
            }
            answer
        }
        ("GET", "/api/features/mods") => match f.mods_on {
            Some(on) => (200, json!({ "on": on }).to_string()),
            None => (404, String::new()),
        },
        ("GET", "/api/repositories") => {
            f.asked_repositories = true;
            (200, json!({ "repositories": f.repositories, "scannedAt": 1 }).to_string())
        }
        ("GET", "/api/machines") => (200, json!({ "machines": [] }).to_string()),
        _ => (200, "{}".to_string()),
    };
    drop(f);
    crate::test_http::respond_json(&mut stream, status, &reply);
}

pub(crate) struct Rig {
    pub(crate) rt: Runtime<State>,
    pub(crate) queue: Receiver<Job>,
    pub(crate) fake: Shared,
}

fn link_to(base: String) -> (Link, Receiver<Job>) {
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    (Link::start(base, Post::new(move |job| drop(jobs.lock().unwrap().send(job)))), queue)
}

/// A connected app on Settings → Skills & tools (at `anchor`), the page not yet read.
pub(crate) fn rig_at(anchor: Option<&str>, setup: impl FnOnce(&mut Fake)) -> Rig {
    let fake: Shared = Default::default();
    setup(&mut fake.lock().unwrap());
    let (link, queue) = link_to(serve(fake.clone()));
    let mut rt = app();
    let anchor = anchor.map(str::to_string);
    rt.update(move |s, _| {
        s.link = Some(link);
        s.rail = Rail::Settings;
        s.open_settings("skills", anchor.as_deref());
    });
    Rig { rt, queue, fake }
}

pub(crate) fn rig(setup: impl FnOnce(&mut Fake)) -> Rig {
    rig_at(None, setup)
}

impl Rig {
    pub(crate) fn wait(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        test_wait::drive(&mut self.rt, &self.queue, what, done, |s| format!("{:?}", s.settings.skills));
    }
    /// The page read: the list, the repositories and the Mods answer are all in.
    pub(crate) fn read(&mut self) {
        self.wait("the page to be read", |s| {
            let p = &s.settings.skills;
            p.loaded_for.is_some() && !p.loading && !p.repos_loading && p.mods_server.is_some() == (s.settings.skills.mods_server.is_some())
        });
        let fake = self.fake.clone();
        test_wait::until_true("the Mods and repositories questions to be answered", || fake.lock().unwrap().asked_repositories);
        self.settle();
    }
    pub(crate) fn settle(&mut self) {
        for _ in 0..3 {
            while let Ok(job) = self.queue.try_recv() {
                self.rt.update(job);
            }
            self.rt.frame(FRAME);
        }
    }
    pub(crate) fn exists(&self, key: &str) -> bool {
        self.rt.elements().iter().any(|e| e.key.as_deref() == Some(key) && !e.exiting)
    }
    pub(crate) fn text(&self, key: &str) -> String {
        let all = self.rt.elements();
        let it = all.iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).unwrap_or_else(|| panic!("no element keyed {key:?}"));
        all.iter().filter(|e| e.path.starts_with(&it.path)).filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("")
    }
    pub(crate) fn page(&self) -> &skills::SkillsPage {
        &self.rt.state.settings.skills
    }
    /// The calls heard on /api/skills.
    pub(crate) fn heard(&self) -> Vec<(String, String)> {
        self.fake.lock().unwrap().heard.clone()
    }
    pub(crate) fn heard_count(&self, method: &str, prefix: &str) -> usize {
        self.heard().iter().filter(|(m, p)| m == method && p.starts_with(prefix)).count()
    }
    pub(crate) fn press(&mut self, key: &str) {
        click(&mut self.rt, key);
    }
    pub(crate) fn tabs(&self) -> Vec<String> {
        self.rt
            .elements()
            .iter()
            .filter(|e| e.role == "tab" && e.key.as_deref().is_some_and(|k| k.starts_with("skills-tools-tab-")))
            .map(|e| self.text(e.key.as_deref().unwrap()))
            .collect()
    }
}

#[test]
fn has_a_mods_tab_only_while_the_mods_switch_is_on() {
    let mut off = rig(|f| f.mods_on = Some(false));
    off.read();
    assert_eq!(off.tabs(), ["Skills", "Tools"]);
    let mut on = rig(|f| f.mods_on = Some(true));
    on.read();
    assert_eq!(on.tabs(), ["Skills", "Tools", "Mods"]);
}

#[test]
fn opens_the_tools_tab_for_the_old_tools_id_and_the_mods_tab_for_the_old_mods_id() {
    let mut r = rig_at(Some("tools"), |f| f.mods_on = Some(true));
    r.read();
    assert_eq!(r.page().tab, Tab::Tools);
    r.settle();
    assert!(r.exists("tools-section"));
    let mut r = rig_at(Some("mods"), |f| f.mods_on = Some(true));
    r.read();
    assert_eq!(r.page().tab, Tab::Mods, "landing on mods before the switch was read opens it as soon as it is known to be on");
    assert!(r.exists("mods-not-yet"));
}

#[test]
fn falls_back_to_skills_when_the_old_mods_id_is_used_with_mods_off() {
    let mut r = rig_at(Some("mods"), |f| f.mods_on = Some(false));
    r.read();
    assert_eq!(r.page().tab, Tab::Skills);
    assert!(r.exists("skills-section") && !r.exists("mods-not-yet"));
}

#[test]
fn lets_the_users_choice_in_labs_win_over_the_server_and_drops_back_to_skills_when_it_goes_off() {
    let mut r = rig(|f| f.mods_on = Some(false));
    r.read();
    r.rt.update(|s, _| {
        s.pickers.prefs.set("Mods", "true");
    });
    r.settle();
    assert_eq!(r.tabs(), ["Skills", "Tools", "Mods"]);
    r.press("skills-tools-tab-mods");
    assert_eq!(r.page().tab, Tab::Mods);
    r.rt.update(|s, _| {
        s.pickers.prefs.set("Mods", "false");
    });
    r.settle();
    assert_eq!(r.page().tab, Tab::Skills, "the switch going off while Mods is open falls back to Skills");
}

#[test]
fn starts_on_skills_and_installed_each_time_the_page_opens() {
    let mut r = rig(|_| {});
    r.read();
    assert_eq!((r.page().tab, r.page().inner), (Tab::Skills, Inner::Installed));
    r.press("skills-tab-catalog");
    assert_eq!(r.page().inner, Inner::Catalog);
    r.rt.update(|s, _| s.open_settings("memory", None));
    r.settle();
    r.rt.update(|s, _| s.open_settings("skills", None));
    r.settle();
    assert_eq!(r.page().inner, Inner::Installed);
}

#[test]
fn says_what_the_page_is_for_in_the_words_fleet_uses() {
    let mut r = rig(|_| {});
    r.read();
    assert_eq!(r.text("settings-header"), "Skills & toolsWhat agents can use: skills, tools and, with Mods on, mods.");
    assert_eq!(
        r.text("skills-section").split("Installed").next().unwrap(),
        "Manage installed skills, browse the catalog, or install from a custom source."
    );
}

#[test]
fn can_be_called_manually_to_refresh() {
    let mut r = rig(|f| f.skills = vec![entry("skill-one", json!({}))]);
    r.read();
    assert_eq!(r.page().skills.len(), 1);
    assert_eq!(r.heard_count("GET", "/api/skills"), 1);
    r.fake.lock().unwrap().skills.push(entry("skill-two", json!({})));
    // The Installed tab reads each time it is opened: leave it and come back.
    r.press("skills-tab-custom");
    r.press("skills-tab-installed");
    r.wait("the second read", |s| s.settings.skills.skills.len() == 2);
    assert_eq!(r.heard_count("GET", "/api/skills"), 2);
}

#[test]
fn shows_a_failed_read_alone_when_there_is_nothing_to_list() {
    let mut r = rig(|f| f.broken = Some((500, "Network error".into())));
    r.wait("the failure", |s| s.settings.skills.error.is_some());
    r.settle();
    assert_eq!(r.text("skills-error"), "Network error");
    assert!(!r.exists("skills-empty") && !r.exists("skills-list"));
}

#[test]
fn says_so_when_nothing_is_installed() {
    let mut r = rig(|_| {});
    r.read();
    assert_eq!(r.text("skills-empty"), "No skills installedInstall a skill from the Catalog or Custom tab.");
}

// ─── The cards ──────────────────────────────────────────────────────────────────

fn mixed() -> Vec<Value> {
    vec![
        entry("tidy-imports", json!({})),
        entry("release-notes", json!({ "source": 2, "repoUrl": null, "localPath": "/work/skills/release-notes" })),
        entry(
            "release-notes",
            json!({ "source": 2, "repoUrl": null, "localPath": "/work/skills/release-notes", "scope": "project", "projectPath": "/work/lantern", "installedPaths": [] }),
        ),
        entry("commit-style", json!({ "source": 0, "repoUrl": null })),
    ]
}

#[test]
fn draws_a_card_per_install_and_keeps_a_global_and_a_project_install_of_the_same_skill_apart() {
    let mut r = rig(|f| f.skills = mixed());
    r.read();
    let keys: Vec<_> = r.page().skills.iter().map(InstalledSkill::key).collect();
    assert_eq!(keys, ["tidy-imports|global", "release-notes|global", "release-notes|project:/work/lantern", "commit-style|global"]);
    for k in &keys {
        assert!(r.exists(&format!("skill-{k}")), "{k}");
    }
    assert_eq!(r.text("skill-target-release-notes|project:/work/lantern"), "lantern");
    assert_eq!(r.text("skill-target-release-notes|global"), "Global");
}

#[test]
fn shows_a_github_install_s_address_twice_as_fleet_does() {
    let mut r = rig(|f| f.skills = mixed());
    r.read();
    let card = r.text("skill-tidy-imports|global");
    assert_eq!(card.matches("https://github.com/example/tidy-imports").count(), 2, "{card}");
    assert!(card.contains("\u{2192} /home/user/.config/opencode/skills/tidy-imports"));
}

#[test]
fn warns_about_a_skill_no_harness_reads() {
    let mut r = rig(|f| f.skills = mixed());
    r.read();
    assert!(
        r.text("skill-release-notes|project:/work/lantern").contains("Not in any folder a harness reads. Remove it and install it again.")
    );
    assert!(!r.exists("skill-no-folder-release-notes|global"));
}

#[test]
fn offers_check_for_updates_only_for_github_and_remove_never_for_bundled() {
    let mut r = rig(|f| f.skills = mixed());
    r.read();
    assert!(r.exists("skill-check-tidy-imports|global"));
    assert!(!r.exists("skill-check-release-notes|global") && !r.exists("skill-check-commit-style|global"));
    assert!(r.exists("skill-remove-release-notes|global") && !r.exists("skill-remove-commit-style|global"));
    assert!(r.text("skill-commit-style|global").contains("Bundled"));
    assert!(!r.exists("skill-update-tidy-imports|global"));
}

// ─── use-skills.spec.ts: checkUpdate ────────────────────────────────────────────

#[test]
fn checks_for_updates_and_updates_skill_state() {
    let mut r = rig(|f| f.skills = vec![entry("test-skill", json!({}))]);
    r.read();
    r.press("skill-check-test-skill|global");
    r.wait("the check to answer", |s| s.settings.skills.skills[0].update_available);
    assert!(r.heard().contains(&("GET".into(), "/api/skills/test-skill/update-check?scope=global".into())));
    r.settle();
    assert_eq!(r.page().skills[0].update_check_error, None);
    assert_eq!(r.page().error, None);
    assert!(r.exists("skill-update-badge-test-skill|global") && r.exists("skill-update-test-skill|global"));
    assert_eq!(r.text("skill-update-badge-test-skill|global"), "Update Available");
}

#[test]
fn handles_update_check_errors_and_updates_skill_state() {
    let mut r = rig(|f| {
        f.skills = vec![entry("test-skill", json!({}))];
        f.check = Some((502, "Failed to check updates".into()));
    });
    r.read();
    r.press("skill-check-test-skill|global");
    r.wait("the failure", |s| s.settings.skills.skills[0].update_check_error.is_some());
    r.settle();
    assert_eq!(r.page().skills[0].update_check_error.as_deref(), Some("Failed to check updates"));
    assert_eq!(r.page().error.as_deref(), Some("Failed to check updates"));
    assert_eq!(r.text("skill-check-error-test-skill|global"), "Failed to check updates");
    assert_eq!(r.text("skills-error"), "Failed to check updates", "under the list too");
    assert!(!r.page().skills[0].update_available);
}

#[test]
fn handles_non_existent_skill_gracefully() {
    let mut r = rig(|f| f.skills = vec![entry("other-skill", json!({}))]);
    r.read();
    r.rt.update(|s, _| s.settings.skills.patch("non-existent", &GLOBAL, |c| c.update_available = true));
    assert_eq!(r.page().skills.len(), 1);
    assert!(!r.page().skills[0].update_available);
}

#[test]
fn keeps_a_global_and_a_project_install_of_the_same_skill_apart_when_checking() {
    let mut r = rig(|f| {
        f.skills = vec![
            entry("shared", json!({})),
            entry(
                "shared",
                json!({ "scope": "project", "projectPath": "/src/app", "installedPaths": ["/src/app/.opencode/skills/shared"] }),
            ),
        ]
    });
    r.read();
    r.press("skill-check-shared|project:/src/app");
    r.wait("the check", |s| s.settings.skills.skills.iter().any(|c| c.update_available));
    assert!(r.heard().contains(&("GET".into(), "/api/skills/shared/update-check?scope=project&projectPath=%2Fsrc%2Fapp".into())));
    let flags: Vec<_> = r.page().skills.iter().map(|s| s.update_available).collect();
    assert_eq!(flags, [false, true]);
}

// ─── use-skills.spec.ts: updateSkill ────────────────────────────────────────────

fn with_update_available(r: &mut Rig) {
    r.press("skill-check-test-skill|global");
    r.wait("the check", |s| s.settings.skills.skills[0].update_available);
    r.settle();
}

#[test]
fn updates_a_skill_and_refreshes_the_list() {
    let mut r = rig(|f| f.skills = vec![entry("test-skill", json!({}))]);
    r.read();
    with_update_available(&mut r);
    r.fake.lock().unwrap().skills = vec![entry("test-skill", json!({ "ref": "v2.0.0" }))];
    r.press("skill-update-test-skill|global");
    r.wait("the update and the read after it", |s| {
        s.settings.skills.updating.is_none() && !s.settings.skills.loading && !s.settings.skills.skills[0].update_available
    });
    assert_eq!(r.heard_count("POST", "/api/skills/test-skill/update?scope=global"), 1);
    assert_eq!(
        r.heard_count("GET", "/api/skills") - r.heard_count("GET", "/api/skills/"),
        2,
        "the first read and the one after the update"
    );
    assert_eq!(r.page().error, None);
}

#[test]
fn handles_update_errors() {
    let mut r = rig(|f| {
        f.skills = vec![entry("test-skill", json!({}))];
        f.update = Some((500, "Update failed".into()));
    });
    r.read();
    with_update_available(&mut r);
    r.press("skill-update-test-skill|global");
    r.wait("the failure", |s| s.settings.skills.error.is_some());
    r.settle();
    assert_eq!(r.page().error.as_deref(), Some("Update failed"));
    assert_eq!(r.text("skills-error"), "Update failed");
    assert_eq!(r.page().updating, None);
    assert!(r.page().skills[0].update_available, "nothing changed on the card");
}

#[test]
fn handles_a_missing_data_response_to_an_update() {
    let mut r = rig(|f| {
        f.skills = vec![entry("test-skill", json!({}))];
        f.update = Some((200, String::new()));
    });
    r.read();
    with_update_available(&mut r);
    r.press("skill-update-test-skill|global");
    r.wait("the failure", |s| s.settings.skills.error.is_some());
    assert_eq!(r.page().error.as_deref(), Some("No data returned"));
}

// ─── use-skills.spec.ts: removeSkill, and the confirm ───────────────────────────

#[test]
fn removes_a_skill_and_refreshes_the_list() {
    let mut r = rig(|f| f.skills = vec![entry("skill-one", json!({})), entry("skill-two", json!({}))]);
    r.read();
    assert_eq!(r.page().skills.len(), 2);
    r.press("skill-remove-skill-one|global");
    assert_eq!(r.page().confirm_remove.as_deref(), Some("skill-one|global"));
    assert_eq!(r.heard_count("DELETE", "/api/skills/"), 0, "the first press only asks");
    assert_eq!(r.text("skill-remove-skill-one|global"), "Confirm remove");
    assert!(r.exists("skill-cancel-skill-one|global"));
    r.press("skill-remove-skill-one|global");
    r.wait("the remove and the read after it", |s| {
        s.settings.skills.removing.is_none() && !s.settings.skills.loading && s.settings.skills.skills.len() == 1
    });
    assert_eq!(r.heard_count("DELETE", "/api/skills/skill-one?scope=global"), 1);
    assert_eq!(r.page().skills[0].name, "skill-two");
    assert_eq!(r.page().error, None);
    assert_eq!(r.page().confirm_remove, None);
}

#[test]
fn handles_removal_errors() {
    let mut r = rig(|f| {
        f.skills = vec![entry("test-skill", json!({}))];
        f.remove = Some((500, "Removal failed".into()));
    });
    r.read();
    r.press("skill-remove-test-skill|global");
    r.press("skill-remove-test-skill|global");
    r.wait("the failure", |s| s.settings.skills.error.is_some());
    r.settle();
    assert_eq!(r.page().error.as_deref(), Some("Removal failed"));
    assert_eq!(r.page().confirm_remove.as_deref(), Some("test-skill|global"), "the confirm state stays");
    assert_eq!(r.text("skills-error"), "Removal failed");
    assert_eq!(r.page().skills.len(), 1);
}

#[test]
fn cancels_a_remove_and_moves_the_confirm_to_the_card_pressed_last() {
    let mut r = rig(|f| f.skills = vec![entry("skill-one", json!({})), entry("skill-two", json!({}))]);
    r.read();
    r.press("skill-remove-skill-one|global");
    r.press("skill-remove-skill-two|global");
    assert_eq!(r.page().confirm_remove.as_deref(), Some("skill-two|global"), "one confirm at a time");
    assert!(!r.exists("skill-cancel-skill-one|global") && r.exists("skill-cancel-skill-two|global"));
    assert_eq!(r.text("skill-remove-skill-one|global"), "Remove");
    // The Cancel button slides in: let it arrive before pressing where it is.
    for _ in 0..30 {
        r.rt.frame(FRAME);
    }
    r.press("skill-cancel-skill-two|global");
    assert_eq!(r.page().confirm_remove, None);
    assert_eq!(r.heard_count("DELETE", "/api/skills/"), 0);
}

// ─── The picker and where the page asks ─────────────────────────────────────────

#[test]
fn offers_global_and_every_repository_and_says_where_the_install_goes() {
    let mut r = rig(|f| f.repositories = vec![json!({ "name": "lantern", "path": "/work/lantern", "parentRoot": "/work" })]);
    r.read();
    r.press("skills-tab-custom");
    assert_eq!(r.text("skills-target-line"), "Every session \u{b7} ~/.config/opencode/skills/");
    r.press("skills-target");
    assert!(r.exists("skills-target-list-opt0") && r.exists("skills-target-list-opt1"));
    assert_eq!(r.text("skills-target-list-opt1"), "lantern");
    r.press("skills-target-list-opt1");
    assert_eq!(r.page().target, project("/work/lantern"));
    assert_eq!(r.text("skills-target-line"), "Sessions in this repository \u{b7} /work/lantern/.opencode/skills/");
    // Kept while you switch between Catalog and Custom.
    r.press("skills-tab-catalog");
    assert_eq!(r.page().target, project("/work/lantern"));
    assert_eq!(r.text("skills-target-line"), "Sessions in this repository \u{b7} /work/lantern/.opencode/skills/");
}

#[test]
fn asks_the_machine_settings_shows_not_home() {
    // No server: the page draws the invented skills, and the buttons change them.
    let mut rt = app();
    rt.update(|s, _| {
        s.rail = Rail::Settings;
        s.open_settings("skills", None);
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    let names: Vec<String> = rt.state.settings.skills.skills.iter().map(InstalledSkill::key).collect();
    assert_eq!(names.first().map(String::as_str), Some("release-notes|global"));
    click(&mut rt, "skill-check-tidy-imports|global");
    assert!(rt.state.settings.skills.skills.iter().any(|s| s.update_available));
    click(&mut rt, "skill-remove-harbor-deploy|project:/work/lantern");
    click(&mut rt, "skill-remove-harbor-deploy|project:/work/lantern");
    assert!(!rt.state.settings.skills.skills.iter().any(|s| s.name == "harbor-deploy"));
}
