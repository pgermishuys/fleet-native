//! Component 16, piece AH: Settings → Skills & tools → Skills → Catalog and Custom. Fleet has no test for these two tabs
//! (`SkillsToolsPage.test.ts` is the page), so these are written from the rows: what the catalog shows and reads, the body an
//! install from a card and from a typed address sends, the failures, and that Installed means installed *here*.
use crate::fleet::settings::install_target::InstallTarget;
use crate::fleet::settings::skills::Inner;
use crate::fleet::settings::skills_catalog::{custom_install_body, parse_catalog};
use crate::tests_settings_skills::{Rig, entry, rig};
use rune::TextEdit;
use serde_json::{Value, json};

fn card(name: &str, over: Value) -> Value {
    let mut v = json!({
        "name": name, "displayName": null, "description": "Does a thing.", "source": 1,
        "repoUrl": format!("https://github.com/example/{name}.git"), "ref": "main", "subPath": null, "localPath": null,
        "targetHarnesses": ["opencode"], "author": null, "version": null, "tags": [],
    });
    for (k, x) in over.as_object().into_iter().flatten() {
        v[k] = x.clone();
    }
    v
}

fn project(path: &str) -> InstallTarget {
    InstallTarget::Project(path.into())
}

/// The page read, then the Catalog tab opened and its catalog and the list read again.
fn catalog_open(setup: impl FnOnce(&mut crate::tests_settings_skills::Fake)) -> Rig {
    let mut r = rig(setup);
    r.read();
    r.press("skills-tab-catalog");
    r.wait("the catalog", |s| !s.settings.skills.catalog_loading && !s.settings.skills.loading);
    r.settle();
    r
}

fn custom_open(setup: impl FnOnce(&mut crate::tests_settings_skills::Fake)) -> Rig {
    let mut r = rig(setup);
    r.read();
    r.press("skills-tab-custom");
    r.settle();
    r
}

fn type_address(r: &mut Rig, words: &str) {
    let words = words.to_string();
    r.rt.update(move |s, _| s.settings.skills.custom = TextEdit::new(words));
    r.settle();
}

// ─── the data ───────────────────────────────────────────────────────────────────

#[test]
fn reads_a_catalog_answer_and_drops_an_entry_with_no_name() {
    let body = json!({ "entries": [card("a", json!({ "displayName": "A", "tags": ["x", "y"], "version": "1.0.0" })), { "description": "nameless" }] });
    let list = parse_catalog(&body).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].title(), "A");
    assert_eq!(list[0].tags, ["x", "y"]);
    assert_eq!(parse_catalog(&Value::Null), Err("No data returned".to_string()));
    assert_eq!(parse_catalog(&json!({})).unwrap(), vec![]);
}

#[test]
fn builds_a_catalog_install_with_the_local_path_for_the_missing_sub_path() {
    let local = parse_catalog(&json!({ "entries": [card("l", json!({ "source": 2, "repoUrl": null, "localPath": "/work/l" }))] })).unwrap();
    let body = local[0].install_body(&project("/src/app"));
    assert_eq!(body["subPath"], "/work/l");
    assert_eq!(body["source"], 2);
    assert_eq!((body["scope"].clone(), body["projectPath"].clone()), (json!("project"), json!("/src/app")));
    let github = parse_catalog(&json!({ "entries": [card("g", json!({ "subPath": "skills/g" }))] })).unwrap();
    let body = github[0].install_body(&InstallTarget::Global);
    assert_eq!(
        (body["subPath"].clone(), body["ref"].clone(), body["projectPath"].clone()),
        (json!("skills/g"), json!("main"), Value::Null)
    );
}

#[test]
fn turns_a_typed_address_into_an_install() {
    let g = custom_install_body("  https://github.com/acme/repo/tree/main/skills/my-skill ", &InstallTarget::Global).unwrap();
    assert_eq!(g["name"], "my-skill");
    assert_eq!((g["source"].clone(), g["ref"].clone(), g["subPath"].clone()), (json!(1), json!("main"), json!("skills/my-skill")));
    assert_eq!(g["repoUrl"], "https://github.com/acme/repo.git");
    assert_eq!(g["targetHarnesses"], Value::Null);
    let l = custom_install_body("C:\\skills\\mine\\", &project("/src/app")).unwrap();
    assert_eq!((l["name"].clone(), l["source"].clone(), l["localPath"].clone()), (json!("mine"), json!(2), json!("C:\\skills\\mine\\")));
    assert_eq!(l["scope"], "project");
    assert_eq!(custom_install_body("/", &InstallTarget::Global).unwrap()["name"], "unknown-skill");
    let bad = custom_install_body("https://github.com/acme", &InstallTarget::Global).unwrap_err();
    assert!(bad.starts_with("Invalid GitHub URL format. Expected: https://github.com/owner/repo"), "{bad}");
}

// ─── Catalog ────────────────────────────────────────────────────────────────────

