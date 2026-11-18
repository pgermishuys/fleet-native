//! Revert in the Changes review (component 12, piece I): `ChangesReview.test.ts` "asks before reverting a hunk…", "reverts nothing when
//! the confirm is cancelled", "refuses to revert from a stale picture of the file" and "reverting files that aren't plain edits", under
//! their names. A fake Fleet holds one invented session folder: it serves the list and each file's diff, answers the read, write and
//! restore, and logs every request, so a test counts only the requests it means.
use crate::fleet::actions::CONFIRM_ACTION;
use crate::fleet::diff_contents::Entry;
use crate::fleet::link::{Job, Link, Post};
use crate::tests::{FRAME, app, click, finish, read_request};
use crate::{State, test_wait};
use rune::Runtime;
use serde_json::{Value, json};
use std::io::Write;
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

type Rt = Runtime<State>;

const BEFORE: &str = "export interface Note {\n  id: string;\n  title: string;\n}\n\nconst a = 1;\nconst b = 2;\nconst c = 3;\nconst d = 4;\nconst e = 5;\nconst f = oldName(1);\n";

fn after() -> String {
    BEFORE.replace("  title: string;", "  title: string;\n  tags: string[];").replace("oldName", "newName")
}

fn item(file: &str, status: &str, before: &str, after: &str) -> Value {
    let (a, d) = (after.lines().count() as u64, before.lines().count() as u64);
    json!({ "file": file, "status": status, "additions": a, "deletions": d, "before": before, "after": after })
}

/// What the fake Fleet answers.
struct Server {
    /// The file as it is on disk (`None`: no such file), and the hash its read gives.
    disk: Option<String>,
    /// The status a write and a restore answer with.
    save: u16,
    /// The list empties once a write or restore took (a file reverted clean leaves it).
    clean_after: bool,
}

struct Rig {
    rt: Rt,
    jobs: Receiver<Job>,
    log: Arc<Mutex<Vec<String>>>,
}

/// A session whose changes are `items`, over a fake Fleet, with every file's diff loaded.
fn rig(items: Vec<Value>, server: Server) -> Rig {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
    let base = format!("http://{}", listener.local_addr().expect("an address"));
    let log = Arc::new(Mutex::new(Vec::<String>::new()));
    let written = log.clone();
    let list = Arc::new(Mutex::new(items.clone()));
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let request = read_request(&mut stream);
            let line = request.lines().next().unwrap_or("").to_string();
            let body = request.split_once("\r\n\r\n").map_or("", |(_, b)| b);
            written.lock().unwrap().push(format!("{line} {body}"));
            let method = line.split(' ').next().unwrap_or("");
            let (status, answer) = if line.contains("/diffs/file") {
                let path = line.split("path=").nth(1).and_then(|r| r.split(' ').next()).unwrap_or("").replace("%2F", "/");
                let found = items.iter().find(|i| i["file"] == path.as_str()).cloned();
                found.map_or((404, "{}".to_string()), |i| (200, i.to_string()))
            } else if line.contains("/diffs") {
                let summary: Vec<Value> = list
                    .lock()
                    .unwrap()
                    .iter()
                    .map(|i| json!({ "file": i["file"], "status": i["status"], "additions": i["additions"], "deletions": i["deletions"], "isBinary": i["isBinary"] }))
                    .collect();
                (200, json!({ "diffs": summary, "available": true, "base": { "kind": "session" } }).to_string())
            } else if method == "GET" && line.contains("/files/content") {
                match &server.disk {
                    Some(text) => (200, json!({ "path": "x", "content": text, "hash": "h1" }).to_string()),
                    None => (404, "{}".to_string()),
                }
            } else if line.contains("/files/content") || line.contains("/files/restore") {
                if server.save == 200 {
                    if server.clean_after {
                        list.lock().unwrap().clear();
                    }
                    (200, json!({ "saved": true, "hash": "h2" }).to_string())
                } else {
                    (server.save, json!({ "content": "something newer", "hash": "h3", "error": "refused" }).to_string())
                }
            } else {
                (404, "{}".to_string())
            };
            let _ = write!(stream, "HTTP/1.1 {status} X\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{answer}", answer.len());
            finish(&mut stream);
        }
    });
    let mut rt = app();
    let (tx, jobs) = channel::<Job>();
    let tx = Mutex::new(tx);
    let link = Link::start(base, Post::new(move |job| drop(tx.lock().unwrap().send(job))));
    rt.update(move |s, _| {
        s.show_changes_scene(crate::mock::changes::Case::Loading, true);
        s.link = Some(link);
        s.right.review = true;
        let id = s.diffs.session().to_string();
        s.review.open(&id);
    });
    let mut rig = Rig { rt, jobs, log };
    // The first list comes from the server, as it does for a session opened.
    rig.rt.update(|s, _| s.refetch_diffs());
    rig.settle("the list", |s| s.diffs.has_loaded);
    let paths: Vec<String> = rig.rt.state.diffs.files().iter().map(|f| f.file.clone()).collect();
    rig.rt.update(|s, _| {
        let files = s.review_files();
        s.review.decide(&files);
    });
    for path in paths {
        rig.settle("a file's contents", |s| matches!(s.diff_contents.entry(&path), Some(Entry::Ready(_))));
    }
    rig
}

