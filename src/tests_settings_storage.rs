//! Component 16, piece AC: Settings → Folders → Storage (`StorageSection.vue`). The six cases of Fleet's `StorageSection.test.ts`
//! under the same names (the last one is the native rule for when to measure), then what Fleet leaves untested: the refusal and
//! error lines, the totals' labels and the cache. Every call goes to a fake server on a port of its own, and the checkouts
//! are invented (`harbor-api`, `lumen-site`); nothing here can reach a real folder.
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::settings::storage::{Checkout, SIZE_TTL_SECS, Size, SizeCache, Sizes, Verdict, noun, parse_list, sum_of};
use crate::tests::FRAME;
use crate::{Rail, State, test_wait};
use rune::{App, Env, Runtime};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

const GB: u64 = 1024 * 1024 * 1024;

fn row(id: &str, repository: &str, title: &str, verdict: &str, reason: &str) -> Value {
    json!({ "workspaceId": id, "sessionId": format!("s-{id}"), "title": title, "repository": repository, "directory": format!("/w/{id}"),
        "branch": format!("fleet/{id}"), "verdict": verdict, "reason": reason, "unpushedCommits": null, "archivedAt": null })
}

fn list() -> Vec<Value> {
    vec![
        row("w1", "harbor-api", "Export to CSV", "remove", "Archived 9 days ago."),
        row("w2", "pocket-notes", "Fix note sync", "remove", "Archived 12 days ago."),
        row("w3", "lumen-site", "Checkout flow", "keep", "Has an ignored file you may want: .env.local"),
        row("w4", "lumen-site", "Hero refresh", "recent", "Archived yesterday."),
        row("w5", "harbor-api", "Add webhooks", "in-use", "The session isn't archived."),
    ]
}

