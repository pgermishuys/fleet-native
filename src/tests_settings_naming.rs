//! Component 16, piece AB: Settings → Folders → Worktrees (`WorktreeNamingSection.vue`). Fleet has no test for the card, so
//! these are native: the fields start from the user's own layer, the chips append, Save sends nulls for empty fields,
//! "Use Fleet's defaults", a refusal shown in the server's words, "Saved" for 2 s, and a preview that follows the draft and
//! the sample. The resolver itself is asserted against the shared cases by component 7's tests.
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::settings::naming_draft::{Field, NamingPage};
use crate::tests::{FRAME, click};
use crate::{Rail, State, test_wait};
use rune::{App, Env, Key, KeyEvent, Mods, Runtime};
use serde_json::{Value, json};
use std::io::Write;
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Fake {
    /// Every call heard: method, path and body.
    heard: Vec<(String, String, String)>,
    /// The user's layer, as the server holds it.
    user: Value,
    /// What the read answers instead of the layers.
    broken: Option<(u16, String)>,
    /// What a save answers instead of success.
    refuse: Option<(u16, String)>,
    machines: Vec<Value>,
}

type Shared = Arc<Mutex<Fake>>;

fn defaults() -> Value {
    json!({ "branch": "{prefix}/{slug}", "root": "{repoParent}/{repo}-worktrees", "folder": "{branch}", "prefix": "fleet" })
}

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
    let body = req.body_text();
    let (method, path) = (req.method, req.path);
    let mut f = fake.lock().unwrap();
    // The live-events link negotiates in the background at any moment; it is not what a test asks about.
    if !path.starts_with("/hubs/") {
        f.heard.push((method.clone(), path.clone(), body.clone()));
    }
    let layers = |f: &Fake| json!({ "effective": defaults(), "user": f.user, "defaults": defaults(), "layers": {} }).to_string();
    let (status, reply) = match (method.as_str(), path.as_str()) {
        ("GET", "/api/worktrees/naming") => match f.broken.clone() {
            Some(broken) => broken,
            None => (200, layers(&f)),
        },
        ("PUT", "/api/worktrees/naming") => match f.refuse.clone() {
            Some(refused) => refused,
            None => {
                let sent: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
                let mut user = json!({});
                for k in ["branch", "root", "folder", "capture", "prefix"] {
                    if !sent[k].is_null() {
                        user[k] = sent[k].clone();
                    }
                }
                f.user = user;
                (200, layers(&f))
            }
        },
        ("GET", "/api/machines") => (200, json!({ "machines": f.machines }).to_string()),
        _ => (200, "{}".to_string()),
    };
    let out = format!(
        "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}",
        reply.len()
    );
    let _ = stream.write_all(out.as_bytes());
}

struct Rig {
    rt: Runtime<State>,
    queue: Receiver<Job>,
    fake: Shared,
}

fn link_to(base: String) -> (Link, Receiver<Job>) {
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    (Link::start(base, Post::new(move |job| drop(jobs.lock().unwrap().send(job)))), queue)
}

