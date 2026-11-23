//! Piece M of the New session page: `@`, pictures and the GitHub place (`NewSessionComposer.test.ts`: "keyboard", "GitHub preset",
//! "@ references"; rows A8, A9, A7, A19), against the fake Fleet of `tests_new_send` with folders and sessions to list.
use crate::fleet::new_session::request::{Folder, GitHubPreset};
use crate::fleet::store::Route;
use crate::tests::{FRAME, app, click};
use crate::tests_new_send::{ROCKET, Rig, rig};
use rune::{Key, KeyEvent, Mods};
use serde_json::json;

const COMET: &str = "/home/me/src/comet";
const BILLING: &str = "Pick up where @billing-retries-spike left off: finish @src/billing/retry-policy.ts and @docs/billing/ ";

fn issue() -> GitHubPreset {
    GitHubPreset {
        source_type: "github-issue".into(),
        owner: "acme".into(),
        repo: "rocket".into(),
        number: 42,
        title: "Login redirect loops".into(),
        body: None,
        html_url: "https://github.com/acme/rocket/issues/42".into(),
        repo_full_name: "acme/rocket".into(),
        suggested_branch: Some("fix/issue-42".into()),
    }
}

/// A rig whose Fleet lists a session and the files of two folders, and whose harness takes pictures when `images`.
fn listing(images: bool) -> Rig {
    let mut r = rig();
    {
        let mut fake = r.fake.lock().unwrap();
        fake.folder_files.insert(ROCKET.into(), vec!["src/billing/retry-policy.ts".into(), "src/app.ts".into(), "docs/billing/".into()]);
        fake.folder_files.insert(COMET.into(), vec!["docs/billing/".into(), "README.md".into()]);
        fake.sessions = vec![json!({ "sessionStatus": "idle", "harnessType": "opencode", "workspaceDirectory": ROCKET,
            "session": { "id": "ses-billing", "title": "Billing retries spike", "time": { "created": 1, "updated": 1 } } })];
    }
    r.rt.update(move |s, _| {
        let home = s.feeds.harnesses.get_mut(crate::fleet::settings::HOME_MACHINE_KEY).expect("home's harnesses");
        for h in &mut home.harnesses.all {
            h.supports_images = images;
        }
    });
    r.rt.frame(FRAME);
    r
}

impl Rig {
    /// `typeAndWait`: types at the caret, then waits for the `@` list to have what the text asks for.
    fn type_and_wait(&mut self, words: &str, wanted: &str) {
        self.text_at_end(words);
        let wanted = wanted.to_string();
        self.wait("the @ list", move |s| s.new_popup().is_some_and(|p| p.items.iter().any(|i| i.value.contains(&wanted))));
    }

    fn text_at_end(&mut self, words: &str) {
        click(&mut self.rt, "new-session-message");
        self.rt.key(KeyEvent { key: Key::End, mods: Mods { cmd: true, ..Mods::default() } });
        self.rt.key(KeyEvent { key: Key::Text(words.into()), mods: Mods::default() });
        self.rt.frame(FRAME);
        self.rt.frame(FRAME);
    }

    fn pick_both(&mut self) {
        self.type_and_wait("Pick up where @bill", "session:ses-billing");
        self.press(Key::Tab, Mods::default());
        self.type_and_wait("left off: finish @retry", "retry-policy");
        self.press(Key::Enter, Mods::default());
        self.type_and_wait("and @docs/bil", "docs/billing/");
        self.press(Key::Enter, Mods::default());
    }

    fn box_text(&self) -> String {
        self.rt.state.new_box.edit.text.clone()
    }

    fn tinted(&self) -> Vec<String> {
        let text = self.box_text();
        self.rt.state.new_references().into_iter().map(|r| text[r.range].to_string()).collect()
    }

    fn picture(&mut self) -> std::path::PathBuf {
        // One folder per call: tests share a process, and a sibling rewriting the same file truncates it under the reading thread.
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("fleet-native-pictures-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a folder");
        let path = dir.join("login-error.png");
        std::fs::write(&path, [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]).expect("a picture");
        path
    }
}