#[derive(Default)]
struct Fake {
    heard: Vec<(String, String, String)>,
    checkouts: Vec<Value>,
    /// What `complete` says in a size answer.
    complete: Option<bool>,
    /// Size answers wait while this is set.
    hold: bool,
    asking: usize,
    most_at_once: usize,
    /// What the list read answers instead of the checkouts.
    broken: Option<(u16, String)>,
    /// What a clean answers.
    cleaned: Option<(u16, String)>,
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
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let body_at = loop {
        let n = stream.read(&mut chunk).unwrap_or(0);
        buf.extend_from_slice(&chunk[..n]);
        if let Some(at) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break at + 4;
        }
        if n == 0 {
            break buf.len();
        }
    };
    let head = String::from_utf8_lossy(&buf[..body_at]).to_string();
    let length = head
        .lines()
        .find_map(|l| l.to_lowercase().strip_prefix("content-length:").and_then(|v| v.trim().parse::<usize>().ok()))
        .unwrap_or(0);
    while buf.len() < body_at + length {
        let n = stream.read(&mut chunk).unwrap_or(0);
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let body = String::from_utf8_lossy(&buf[body_at..]).to_string();
    let mut first = head.lines().next().unwrap_or("").split(' ');
    let (method, path) = (first.next().unwrap_or("").to_string(), first.next().unwrap_or("").to_string());
    fake.lock().unwrap().heard.push((method.clone(), path.clone(), body.clone()));
    let (status, reply) = match (method.as_str(), path.as_str()) {
        ("GET", "/api/storage/worktrees") => {
            let f = fake.lock().unwrap();
            match f.broken.clone() {
                Some(broken) => broken,
                None => (200, json!({ "checkouts": f.checkouts, "archivedDays": 7 }).to_string()),
            }
        }
        ("GET", p) if p.starts_with("/api/storage/worktrees/") && p.ends_with("/size") => {
            {
                let mut f = fake.lock().unwrap();
                f.asking += 1;
                f.most_at_once = f.most_at_once.max(f.asking);
            }
            while fake.lock().unwrap().hold {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            let mut f = fake.lock().unwrap();
            f.asking -= 1;
            let id = p.trim_start_matches("/api/storage/worktrees/").trim_end_matches("/size");
            // An int64 can come as a string.
            let bytes = if id == "w2" { GB } else { 2 * GB };
            (200, json!({ "bytes": bytes.to_string(), "complete": f.complete.unwrap_or(true) }).to_string())
        }
        ("POST", "/api/storage/worktrees/clean") => match fake.lock().unwrap().cleaned.clone() {
            Some(reply) => reply,
            None => {
                let kept = row("w2", "pocket-notes", "Fix note sync", "keep", "A terminal is open in it.");
                (200, json!({ "removed": [list()[0].clone()], "kept": [kept] }).to_string())
            }
        },
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

fn rig_with(setup: impl FnOnce(&mut Fake)) -> Rig {
    let fake: Shared = Default::default();
    {
        let mut f = fake.lock().unwrap();
        f.checkouts = list();
        setup(&mut f);
    }
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let link = Link::start(serve(fake.clone()), Post::new(move |job| drop(jobs.lock().unwrap().send(job))));
    let mut rt = App::new(crate::spec()).headless_runtime((1280.0, 2400.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.frame(FRAME);
    rt.update(move |s, _| {
        s.link = Some(link);
        s.rail = Rail::Settings;
        s.open_settings("folders", None);
    });
    Rig { rt, queue, fake }
}

fn rig() -> Rig {
    rig_with(|_| {})
}

impl Rig {
    fn wait(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        test_wait::drive(&mut self.rt, &self.queue, what, done, |s| format!("{:?}", s.settings.storage));
    }
    /// The counts are in and every size has come back.
    fn measured(&mut self) {
        self.wait("the checkouts and their sizes", |s| {
            let p = &s.settings.storage;
            !p.loading && p.loaded_for.is_some() && !p.checkouts.is_empty() && p.queue.is_empty() && p.in_flight == 0
        });
        self.settle();
    }
    fn settle(&mut self) {
        self.rt.frame(FRAME);
        self.rt.frame(FRAME);
    }
    fn page(&self) -> &crate::fleet::settings::storage::StoragePage {
        &self.rt.state.settings.storage
    }
    fn exists(&self, key: &str) -> bool {
        self.rt.elements().iter().any(|e| e.key.as_deref() == Some(key) && !e.exiting)
    }
    fn text(&self, key: &str) -> String {
        let all = self.rt.elements();
        let it = all.iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).unwrap_or_else(|| panic!("no element keyed {key:?}"));
        all.iter().filter(|e| e.path.starts_with(&it.path)).filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("")
    }
    fn press(&mut self, key: &str) {
        crate::tests::click(&mut self.rt, key);
        self.settle();
    }
    fn size_calls(&self) -> usize {
        self.fake.lock().unwrap().heard.iter().filter(|(m, p, _)| m == "GET" && p.ends_with("/size")).count()
    }
    /// Away from Folders and back, a frame between: the card is forgotten and read again.
    fn leave_and_return(&mut self) {
        self.rt.update(|s, _| s.open_settings("general", None));
        self.settle();
        self.rt.update(|s, _| s.open_settings("folders", None));
    }
}

#[test]
fn shows_the_counts_fills_sizes_in_and_previews_what_would_go() {
    let mut r = rig();
    r.measured();
    assert_eq!(r.text("storage-total"), "9.0 GB in 5 checkouts");
    assert!(r.text("storage-split-remove").contains("3.0 GB"));
    assert!(r.text("storage-split-remove").contains("2 could be freed"));
    assert!(r.text("storage-split-keep").contains("1 need a look"));
    assert!(r.text("storage-split-in-use").contains("2 in use"), "in use counts the recent one too");

    r.press("storage-preview");
    let panel = r.text("storage-preview-panel");
    assert!(panel.contains("Would remove 2 checkouts and free 3.0 GB"), "{panel}");
    assert!(panel.contains("1 would be kept, and 1 more is too recent"), "{panel}");
    assert!(panel.contains("harbor-api · Export to CSV"), "{panel}");
    assert!(panel.contains("Has an ignored file you may want: .env.local"), "{panel}");
    assert!(panel.contains("will remove") && panel.contains("will keep"));
    assert!(!panel.contains("Hero refresh") && !panel.contains("Add webhooks"), "recent and in-use are counted, not listed");
}

#[test]
fn removes_the_would_remove_checkouts_and_reports_what_went_and_what_stayed() {
    let mut r = rig();
    r.measured();
    r.press("storage-preview");
    r.press("storage-confirm");
    r.wait("the clean to finish", |s| !s.settings.storage.cleaning && s.settings.storage.report.is_some());
    r.measured();

    let body =
        r.fake.lock().unwrap().heard.iter().find(|(m, p, _)| m == "POST" && p == "/api/storage/worktrees/clean").map(|h| h.2.clone());
    assert_eq!(serde_json::from_str::<Value>(&body.expect("a clean was sent")).unwrap(), json!({ "workspaceIds": ["w1", "w2"] }));
    let report = r.text("storage-report");
    assert!(report.contains("Freed 2.0 GB"), "{report}");
    assert!(report.contains("Removed 1 checkout. Kept 2, and 1 is too recent."), "{report}");
    // What the preview already kept is in the report too, after what the run kept.
    assert!(report.contains("Has an ignored file you may want: .env.local"), "{report}");
    assert!(report.contains("A terminal is open in it."), "{report}");
    assert!(report.contains("Each removed session shows “Checkout removed”."));
    assert!(!r.page().previewing, "the preview closes");
    let reads = r.fake.lock().unwrap().heard.iter().filter(|(m, p, _)| m == "GET" && p == "/api/storage/worktrees").count();
    assert_eq!(reads, 2, "the card reads again after the clean");
}

#[test]
fn disables_preview_when_nothing_could_be_freed() {
    let mut r = rig_with(|f| {
        f.checkouts = vec![list()[4].clone()];
        f.complete = Some(false);
    });
    r.measured();
    let preview = r.rt.elements().into_iter().find(|e| e.key.as_deref() == Some("storage-preview")).expect("the Preview button");
    assert!(!preview.focusable, "Preview is off with nothing to remove");
    assert!(r.text("storage-hint").contains("Nothing to remove right now. Only checkouts of sessions archived for 7 days or more can go."));
    assert!(r.text("storage-total").contains("≥ 1.0 GB") || r.text("storage-total").contains("≥ 2.0 GB"), "{}", r.text("storage-total"));
    r.press("storage-preview");
    assert!(!r.exists("storage-preview-panel"), "a disabled Preview opens nothing");
}

#[test]
fn shows_the_sizes_it_just_measured_on_the_next_visit_instead_of_walking_every_folder_again() {
    let mut r = rig();
    r.measured();
    assert_eq!(r.size_calls(), 5);
    r.leave_and_return();
    r.measured();
    assert_eq!(r.size_calls(), 5, "no new walk");
    assert_eq!(r.text("storage-total"), "9.0 GB in 5 checkouts");
}

#[test]
fn measures_again_once_the_remembered_sizes_are_old() {
    let mut r = rig();
    r.measured();
    r.rt.frame(6.0 * 60.0); // the app's clock follows the frames
    r.leave_and_return();
    r.measured();
    assert_eq!(r.size_calls(), 10);
}

#[test]
fn waits_to_measure_until_folders_is_open_and_asks_for_three_at_a_time() {
    // Native rule (PORT.md, the decision): sizes start when the page opens, not before, three at a time.
    let mut r = rig_with(|f| f.hold = true);
    r.wait("the counts", |s| !s.settings.storage.loading && !s.settings.storage.checkouts.is_empty());
    r.settle();
    assert_eq!(r.text("storage-total"), "— in 5 checkouts", "counts first, sizes later");
    r.wait("three folders being measured", |s| s.settings.storage.in_flight == 3);
    assert_eq!(r.page().queue.len(), 2, "the other two wait their turn");
    assert_eq!(r.page().queue.iter().collect::<Vec<_>>(), ["w4", "w5"], "the first three (w1, w2 removable, then w3) are out already");
    r.fake.lock().unwrap().hold = false;
    r.measured();
    assert_eq!(r.fake.lock().unwrap().most_at_once, 3);
    assert_eq!(r.text("storage-total"), "9.0 GB in 5 checkouts");

    // Away from Folders nothing is measured at all.
    let mut r = rig();
    r.rt.update(|s, _| s.open_settings("general", None));
    r.settle();
    assert_eq!(r.size_calls(), 0);
    assert!(r.page().checkouts.is_empty());
}

#[test]
fn says_so_when_the_checkouts_cannot_be_read_and_when_a_clean_fails() {
    let mut r = rig_with(|f| f.broken = Some((500, json!({ "error": "The disk is busy." }).to_string())));
    r.wait("the read to fail", |s| !s.settings.storage.loading && s.settings.storage.error.is_some());
    r.settle();
    assert_eq!(r.text("storage-error"), "The disk is busy.");
    assert!(!r.exists("storage-total"), "nothing to count");

    let mut r = rig_with(|f| f.cleaned = Some((500, "{}".into())));
    r.measured();
    r.press("storage-preview");
    r.press("storage-confirm");
    r.wait("the clean to fail", |s| s.settings.storage.error.is_some());
    r.settle();
    assert_eq!(r.text("storage-error"), "Couldn't clean the checkouts.");
    assert!(r.exists("storage-total"), "the list stays drawn above the error");
    r.press("storage-cancel");
    assert!(!r.exists("storage-preview-panel"));
}

#[test]
fn the_storage_anchor_scrolls_once_then_lets_go() {
    use crate::fleet::settings::anchor::{Settle, settle};
    use crate::fleet::settings_nav::Page;
    assert_eq!(settle(&Page::Folders, Some("storage")), Settle::Scroll { key: "settings-anchor-storage".into(), release: true });
    assert_eq!(settle(&Page::Folders, Some("elsewhere")), Settle::Wait, "an unknown anchor is kept");
    let mut r = rig();
    r.rt.update(|s, _| s.open_settings("folders", Some("storage")));
    r.settle();
    assert!(r.exists("settings-anchor-storage"));
    assert_eq!(r.rt.state.settings.anchor, None, "spent after the scroll");
}

#[test]
fn totals_say_dash_for_nothing_measured_and_at_least_for_a_partial_sum() {
    let c = |id: &str| Checkout {
        workspace_id: id.into(),
        title: String::new(),
        repository: String::new(),
        verdict: Verdict::Keep,
        reason: String::new(),
    };
    let (a, b) = (c("a"), c("b"));
    let mut sizes = Sizes::new();
    assert_eq!(sum_of(&[&a, &b], &sizes).1, "—");
    assert_eq!(sum_of(&[], &sizes).1, "0 B", "an empty list is zero");
    sizes.insert("a".into(), Size { bytes: 3.0 * GB as f64, complete: true });
    assert_eq!(sum_of(&[&a, &b], &sizes).1, "≥ 3.0 GB");
    sizes.insert("b".into(), Size { bytes: 512.0 * 1024.0, complete: true });
    assert_eq!(sum_of(&[&a, &b], &sizes).1, "3.0 GB");
    sizes.insert("b".into(), Size { bytes: 512.0 * 1024.0, complete: false });
    assert_eq!(sum_of(&[&a, &b], &sizes).1, "≥ 3.0 GB");
    assert_eq!((noun(1), noun(0), noun(2)), ("1 checkout".into(), "0 checkouts".into(), "2 checkouts".into()));
}

#[test]
fn the_size_cache_keeps_each_machines_sizes_for_five_minutes() {
    let mut cache = SizeCache::default();
    let size = Size { bytes: 5.0, complete: true };
    cache.remember("home", "w1", size, 100.0);
    assert_eq!(cache.recent("home", 100.0 + SIZE_TTL_SECS - 1.0).len(), 1);
    assert!(cache.recent("home", 100.0 + SIZE_TTL_SECS).is_empty(), "old enough to measure again");
    assert!(cache.recent("falcon", 100.0).is_empty(), "another machine's own");
}

#[test]
fn an_answer_that_is_not_a_list_is_the_fallback() {
    assert_eq!(parse_list(200, "[]").unwrap_err(), "Couldn't load the worktree checkouts.");
    assert_eq!(parse_list(503, "{\"message\":\"Starting up.\"}").unwrap_err(), "Starting up.");
    let got = parse_list(200, "{\"checkouts\":[],\"archivedDays\":\"14\"}").unwrap();
    assert_eq!(got.archived_days, 14, "a string count reads as a number");
}
