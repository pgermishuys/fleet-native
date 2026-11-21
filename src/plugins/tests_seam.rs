//! The owner's rule: GitHub is an add-on. Core never imports the GitHub plugin; the plugin fills places core offers. The guard
//! reads every `.rs` file under `src/` outside `src/plugins/` and fails one that names the plugin, in the style of
//! `test_http::tests::fakes_read_requests_through_test_http`.
use crate::fleet::places::{Entry, Places};
use crate::fleet::prefs::Prefs;
use crate::fleet::settings::plugins::Plugin;
use crate::tests::{FRAME, app, click};
use crate::{Rail, State};
use serde_json::json;
use std::path::{Path, PathBuf};

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("a source folder").flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// The files outside the plugins' folder that name a plugin's module. `lib.rs` declares the folder and nothing else; the
/// constructor's `plugins::register(..)` names no plugin. This test's own words below are in `src/plugins/`, so they never trip it.
fn names_a_plugin(text: &str) -> bool {
    text.lines().filter(|l| !l.trim_start().starts_with("//")).any(|l| l.contains("plugins::github") || l.contains("mod github"))
}

fn leaks(files: &[(String, String)]) -> Vec<String> {
    files.iter().filter(|(_, text)| names_a_plugin(text)).map(|(path, _)| path.clone()).collect()
}

#[test]
fn core_never_imports_the_github_plugin() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rust_files(&src, &mut files);
    let core: Vec<(String, String)> = files
        .into_iter()
        .filter(|p| !p.starts_with(src.join("plugins")))
        .map(|p| (p.strip_prefix(&src).expect("under src").display().to_string(), std::fs::read_to_string(&p).expect("a file")))
        .collect();
    assert!(core.len() > 100, "the guard read the source tree ({} files)", core.len());
    // The composition root, `src/plugins/mod.rs`, is the one place that registers a plugin, and it is inside the folder.
    assert_eq!(leaks(&core), Vec::<String>::new(), "core files name the GitHub plugin; it must fill a place in `fleet::places` instead");
}

#[test]
fn the_guard_fails_a_file_that_imports_the_plugin() {
    let clean = "use crate::fleet::places::Places;\nfn f() {}\n";
    let planted = format!("{clean}use crate::plugins::github::auth::Github;\n");
    let files = [("fleet/a.rs".to_string(), clean.to_string()), ("fleet/b.rs".to_string(), planted)];
    assert_eq!(leaks(&files), ["fleet/b.rs"]);
    let commented = [("fleet/c.rs".to_string(), "// the plugin (`plugins::github`) fills it\n".to_string())];
    assert!(leaks(&commented).is_empty(), "a comment is not an import");
}

/// A stand-in plugin that fills every place, to see core use them without naming anything.
fn stand_in() -> Entry {
    let manifest = Plugin {
        id: "stand-in",
        name: "Stand-in",
        description: "Fills every place.",
        own_switch: false,
        switchless: false,
        page_title: "Stand-in settings",
        nav_label: "Stand-in",
        adds: &[],
    };
    let mut e = Entry::new(manifest, 0u32);
    e.rail = Some(crate::fleet::places::RailItem {
        label: "Stand-in",
        icon: &crate::icons::ZAP,
        path: "/stand-in",
        order: 110,
        placement: crate::fleet::rails::Placement::Plugin,
    });
    e.panel = Some(|_, _| rune::el().key("stand-in-panel").w(200.0).h(40.0));
    e.page = Some(|_, _| rune::el().key("stand-in-page").grow(1.0).h(40.0));
    e.settings = Some(|_, _| rune::el().key("stand-in-settings").h(20.0));
    e
}

fn off(id: &str) -> Prefs {
    let mut p = Prefs::default();
    p.adopt(&json!({ "plugins.disabled": serde_json::to_string(&[id]).unwrap() }));
    p
}

#[test]
fn a_plugin_switched_off_adds_nothing() {
    let mut places = Places::default();
    places.register(stand_in());
    let on = Prefs::default();
    assert_eq!(places.rail_items(&on).len(), 1);
    assert!(places.rail_views("stand-in", &on).is_some());
    assert!(places.settings_body("stand-in", &on).is_some());
    assert_eq!(places.keeps(&on).len(), 0);
    let gone = off("stand-in");
    assert!(places.rail_items(&gone).is_empty(), "no rail button");
    assert!(places.rail_views("stand-in", &gone).is_none(), "no panel or page");
    assert!(places.settings_body("stand-in", &gone).is_none(), "no settings page");
    // The other plugin is unaffected, and the entry is still there for the Plugins list to show it off.
    let mut both = crate::plugins::register(Places::default());
    both.register(stand_in());
    assert_eq!(both.rail_items(&gone).len(), 1, "GitHub's rail stays while only the stand-in is off");
    assert_eq!(both.rail_items(&off("github")).len(), 1, "GitHub off leaves the stand-in only");
    assert_eq!(both.rail_items(&Prefs::default()).len(), 2);
    assert!(both.manifests().iter().any(|p| p.id == "stand-in"), "the Plugins list still lists it");
}

#[test]
fn the_github_rail_opens_the_plugins_panel_and_page() {
    let mut r = app();
    click(&mut r, "rail-github");
    assert_eq!(r.state.rail, Rail::Plugin("github"));
    // GitHub's own panel (piece D) and page are what its rail draws; the page says so until pieces G and I build it.
    assert!(r.state.places.rail_views("github", &r.state.pickers.prefs).is_some_and(|(panel, _)| panel.is_some()));
    assert!(r.find("github-panel").is_some() && r.find("github-page").is_some());
    // A plugin with a panel and a page gets both drawn, and with them the page's own address.
    r.update(|s: &mut State, _| {
        s.places.register(stand_in());
        s.rail = Rail::Plugin("stand-in");
    });
    r.frame(FRAME);
    r.frame(FRAME);
    assert!(r.find("stand-in-panel").is_some(), "the plugin's panel is drawn");
    assert!(r.find("stand-in-page").is_some(), "the plugin's page is drawn");
    let list = r.state.rail_list();
    assert!(list.iter().any(|d| d.rail == Rail::Plugin("stand-in") && d.to == "/stand-in"));
}