// ─── keyboard: images ───────────────────────────────────────────────────────────

#[test]
fn keyboard_a_pasted_image_goes_with_the_first_prompt() {
    let mut r = listing(true);
    r.text("What is wrong here?");
    let path = r.picture();
    r.rt.update(move |s, tx| s.add_files(&[path], tx));
    r.wait("the picture to be read", |s| s.held_images().len() == 1);
    r.rt.frame(FRAME);
    assert!(r.rt.find("attachment-strip").is_some(), "the chip is drawn");
    r.send_and_wait();
    let sent = &r.posted()[0];
    assert_eq!(sent["attachments"].as_array().map(Vec::len), Some(1));
    assert_eq!(sent["attachments"][0]["mime"], "image/png");
    assert_eq!(sent["attachments"][0]["filename"], "login-error.png");
    assert!(sent["attachments"][0]["data"].as_str().is_some_and(|d| !d.is_empty()));
    assert!(r.rt.state.held_images().is_empty(), "cleared after the session was made");
}

#[test]
fn the_first_bubble_of_the_new_session_carries_its_pictures_and_session_references_a8_12_a9_7() {
    let mut r = listing(true);
    r.type_and_wait("Pick up where @bill", "session:ses-billing");
    r.press(Key::Tab, Mods::default());
    r.text(" what is wrong here?");
    let path = r.picture();
    r.rt.update(move |s, tx| s.add_files(&[path], tx));
    r.wait("the picture to be read", |s| s.held_images().len() == 1);
    r.send_and_wait();
    let sent = r.rt.state.prompts.of("ses-new").first().cloned().expect("the first message is shown as sent");
    assert_eq!(sent.images.len(), 1);
    assert_eq!(sent.images[0].filename.as_deref(), Some("login-error.png"));
    assert!(sent.images[0].data_url().starts_with("data:image/png;base64,"));
    assert_eq!(sent.session_references, vec![("@billing-retries-spike".to_string(), "ses-billing".to_string())]);
    // The bubble is drawn from the message list's own copy of it.
    let bubble = crate::conv::message_list::optimistic_message(&sent);
    assert_eq!(bubble.images.len(), 1);
    assert_eq!(bubble.images[0].filename, "login-error.png");
}

#[test]
fn keyboard_says_so_when_the_harness_cant_take_images() {
    let mut r = listing(false);
    let path = r.picture();
    r.rt.update(move |s, tx| s.add_files(&[path], tx));
    r.rt.frame(FRAME);
    assert!(r.rt.state.held_images().is_empty());
    assert_eq!(r.rt.state.new_session_error(), Some("This session's harness can't take images."));
    assert!(r.rt.find("new-session-attach").is_none(), "no paperclip without image support");
}