impl Rig {
    fn settle(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        test_wait::drive(&mut self.rt, &self.jobs, what, done, test_wait::app_state);
        self.rt.frame(FRAME);
    }

    /// Counts the logged requests that start with `what` ("PUT", "/files/restore", "GET /api/sessions/x/diffs/file").
    fn asked(&self, what: &str) -> Vec<String> {
        self.log.lock().unwrap().iter().filter(|l| l.contains(what)).cloned().collect()
    }

    fn key_with(&self, prefix: &str) -> String {
        let keys: Vec<String> = self.rt.elements().into_iter().filter_map(|e| e.key).filter(|k| k.starts_with(prefix)).collect();
        keys.into_iter().next().unwrap_or_else(|| panic!("nothing keyed {prefix}…"))
    }

    /// Presses a revert button, then answers the question: `Some(yes)` answers it, `None` leaves it open.
    fn revert(&mut self, key: &str, answer: Option<bool>) {
        click(&mut self.rt, key);
        match answer {
            Some(true) => {
                click(&mut self.rt, CONFIRM_ACTION);
                self.settle("the revert to finish", |s| !s.review.reverting && s.ui.confirm.is_none());
            }
            Some(false) => click(&mut self.rt, crate::fleet::actions::CONFIRM_CANCEL),
            None => {}
        }
    }

    /// Waits until the list asked again after the revert has come and the file's diff, if it is still in it, has too.
    fn settled_after(&mut self, lists_before: usize) {
        let log = self.log.clone();
        self.settle("the list asked again", |_| {
            log.lock().unwrap().iter().filter(|l| l.contains("/diffs ") || l.ends_with("/diffs ")).count() > lists_before
        });
        for _ in 0..30 {
            self.rt.update(|_, _| {});
            while let Ok(job) = self.jobs.try_recv() {
                self.rt.update(job);
            }
            self.rt.frame(FRAME);
        }
    }

    fn lists(&self) -> usize {
        self.log.lock().unwrap().iter().filter(|l| l.starts_with("GET /api/sessions/") && l.contains("/diffs HTTP")).count()
    }

    fn notice(&self) -> Option<String> {
        self.rt.state.review.notice.clone()
    }
}

fn body(request: &str) -> Value {
    let start = request.find('{').unwrap_or(0);
    serde_json::from_str(&request[start..]).unwrap_or(Value::Null)
}

fn plain(disk: Option<String>) -> Server {
    Server { disk, save: 200, clean_after: false }
}

// ChangesReview.test.ts: "asks before reverting a hunk, then writes the file with that hunk undone"
#[test]
fn asks_before_reverting_a_hunk_then_writes_the_file_with_that_hunk_undone() {
    let mut r = rig(vec![item("src/notes.ts", "modified", BEFORE, &after())], plain(Some(after())));
    let key = r.key_with("review-revert-hunk-src/notes.ts-");
    r.revert(&key, None);
    assert_eq!(r.asked("PUT").len(), 0, "nothing is written before the answer");
    let c = r.rt.state.ui.confirm.clone().expect("a question");
    assert_eq!(
        (c.title.as_str(), c.subject.as_str(), c.confirm_label.as_str(), c.destructive),
        ("Revert this change?", "src/notes.ts", "Revert change", true)
    );
    assert_eq!(c.body, "Puts these lines back as they were. The agent's other edits to the file stay.");
    let lists = r.lists();
    click(&mut r.rt, CONFIRM_ACTION);
    r.settle("the write", |s| !s.review.reverting && s.ui.confirm.is_none());
    r.settled_after(lists);
    let writes = r.asked("PUT");
    assert_eq!(writes.len(), 1, "{writes:?}");
    let sent = body(&writes[0]);
    assert_eq!(sent["path"], "src/notes.ts");
    assert_eq!(sent["baseHash"], "h1");
    let text = sent["content"].as_str().unwrap();
    assert!(text.contains("  title: string;\n}"), "{text}");
    assert!(!text.contains("tags"), "{text}");
    assert!(text.contains("newName"), "the agent's other edit stays: {text}");
    assert!(r.lists() > lists, "the list is asked again");
}