/// The app in a window tall enough to hold the whole Folders page, so every control is on screen to press.
fn tall() -> Runtime<State> {
    let mut rt = App::new(crate::spec()).headless_runtime((1280.0, 1900.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.frame(FRAME);
    rt
}

/// A connected app on Settings → Folders, the layers not yet read.
fn rig(user: Value) -> Rig {
    rig_with(|f| f.user = user)
}

fn rig_with(setup: impl FnOnce(&mut Fake)) -> Rig {
    let fake: Shared = Default::default();
    setup(&mut fake.lock().unwrap());
    let (link, queue) = link_to(serve(fake.clone()));
    let mut rt = tall();
    rt.update(move |s, _| {
        s.link = Some(link);
        s.rail = Rail::Settings;
        s.open_settings("folders", None);
    });
    Rig { rt, queue, fake }
}

impl Rig {
    fn wait(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        test_wait::drive(&mut self.rt, &self.queue, what, done, |s| format!("{:?}", s.settings.naming));
    }
    fn read(&mut self) {
        self.wait("the worktree naming to be read", |s| !s.settings.naming.loading && s.settings.naming.loaded_for.is_some());
        self.settle();
    }
    fn settle(&mut self) {
        self.rt.frame(FRAME);
        self.rt.frame(FRAME);
    }
    fn page(&self) -> &NamingPage {
        &self.rt.state.settings.naming
    }
    fn exists(&self, key: &str) -> bool {
        self.rt.elements().iter().any(|e| e.key.as_deref() == Some(key) && !e.exiting)
    }
    fn text(&self, key: &str) -> String {
        let all = self.rt.elements();
        let it = all.iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).unwrap_or_else(|| panic!("no element keyed {key:?}"));
        all.iter().filter(|e| e.path.starts_with(&it.path)).filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("")
    }
    fn sent(&self) -> Vec<Value> {
        let f = self.fake.lock().unwrap();
        f.heard.iter().filter(|(m, ..)| m == "PUT").map(|(.., b)| serde_json::from_str(b).unwrap()).collect()
    }
    fn type_into(&mut self, key: &str, words: &str) {
        click(&mut self.rt, key);
        self.rt.key(KeyEvent { key: Key::Text(words.into()), mods: Mods::default() });
        self.settle();
    }
    fn saved(&mut self) {
        self.wait("the save to finish", |s| !s.settings.naming.saving);
        self.settle();
    }
}

fn set(r: &mut Rig, field: Field, words: &str) {
    r.rt.update(|s, _| s.settings.naming.edit_mut(field).replace_text(""));
    r.type_into(&format!("naming-{}", field.key()), words);
}

// ─── The fields ─────────────────────────────────────────────────────────────────

#[test]
fn the_fields_start_from_the_users_own_layer_not_the_effective_templates() {
    let mut r = rig(json!({ "prefix": "pg", "branch": "{prefix}/{ticket}-{slug}", "capture": { "ticket": "[A-Z]{2,}-\\d+" } }));
    r.read();
    let p = r.page();
    assert_eq!(p.prefix.text, "pg");
    assert_eq!(p.branch.text, "{prefix}/{ticket}-{slug}");
    assert_eq!(p.ticket.text, "[A-Z]{2,}-\\d+");
    assert_eq!(p.root.text, "", "a field the user doesn't set is empty, its default shows as the placeholder");
    assert_eq!(p.folder.text, "");
    assert_eq!(p.defaults.root, "{repoParent}/{repo}-worktrees");
}

#[test]
fn the_card_has_its_heading_and_words_and_the_five_fields_in_order() {
    let mut r = rig(json!({}));
    r.read();
    assert!(r.exists("naming-card"));
    for f in Field::ALL {
        assert!(r.exists(&format!("naming-{}", f.key())), "{} has a box", f.label());
    }
    let order: Vec<f32> = Field::ALL
        .iter()
        .map(|f| r.rt.elements().into_iter().find(|e| e.key.as_deref() == Some(&format!("naming-{}", f.key()))).unwrap().rect[1])
        .collect();
    assert!(order.windows(2).all(|w| w[0] < w[1]), "prefix, branch, root, folder, ticket from top to bottom: {order:?}");
    assert!(r.text("naming-card").contains("How new worktrees are named."));
    assert!(r.text("naming-card").contains("weave.jsonc"));
}

#[test]
fn the_prefix_box_is_no_wider_than_220_pixels() {
    let mut r = rig(json!({}));
    r.read();
    let boxed = r.rt.elements().into_iter().find(|e| e.key.as_deref() == Some("naming-prefix")).unwrap();
    assert!(boxed.rect[2] <= 220.5, "got {}", boxed.rect[2]);
}

#[test]
fn it_says_what_went_wrong_when_the_layers_cant_be_read() {
    let mut r = rig_with(|f| f.broken = Some((500, json!({ "error": "The naming file is locked." }).to_string())));
    r.read();
    assert_eq!(r.text("naming-error"), "The naming file is locked.");
    let mut r = rig_with(|f| f.broken = Some((500, "boom".into())));
    r.read();
    assert_eq!(r.text("naming-error"), "Couldn't load worktree naming.");
}

// ─── The chips ──────────────────────────────────────────────────────────────────

#[test]
fn the_chips_are_the_ones_fleet_lists_for_each_field() {
    let names = |f: Field| f.tokens().iter().map(|t| t.name).collect::<Vec<_>>();
    assert_eq!(names(Field::Branch), ["slug", "user", "prefix", "ticket", "date", "repo", "shortid"]);
    assert_eq!(names(Field::Root), ["repoParent", "repo", "home", "user"]);
    assert_eq!(names(Field::Folder), ["branch", "slug", "ticket", "repo", "shortid"]);
    assert!(Field::Prefix.tokens().is_empty() && Field::Ticket.tokens().is_empty());
    let hint = |f: Field, n: &str| f.tokens().iter().find(|t| t.name == n).unwrap().hint;
    assert_eq!(hint(Field::Branch, "date"), "2026-09-21");
    assert_eq!(hint(Field::Root, "repoParent"), "beside the repository");
    assert_eq!(hint(Field::Folder, "branch"), "the resolved branch");
}

#[test]
fn a_chip_appends_its_token_to_the_end_of_its_field() {
    let mut r = rig(json!({}));
    r.read();
    click(&mut r.rt, "naming-token-branch-slug");
    click(&mut r.rt, "naming-token-branch-date");
    assert_eq!(r.page().branch.text, "{slug}{date}");
    click(&mut r.rt, "naming-token-root-home");
    assert_eq!(r.page().root.text, "{home}");
    assert_eq!(r.page().folder.text, "", "another field is left alone");
    assert_eq!(r.text("naming-token-branch-slug"), "{slug} from your message");
}

// ─── The preview ────────────────────────────────────────────────────────────────

#[test]
fn the_preview_names_the_sample_with_fleets_defaults_while_the_fields_are_empty() {
    let mut r = rig(json!({}));
    r.read();
    assert_eq!(r.text("naming-branch-line"), "fleet/plat-1841-add-rate-limiting-session");
    assert_eq!(r.text("naming-worktree-line"), "/home/you/source/lantern-worktrees/fleet-plat-1841-add-rate-limiting-session");
    assert!(!r.exists("naming-no-slug"));
}

#[test]
fn the_preview_follows_the_draft_and_a_ticket_capture_leaves_the_slug() {
    let mut r = rig(json!({}));
    r.read();
    set(&mut r, Field::Prefix, "pg");
    set(&mut r, Field::Branch, "{prefix}/{ticket}-{slug}");
    set(&mut r, Field::Ticket, "[A-Z]{2,}-\\d+");
    assert_eq!(r.text("naming-branch-line"), "pg/PLAT-1841-add-rate-limiting-session-create");
}

#[test]
fn the_preview_follows_the_sample() {
    let mut r = rig(json!({}));
    r.read();
    set(&mut r, Field::Prefix, "pg");
    r.rt.update(|s, _| s.settings.naming.sample.replace_text(""));
    r.settle();
    assert_eq!(r.text("naming-branch-line"), "pg/session-a1b2c3d4");
    r.type_into("naming-sample", "fix the login redirect");
    assert_eq!(r.text("naming-branch-line"), "pg/fix-login-redirect");
}

#[test]
fn an_empty_sample_says_the_session_id_stands_in_for_the_slug() {
    let mut r = rig(json!({}));
    r.read();
    r.rt.update(|s, _| s.settings.naming.sample.replace_text(""));
    r.settle();
    assert!(r.text("naming-no-slug").starts_with("This message gives no slug, so the session id stands in for it"));
    set(&mut r, Field::Branch, "{prefix}/work");
    assert!(!r.exists("naming-no-slug"), "a branch with no {{slug}} has nothing to stand in for");
}

#[test]
fn the_preview_is_of_an_invented_repository_and_never_asks_the_server() {
    let mut r = rig(json!({}));
    r.read();
    // Background reads (the machines, presence, refreshes) land at any moment, so count only what the naming card itself asks.
    let asked = |r: &Rig| r.fake.lock().unwrap().heard.iter().filter(|(_, p, _)| p.starts_with("/api/worktrees/naming")).count();
    let before = asked(&r);
    r.type_into("naming-sample", " more");
    set(&mut r, Field::Folder, "{repo}-{shortid}");
    assert_eq!(asked(&r), before, "typing asks for nothing");
    assert!(r.text("naming-worktree-line").ends_with("/lantern-a1b2c3d4"), "{}", r.text("naming-worktree-line"));
    assert!(!r.text("naming-worktree-line").contains("weave-fleet"));
}

#[test]
fn a_long_worktree_path_breaks_anywhere_inside_the_card() {
    let mut r = rig(json!({}));
    r.read();
    r.type_into("naming-sample", &" word".repeat(30));
    let card = r.rt.elements().into_iter().find(|e| e.key.as_deref() == Some("naming-card")).unwrap();
    let line = r.rt.elements().into_iter().find(|e| e.key.as_deref() == Some("naming-worktree-line")).unwrap();
    assert!(line.rect[0] + line.rect[2] <= card.rect[0] + card.rect[2], "the value stays inside the card");
}

// ─── Save ───────────────────────────────────────────────────────────────────────

#[test]
fn save_sends_null_for_an_empty_field_and_the_ticket_inside_capture() {
    let mut r = rig(json!({}));
    r.read();
    click(&mut r.rt, "naming-save");
    r.saved();
    assert_eq!(r.sent(), [json!({ "branch": null, "root": null, "folder": null, "capture": null, "prefix": null })]);
    set(&mut r, Field::Prefix, "pg");
    set(&mut r, Field::Ticket, "[A-Z]{2,}-\\d+");
    click(&mut r.rt, "naming-save");
    r.saved();
    assert_eq!(
        r.sent()[1],
        json!({ "branch": null, "root": null, "folder": null, "capture": { "ticket": "[A-Z]{2,}-\\d+" }, "prefix": "pg" })
    );
}

#[test]
fn the_answer_to_a_save_replaces_the_layers_and_the_fields_follow() {
    let mut r = rig(json!({}));
    r.read();
    set(&mut r, Field::Branch, "{prefix}/{slug}");
    click(&mut r.rt, "naming-save");
    r.saved();
    assert_eq!(r.page().user.branch.as_deref(), Some("{prefix}/{slug}"));
    assert_eq!(r.page().branch.text, "{prefix}/{slug}");
}

#[test]
fn save_is_off_while_it_works_and_shows_a_spinner() {
    let mut r = rig(json!({}));
    r.read();
    r.rt.update(|s, _| s.settings.naming.saving = true);
    r.settle();
    r.rt.update(|s, _| s.save_naming());
    assert!(r.sent().is_empty(), "a save under way is not started twice");
    click(&mut r.rt, "naming-save");
    click(&mut r.rt, "naming-reset");
    assert!(r.sent().is_empty(), "pressing either sends nothing");
    assert_eq!(r.page().prefix.text, "", "and Use Fleet's defaults leaves the fields alone");
}

#[test]
fn saved_shows_for_two_seconds_and_then_goes() {
    let mut r = rig(json!({}));
    r.read();
    click(&mut r.rt, "naming-save");
    r.saved();
    assert!(r.exists("naming-saved"));
    assert_eq!(r.text("naming-saved"), "Saved");
    r.rt.frame(1.0);
    assert!(r.exists("naming-saved"), "still there after a second");
    r.rt.frame(1.5);
    r.rt.frame(FRAME);
    assert!(!r.exists("naming-saved"), "gone after two");
}

#[test]
fn a_new_save_hides_the_old_saved_mark() {
    let mut r = rig(json!({}));
    r.read();
    click(&mut r.rt, "naming-save");
    r.saved();
    r.rt.update(|s, _| s.settings.naming.saving = false);
    r.rt.update(|s, _| s.save_naming());
    assert!(r.page().saved_until.is_none());
}

#[test]
fn the_server_refusing_a_template_is_shown_in_its_words_and_the_fields_stay() {
    let mut r =
        rig_with(|f| f.refuse = Some((400, json!({ "error": "The branch template needs {slug}, {shortid} or {branch}." }).to_string())));
    r.read();
    set(&mut r, Field::Branch, "{prefix}");
    click(&mut r.rt, "naming-save");
    r.saved();
    assert_eq!(r.text("naming-error"), "The branch template needs {slug}, {shortid} or {branch}.");
    assert_eq!(r.page().branch.text, "{prefix}", "what you typed stays");
    assert!(!r.exists("naming-saved"));
}

#[test]
fn a_refusal_without_words_says_it_couldnt_save() {
    let mut r = rig_with(|f| f.refuse = Some((500, "boom".into())));
    r.read();
    click(&mut r.rt, "naming-save");
    r.saved();
    assert_eq!(r.text("naming-error"), "Couldn't save worktree naming.");
}

#[test]
fn the_next_save_clears_the_error() {
    let mut r = rig_with(|f| f.refuse = Some((400, json!({ "error": "No." }).to_string())));
    r.read();
    click(&mut r.rt, "naming-save");
    r.saved();
    r.fake.lock().unwrap().refuse = None;
    click(&mut r.rt, "naming-save");
    r.saved();
    assert!(!r.exists("naming-error"));
    assert!(r.exists("naming-saved"));
}

// ─── Use Fleet's defaults ───────────────────────────────────────────────────────

#[test]
fn use_fleets_defaults_empties_every_field_and_saves() {
    let mut r = rig(json!({ "prefix": "pg", "folder": "{repo}-{slug}", "capture": { "ticket": "[A-Z]+-\\d+" } }));
    r.read();
    assert_eq!(r.page().prefix.text, "pg");
    click(&mut r.rt, "naming-reset");
    r.saved();
    assert_eq!(r.sent(), [json!({ "branch": null, "root": null, "folder": null, "capture": null, "prefix": null })]);
    for f in Field::ALL {
        assert_eq!(r.page().edit(f).text, "", "{} is empty", f.label());
    }
    assert!(r.exists("naming-saved"));
}

// ─── The machine ────────────────────────────────────────────────────────────────

#[test]
fn leaving_folders_forgets_the_page_and_coming_back_reads_it_again() {
    let mut r = rig(json!({ "prefix": "pg" }));
    r.read();
    r.rt.update(|s, _| s.open_settings("general", None));
    r.settle();
    assert!(r.page().loaded_for.is_none());
    r.rt.update(|s, _| s.open_settings("folders", None));
    r.read();
    assert_eq!(r.page().prefix.text, "pg");
    let gets = r.fake.lock().unwrap().heard.iter().filter(|(m, p, _)| m == "GET" && p == "/api/worktrees/naming").count();
    assert_eq!(gets, 2);
}

#[test]
fn with_no_fleet_the_page_works_on_fleets_own_defaults() {
    let mut rt = tall();
    rt.update(|s, _| {
        s.rail = Rail::Settings;
        s.open_settings("folders", None);
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(rt.elements().iter().any(|e| e.key.as_deref() == Some("naming-card")));
    assert_eq!(rt.state.settings.naming.defaults.prefix, "fleet");
    assert_eq!(rt.state.settings.naming.sample.text, crate::fleet::settings::naming_draft::SAMPLE);
}