#[test]
fn draws_a_card_per_entry_with_its_author_tags_source_and_version() {
    let r = catalog_open(|f| {
        f.entries = vec![
            card("one", json!({ "displayName": "Number one", "author": "Ada", "tags": ["docs", "git"], "version": "1.2.0" })),
            card("two", json!({ "source": 2, "repoUrl": null, "localPath": "/work/two", "description": null })),
            card("three", json!({ "source": 0 })),
        ]
    });
    assert!(r.exists("skills-catalog-grid"));
    assert!(r.text("catalog-card-one").contains("Number one"));
    assert!(r.text("catalog-card-one").contains("by Ada"));
    assert!(r.text("catalog-card-one").contains("docs"));
    assert!(r.text("catalog-card-one").contains("GitHub"));
    assert!(r.text("catalog-card-one").contains("v1.2.0"));
    assert!(r.text("catalog-card-two").contains("Local"));
    assert!(!r.text("catalog-card-two").contains("Does a thing."));
    assert!(r.text("catalog-card-three").contains("Bundled"));
    assert!(!r.text("catalog-card-three").contains(" by "));
    assert_eq!(r.text("catalog-install-one"), "Install");
}

#[test]
fn says_installed_only_for_a_skill_installed_where_the_picker_points() {
    let mut r = catalog_open(|f| {
        f.entries = vec![card("one", json!({})), card("two", json!({}))];
        f.skills = vec![entry("one", json!({}))];
    });
    assert_eq!(r.text("catalog-installed-one"), "Installed");
    assert!(r.exists("catalog-install-two"));
    r.rt.update(|s, _| s.choose_skills_target("project:/src/app"));
    r.settle();
    assert!(r.exists("catalog-install-one"), "installed elsewhere does not count");
    assert!(!r.exists("catalog-installed-one"));
}

#[test]
fn installs_from_a_card_and_reads_the_list_again_so_the_button_turns_to_installed() {
    let mut r = catalog_open(|f| f.entries = vec![card("one", json!({ "subPath": "skills/one" }))]);
    let before = r.heard_count("GET", "/api/skills");
    r.press("catalog-install-one");
    r.wait("the install and the read after it", |s| s.settings.skills.installing_name.is_none() && !s.settings.skills.installing);
    r.settle();
    let sent = r.fake.lock().unwrap().installs.clone();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0]["name"], "one");
    assert_eq!(sent[0]["repoUrl"], "https://github.com/example/one.git");
    assert_eq!((sent[0]["scope"].clone(), sent[0]["projectPath"].clone()), (json!("global"), Value::Null));
    assert_eq!(
        r.heard_count("GET", "/api/skills") - r.heard_count("GET", "/api/skills/"),
        before - r.heard_count("GET", "/api/skills/") + 1
    );
    assert_eq!(r.text("catalog-installed-one"), "Installed");
    assert_eq!(r.page().install_error, None);
}

#[test]
fn installs_into_the_repository_the_picker_names() {
    let mut r = catalog_open(|f| f.entries = vec![card("one", json!({}))]);
    r.rt.update(|s, _| s.choose_skills_target("project:/src/app"));
    r.press("catalog-install-one");
    r.wait("the install", |s| !s.settings.skills.installing);
    let sent = r.fake.lock().unwrap().installs.clone();
    assert_eq!((sent[0]["scope"].clone(), sent[0]["projectPath"].clone()), (json!("project"), json!("/src/app")));
}

#[test]
fn shows_an_install_failure_under_the_grid_and_keeps_the_button() {
    let mut r = catalog_open(|f| {
        f.entries = vec![card("one", json!({}))];
        f.install = Some((409, json!({ "detail": "Skill already exists" }).to_string()));
    });
    r.press("catalog-install-one");
    r.wait("the failure", |s| s.settings.skills.install_error.is_some());
    r.settle();
    assert_eq!(r.text("skills-install-error"), "Skill already exists");
    assert!(r.exists("catalog-install-one"));
    assert!(!r.page().installing);
}

#[test]
fn takes_an_install_answer_with_nothing_in_it_for_a_failure() {
    let mut r = catalog_open(|f| {
        f.entries = vec![card("one", json!({}))];
        f.install = Some((200, String::new()));
    });
    r.press("catalog-install-one");
    r.wait("the failure", |s| s.settings.skills.install_error.is_some());
    assert_eq!(r.page().install_error.as_deref(), Some("No data returned"));
}

#[test]
fn shows_the_load_error_alone_when_there_is_no_catalog_and_the_empty_state_when_it_is_empty() {
    let broken = catalog_open(|f| f.catalog = Some((500, "Catalog down".into())));
    assert_eq!(broken.text("skills-catalog-error"), "Catalog down");
    assert!(!broken.exists("skills-catalog-empty"));
    let empty = catalog_open(|_| {});
    assert!(empty.text("skills-catalog-empty").contains("No skills in catalog"));
    assert!(empty.text("skills-catalog-empty").contains("The skill catalog is empty or unavailable."));
}