// ChangesReview.test.ts: "reverts nothing when the confirm is cancelled"
#[test]
fn reverts_nothing_when_the_confirm_is_cancelled() {
    let mut r = rig(vec![item("src/notes.ts", "modified", BEFORE, &after())], plain(Some(after())));
    let key = r.key_with("review-revert-file-src/notes.ts");
    r.revert(&key, Some(false));
    for _ in 0..10 {
        r.rt.frame(FRAME);
    }
    assert!(r.rt.state.ui.confirm.is_none());
    assert!(r.asked("PUT").is_empty() && r.asked("/files/restore").is_empty() && r.asked("/files/content").is_empty());
}

#[test]
fn a_file_asks_in_words_that_fit_what_happened_to_it() {
    let mut r = rig(vec![item("src/notes.ts", "modified", BEFORE, &after())], plain(None));
    let key = r.key_with("review-revert-file-src/notes.ts");
    r.revert(&key, None);
    let c = r.rt.state.ui.confirm.clone().expect("a question");
    assert_eq!((c.title.as_str(), c.confirm_label.as_str()), ("Revert this file?", "Revert file"));
    assert_eq!(c.body, "Puts the file back as it was before the agent changed it. The agent's edits to it are lost.");
    use crate::fleet::diffs::FileStatus::{Added, Deleted};
    use crate::ui::confirm::Confirm;
    let call = crate::fleet::actions::Call::Revert { path: "n.txt".into(), range: None };
    assert_eq!(Confirm::revert_file("n.txt", Added, call.clone()).body, "Deletes the file: the session created it. Its contents are lost.");
    assert_eq!(Confirm::revert_file("n.txt", Deleted, call).body, "Puts the deleted file back as it was.");
}

// ChangesReview.test.ts: "refuses to revert from a stale picture of the file"
#[test]
fn refuses_to_revert_from_a_stale_picture_of_the_file() {
    let mut r = rig(vec![item("src/notes.ts", "modified", BEFORE, &after())], plain(Some("something newer".into())));
    let key = r.key_with("review-revert-file-src/notes.ts");
    let lists = r.lists();
    r.revert(&key, Some(true));
    r.settled_after(lists);
    assert!(r.asked("PUT").is_empty(), "{:?}", r.asked("PUT"));
    assert!(r.notice().is_some_and(|n| n.contains("changed while you were reviewing")), "{:?}", r.notice());
    assert!(r.rt.find("review-notice").is_some(), "the notice is drawn above the files");
    // Dismiss takes it away.
    click(&mut r.rt, "review-notice-dismiss");
    assert!(r.rt.find("review-notice").is_none());
}

// "a deleted file is put back from the base, not saved over a hash it doesn't have"
#[test]
fn a_deleted_file_is_put_back_from_the_base_not_saved_over_a_hash_it_doesnt_have() {
    let mut r = rig(vec![item("package.json", "deleted", "{\r\n}\r\n", "")], plain(None));
    let key = r.key_with("review-revert-file-package.json");
    r.revert(&key, Some(true));
    assert!(r.asked("PUT").is_empty());
    assert!(r.asked("/files/content").is_empty(), "a deleted file isn't read first");
    let restores = r.asked("/files/restore");
    assert_eq!(restores.len(), 1, "{restores:?}");
    assert_eq!(body(&restores[0]), json!({ "path": "package.json", "content": "{\r\n}\r\n", "baseHash": null }));
}

// "a new file is deleted, with the hash of what is on disk"
#[test]
fn a_new_file_is_deleted_with_the_hash_of_what_is_on_disk() {
    let mut r = rig(vec![item("new.txt", "added", "", "hi\n")], plain(Some("hi\n".into())));
    let key = r.key_with("review-revert-file-new.txt");
    r.revert(&key, Some(true));
    let restores = r.asked("/files/restore");
    assert_eq!(restores.len(), 1, "{restores:?}");
    assert_eq!(body(&restores[0]), json!({ "path": "new.txt", "content": null, "baseHash": "h1" }));
}

// "a file reverted clean is not asked for again once it has left the diff"
#[test]
fn a_file_reverted_clean_is_not_asked_for_again_once_it_has_left_the_diff() {
    let mut r = rig(vec![item("new.txt", "added", "", "hi\n")], Server { disk: Some("hi\n".into()), save: 200, clean_after: true });
    let for_file = |r: &Rig| r.asked("/diffs/file").len();
    let before = for_file(&r);
    assert!(before > 0);
    let key = r.key_with("review-revert-file-new.txt");
    let lists = r.lists();
    r.revert(&key, Some(true));
    r.settled_after(lists);
    assert!(r.rt.state.diffs.files().is_empty(), "the list no longer holds it");
    assert_eq!(for_file(&r), before);
}