#[test]
fn a_file_that_is_not_a_picture_is_refused_in_the_order_add_image_checks() {
    let mut r = listing(true);
    let dir = std::env::temp_dir().join(format!("fleet-native-pictures-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a folder");
    let notes = dir.join("notes.txt");
    std::fs::write(&notes, "hello").expect("a file");
    r.rt.update(move |s, tx| s.add_files(&[notes], tx));
    r.rt.frame(FRAME);
    assert_eq!(r.rt.state.new_session_error(), Some("Unsupported image type: text/plain"));
    assert!(r.rt.find("new-session-attach").is_some(), "the paperclip is there");
}

// ─── GitHub preset ──────────────────────────────────────────────────────────────

#[test]
fn github_preset_attaches_the_issue_picks_its_repository_and_a_new_worktree() {
    let mut rt = app();
    click(&mut rt, "new-session");
    rt.update(|s, _| s.new_draft.set_initial_source(Some(issue())));
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(rt.find("new-session-github-attachment").is_some());
    let s = &rt.state;
    let d = s.new_draft.draft().expect("a draft");
    assert_eq!(s.folder_label(d.folder.as_ref()), "rocket");
    assert_eq!(d.workspace, crate::fleet::new_session::request::Workspace::New);
    assert!(s.plan_parts().iter().any(|p| p.text.contains("fix/issue-42")), "the plan names the branch");
    assert!(s.new_draft.initial_source().is_none(), "the page took it");
}

#[test]
fn github_preset_can_start_with_only_the_issue() {
    let mut r = rig();
    r.rt.update(|s, _| s.new_draft.set_initial_source(Some(issue())));
    r.rt.frame(FRAME);
    r.rt.frame(FRAME);
    click(&mut r.rt, "create-session-submit");
    r.wait("the session to open", |s| !s.on_new_session_page() || s.new_box.create_error.is_some());
    let sent = &r.posted()[0];
    assert_eq!(sent["title"], "Login redirect loops");
    assert_eq!(sent["branch"], "fix/issue-42");
    assert_eq!(sent["source"]["key"]["providerId"], "builtin.github");
    assert_eq!(sent["source"]["input"]["number"], 42);
    assert!(sent["initialPrompt"].is_null());
    assert!(r.rt.state.new_draft.initial_source().is_none());
}

#[test]
fn github_preset_offers_no_folder_less_choices_while_the_issue_is_attached() {
    let mut r = rig();
    r.rt.update(|s, _| s.new_draft.set_initial_source(Some(issue())));
    r.rt.frame(FRAME);
    r.rt.frame(FRAME);
    let titles: Vec<String> = r.rt.state.folder_rows().into_iter().map(|o| o.title).collect();
    assert!(titles.iter().any(|t| t.contains("rocket")), "{titles:?}");
    assert!(!titles.iter().any(|t| t.contains("No folder")), "{titles:?}");
    assert!(!titles.iter().any(|t| t.contains("Open or create a folder")), "{titles:?}");
    assert!(!r.rt.state.folder_allow_create());
}

#[test]
fn github_preset_can_be_removed() {
    let mut r = rig();
    r.rt.update(|s, _| s.new_draft.set_initial_source(Some(issue())));
    r.rt.frame(FRAME);
    r.rt.frame(FRAME);
    assert_eq!(r.rt.state.new_draft.draft().and_then(|d| d.github_preset.as_ref()).map(|p| p.number), Some(42));
    click(&mut r.rt, "new-session-github-remove");
    r.rt.frame(FRAME);
    assert!(r.rt.find("new-session-github-attachment").is_none());
    assert!(r.rt.state.new_draft.initial_source().is_none());
    assert!(!r.rt.state.can_send_new_session(), "Send is off with no message");
}

// ─── the place a host fills ─────────────────────────────────────────────────────
// What GitHub's host does with it is tested in `plugins::github::tests_picker`; here is only what core keeps.

#[test]
fn the_hash_stays_text_while_the_plugin_that_fills_the_place_is_off() {
    let mut r = rig();
    r.rt.update(|s, _| s.pickers.prefs.adopt(&json!({ "plugins.disabled": "[\"github\"]" })));
    r.text("Fix #31");
    assert!(r.rt.state.new_hash_trigger().is_none());
    r.rt.update(|s, _| s.pickers.prefs.adopt(&json!({})));
    r.rt.frame(FRAME);
    assert!(r.rt.state.new_hash_trigger().is_some(), "on again, the same # opens the place");
}

#[test]
fn esc_shuts_the_hash_until_the_text_changes() {
    let mut r = rig();
    r.text("Fix #31");
    assert!(r.rt.state.new_hash_trigger().is_some());
    r.rt.update(|s, _| s.new_hash_dismiss());
    assert!(r.rt.state.new_hash_trigger().is_none());
    r.text("2");
    assert!(r.rt.state.new_hash_trigger().is_some(), "typing opens it again");
}

// ─── @ references ───────────────────────────────────────────────────────────────

#[test]
fn at_lists_sessions_then_the_files_in_the_folder_chips_folder() {
    let mut r = listing(false);
    r.type_and_wait("Pick up where @bil", "session:ses-billing");
    r.wait("the files", |s| s.new_popup().is_some_and(|p| p.items.iter().any(|i| i.value.starts_with("@docs/billing"))));
    let popup = r.rt.state.new_popup().expect("the list").clone();
    let groups: Vec<_> = popup.items.iter().map(|i| i.group).collect();
    let first_file = groups.iter().position(|g| *g == crate::composer::triggers::Group::File);
    let last_session = groups.iter().rposition(|g| *g == crate::composer::triggers::Group::Session);
    assert!(last_session < first_file, "sessions first: {groups:?}");
    assert!(popup.items.iter().any(|i| i.label == "Billing retries spike"));
    assert_eq!(popup.file_hint.as_deref(), Some("in rocket"));
    let last = r.fake.lock().unwrap().finds.last().cloned();
    assert_eq!(last, Some((ROCKET.to_string(), "bil".to_string())));
}

#[test]
fn at_tints_the_picks_and_sends_the_sessions_with_the_request() {
    let mut r = listing(false);
    r.pick_both();
    assert_eq!(r.box_text(), BILLING);
    assert_eq!(r.tinted(), ["@billing-retries-spike", "@src/billing/retry-policy.ts", "@docs/billing/"]);
    r.send_and_wait();
    let sent = &r.posted()[0];
    assert_eq!(sent["initialPrompt"], BILLING.trim());
    assert_eq!(sent["sessionReferences"], json!([{ "token": "@billing-retries-spike", "sessionId": "ses-billing" }]));
    assert!(r.rt.state.auto.picks_of("new-session-draft-1").is_none_or(|p| p.sessions().is_empty()), "forgotten after the send");
}

#[test]
fn at_turns_picks_the_new_folder_doesnt_have_back_into_text_and_says_so() {
    let mut r = listing(false);
    r.pick_both();
    let draft = r.box_text();
    r.rt.update(|s, _| s.set_folder(Folder::Repository(COMET.into()), true));
    r.wait("the notice", |s| s.new_folder_notice().is_some());
    let (paths, folder) = r.rt.state.new_folder_notice().expect("a notice");
    assert_eq!((paths, folder.as_str()), (vec!["src/billing/retry-policy.ts".to_string()], "comet"));
    let mut asked: Vec<String> = r.fake.lock().unwrap().finds.iter().filter(|(d, _)| d == COMET).map(|(_, q)| q.clone()).collect();
    asked.sort();
    assert_eq!(asked, ["docs/billing", "src/billing/retry-policy.ts"]);
    assert_eq!(r.box_text(), draft, "nothing left the draft");
    assert_eq!(r.tinted(), ["@billing-retries-spike", "@docs/billing/"]);
    r.rt.frame(FRAME);
    assert!(r.rt.find("new-session-folder-notice").is_some());
}

#[test]
fn at_backspace_takes_a_whole_pick_out() {
    let mut r = listing(false);
    r.type_and_wait("Pick up where @bill", "session:ses-billing");
    r.press(Key::Tab, Mods::default());
    let len = r.box_text().len();
    r.rt.update(move |s, _| s.new_box.edit.set_caret(len - 1));
    r.press(Key::Backspace, Mods::default());
    // The key selects the whole pick and the field deletes the selection: it goes in one piece.
    assert_eq!(r.box_text(), "Pick up where  ");
}

#[test]
fn route_stays_on_the_page_while_the_list_is_open() {
    let mut r = listing(false);
    r.type_and_wait("see @bil", "session:ses-billing");
    r.press(Key::Escape, Mods::default());
    assert!(r.rt.state.new_popup().is_none(), "Esc closes the list");
    assert!(matches!(r.rt.state.route, Route::NewSession { .. }));
    assert_eq!(r.box_text(), "see @bil");
}