#[test]
fn reads_the_catalog_each_time_the_tab_opens_and_keeps_the_target_between_catalog_and_custom() {
    let mut r = catalog_open(|f| f.entries = vec![card("one", json!({}))]);
    assert_eq!(r.heard_count("GET", "/api/skills/catalog"), 1);
    r.rt.update(|s, _| s.choose_skills_target("project:/src/app"));
    r.press("skills-tab-custom");
    assert_eq!(r.page().target, project("/src/app"), "the choice is kept");
    assert!(r.page().catalog.is_empty(), "a tab that is left forgets what it read");
    r.press("skills-tab-catalog");
    r.wait("the second read", |s| !s.settings.skills.catalog_loading && !s.settings.skills.catalog.is_empty());
    assert_eq!(r.heard_count("GET", "/api/skills/catalog"), 2);
    assert_eq!(r.page().target, project("/src/app"));
}

// ─── Custom ─────────────────────────────────────────────────────────────────────

#[test]
fn says_what_the_form_is_for_and_gives_three_examples() {
    let r = custom_open(|_| {});
    let form = r.text("skills-custom-form");
    assert!(form.contains("Install from URL or path"));
    assert!(form.contains("Install a skill from a GitHub URL or local file path."));
    assert!(form.contains("Install skill"));
    let examples = r.text("skills-custom-examples");
    for line in [
        "https://github.com/username/skill-name",
        "https://github.com/username/repo/tree/main/skills/my-skill",
        "/Users/username/skills/custom-skill",
    ] {
        assert!(examples.contains(line), "{line}");
    }
}

#[test]
fn asks_for_an_address_before_sending_anything() {
    let mut r = custom_open(|_| {});
    type_address(&mut r, "   ");
    r.press("skills-custom-submit");
    r.settle();
    assert_eq!(r.text("skills-custom-error"), "Skill URL or path is required.");
    assert!(r.fake.lock().unwrap().installs.is_empty());
    assert!(!r.page().installing);
}

#[test]
fn installs_a_github_address_clears_the_field_and_reads_the_list_again() {
    let mut r = custom_open(|_| {});
    type_address(&mut r, "https://github.com/acme/repo/tree/main/skills/my-skill");
    r.press("skills-custom-submit");
    r.wait("the install", |s| !s.settings.skills.installing && s.settings.skills.custom.text.is_empty());
    r.settle();
    let sent = r.fake.lock().unwrap().installs.clone();
    assert_eq!(sent.len(), 1);
    assert_eq!(
        (sent[0]["name"].clone(), sent[0]["source"].clone(), sent[0]["subPath"].clone()),
        (json!("my-skill"), json!(1), json!("skills/my-skill"))
    );
    assert!(r.page().skills.iter().any(|s| s.name == "my-skill"), "the list was read again");
    assert!(!r.exists("skills-custom-error"));
}

#[test]
fn installs_a_local_path_named_by_its_last_segment() {
    let mut r = custom_open(|_| {});
    type_address(&mut r, "/work/skills/release-notes");
    r.press("skills-custom-submit");
    r.wait("the install", |s| !s.settings.skills.installing && s.settings.skills.custom.text.is_empty());
    let sent = r.fake.lock().unwrap().installs.clone();
    assert_eq!(
        (sent[0]["name"].clone(), sent[0]["source"].clone(), sent[0]["localPath"].clone()),
        (json!("release-notes"), json!(2), json!("/work/skills/release-notes"))
    );
}

#[test]
fn refuses_a_github_address_it_cannot_read_without_asking_the_server() {
    let mut r = custom_open(|_| {});
    type_address(&mut r, "https://github.com/acme");
    r.press("skills-custom-submit");
    r.settle();
    assert!(r.text("skills-custom-error").starts_with("Invalid GitHub URL format."));
    assert!(r.fake.lock().unwrap().installs.is_empty());
    assert_eq!(r.page().custom.text, "https://github.com/acme", "what was typed stays");
}

#[test]
fn shows_the_servers_reason_in_the_form_and_keeps_what_was_typed() {
    let mut r = custom_open(|f| f.install = Some((400, json!({ "error": "Not a skill folder" }).to_string())));
    type_address(&mut r, "/work/skills/nope");
    r.press("skills-custom-submit");
    r.wait("the failure", |s| s.settings.skills.custom_error.is_some());
    r.settle();
    assert_eq!(r.text("skills-custom-error"), "Not a skill folder");
    assert_eq!(r.page().custom.text, "/work/skills/nope");
    assert!(!r.page().installing);
}

#[test]
fn forgets_the_form_when_the_tab_is_left() {
    let mut r = custom_open(|_| {});
    type_address(&mut r, "/work/skills/nope");
    r.press("skills-custom-submit");
    r.press("skills-tab-installed");
    r.press("skills-tab-custom");
    assert_eq!(r.page().custom.text, "");
    assert_eq!(r.page().inner, Inner::Custom);
}