// "a new file that changed since is kept and the reader is told"
#[test]
fn a_new_file_that_changed_since_is_kept_and_the_reader_is_told() {
    let mut r = rig(vec![item("new.txt", "added", "", "hi\n")], plain(Some("hi there\n".into())));
    let key = r.key_with("review-revert-file-new.txt");
    let lists = r.lists();
    r.revert(&key, Some(true));
    r.settled_after(lists);
    assert!(r.asked("/files/restore").is_empty());
    assert!(r.notice().is_some_and(|n| n.contains("changed while you were reviewing")), "{:?}", r.notice());
}

// "reverting a hunk in a CRLF file keeps every other line CRLF"
#[test]
fn reverting_a_hunk_in_a_crlf_file_keeps_every_other_line_crlf() {
    let lines = ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"];
    let before = lines.join("\r\n") + "\r\n";
    let after = lines.iter().map(|l| if *l == "b" { "B" } else { l }).collect::<Vec<_>>().join("\r\n") + "\r\n";
    let mut r = rig(vec![item("crlf.txt", "modified", &before, &after)], plain(Some(after.clone())));
    let key = r.key_with("review-revert-hunk-crlf.txt-");
    r.revert(&key, Some(false));
    click(&mut r.rt, &key);
    click(&mut r.rt, CONFIRM_ACTION);
    r.settle("the write", |s| !s.review.reverting && s.ui.confirm.is_none());
    let writes = r.asked("PUT");
    assert_eq!(writes.len(), 1, "{writes:?}");
    assert_eq!(body(&writes[0])["content"], before.as_str());
}

// 10.4, 10.5, 10.6: a 409 says the file wasn't touched
#[test]
fn a_refused_write_says_nothing_was_reverted_and_a_failure_says_why() {
    let conflict = |what: &str, status: &str, before: &str, after: &str, disk: &str, words: &str| {
        let mut r = rig(vec![item(what, status, before, after)], Server { disk: Some(disk.into()), save: 409, clean_after: false });
        let key = r.key_with(&format!("review-revert-file-{what}"));
        let lists = r.lists();
        r.revert(&key, Some(true));
        r.settled_after(lists);
        assert_eq!(r.notice().as_deref(), Some(words));
    };
    conflict("a.txt", "modified", "1\n", "2\n", "2\n", "a.txt changed on disk, so nothing was reverted.");
    conflict("b.txt", "added", "", "x\n", "x\n", "b.txt changed on disk, so nothing was removed.");
    conflict("c.txt", "deleted", "x\n", "", "", "c.txt exists again, so nothing was reverted.");
    // A server that refuses with its reason, or can't read the file at all.
    let mut r = rig(vec![item("d.txt", "modified", "1\n", "2\n")], Server { disk: None, save: 200, clean_after: false });
    let key = r.key_with("review-revert-file-d.txt");
    r.revert(&key, Some(true));
    assert_eq!(r.notice().as_deref(), Some("Could not revert d.txt: Failed to read file: 404 Not Found"));
    assert!(r.asked("PUT").is_empty());
    let mut r = rig(vec![item("e.txt", "modified", "1\n", "2\n")], Server { disk: Some("2\n".into()), save: 500, clean_after: false });
    let key = r.key_with("review-revert-file-e.txt");
    r.revert(&key, Some(true));
    assert_eq!(r.notice().as_deref(), Some("Could not revert e.txt: refused"));
}

// 10.3: nothing happens unless the file's contents are loaded
#[test]
fn nothing_is_reverted_unless_the_files_contents_are_loaded() {
    let mut r = rig(vec![item("a.txt", "modified", "1\n", "2\n")], plain(Some("2\n".into())));
    r.rt.update(|s, _| {
        let id = s.diffs.session().to_string();
        s.diff_contents.put(&id, "a.txt", Entry::Loading);
        s.start_revert("a.txt", None);
    });
    assert!(!r.rt.state.review.reverting);
    assert!(r.asked("/files/content").is_empty());
}

// 6.4: a binary or very large file can't be reverted whole
#[test]
fn a_binary_file_cannot_be_reverted_from_here() {
    let mut r =
        rig(vec![json!({ "file": "logo.png", "status": "modified", "additions": 0, "deletions": 0, "isBinary": true })], plain(None));
    let key = r.key_with("review-revert-file-logo.png");
    click(&mut r.rt, &key);
    assert!(r.rt.state.ui.confirm.is_none(), "the button is off");
}
