//! The autocomplete popup, live (docs/parity/05-composer.md, piece K): "keeps textarea focus when clicking an autocomplete
//! item and applies the selection", "offers no @ references or / commands in a shell command", "lists /btw in the / popup
//! only for a harness that can fork" (of `Composer.test.ts`), the keys as native tests, and the requests read from a fake
//! server ("routes slash commands to the command endpoint", "puts a refused slash command back and says why").
use super::fixture;
use super::reconnect::app;
use crate::State;
use crate::composer::status::HarnessInfo;
use crate::composer::triggers::Group;
use rune::{Key, KeyEvent, Mods, Runtime};

const FRAME: f64 = 1.0 / 60.0;

fn click(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element keyed {key:?}"));
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(FRAME);
}

pub(super) fn press(rt: &mut Runtime<State>, key: Key) {
    rt.key(KeyEvent { key, mods: Mods::default() });
    rt.frame(FRAME);
    rt.frame(FRAME);
}

pub(super) fn typed(rt: &mut Runtime<State>, text: &str) {
    click(rt, "composer");
    press(rt, Key::Text(text.into()));
}

/// The invented demo's idle session, with a harness that has `shell` and `side` (fork) capabilities or not.
fn mock_app(shell: bool, side: bool) -> Runtime<State> {
    let mut rt = app();
    rt.update(move |s, _| {
        s.set_open((0, 2));
        s.projects[0].sessions[2].row.harness_type = Some("opencode".into());
        s.pickers.harnesses = vec![HarnessInfo {
            kind: "opencode".into(),
            supports_shell_commands: shell,
            supports_side_conversations: side,
            display_name: Some("OpenCode".into()),
            ..Default::default()
        }];
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

fn labels(rt: &Runtime<State>) -> Vec<String> {
    rt.state.auto.popup.as_ref().map(|p| p.items.iter().map(|i| i.label.clone()).collect()).unwrap_or_default()
}

fn selected(rt: &Runtime<State>) -> Option<String> {
    rt.state.auto.popup.as_ref().and_then(|p| p.selected.clone())
}

// "keeps textarea focus when clicking an autocomplete item and applies the selection"
#[test]
fn keeps_textarea_focus_when_clicking_an_autocomplete_item_and_applies_the_selection() {
    let mut rt = mock_app(false, false);
    typed(&mut rt, "/");
    assert!(rt.find("autocomplete-popup").is_some(), "the popup didn't open on a `/`");
    click(&mut rt, "autocomplete-option-0");
    assert_eq!(rt.state.draft.text, "/help ");
    // The caret stayed in the text area: what is typed next lands in it with no click.
    press(&mut rt, Key::Text("now".into()));
    assert_eq!(rt.state.draft.text, "/help now");
    assert!(rt.state.auto.popup.is_none(), "a typed space closes the `/` list");
}

// "offers no @ references or / commands in a shell command"
#[test]
fn offers_no_at_references_or_slash_commands_in_a_shell_command() {
    let mut rt = mock_app(true, false);
    typed(&mut rt, "!cat @src");
    assert!(rt.state.auto.popup.is_none());
    assert!(rt.find("autocomplete-popup").is_none());
    // The same text on a harness that can't run commands is an ordinary message: the list opens.
    let mut plain = mock_app(false, false);
    typed(&mut plain, "!cat @src");
    assert!(plain.state.auto.popup.is_some());
}

// "lists /btw in the / popup only for a harness that can fork"
#[test]
fn lists_btw_in_the_slash_popup_only_for_a_harness_that_can_fork() {
    let mut rt = mock_app(false, true);
    typed(&mut rt, "/");
    assert_eq!(labels(&rt)[0], "/btw");
    let first = rt.state.auto.popup.as_ref().unwrap().items[0].description.clone();
    assert_eq!(first.as_deref(), Some("Ask a side question without disturbing the session"));
    let mut other = mock_app(false, false);
    typed(&mut other, "/");
    assert!(!labels(&other).contains(&"/btw".to_string()));
}

#[test]
fn the_arrows_wrap_around_and_enter_picks_without_sending() {
    let mut rt = mock_app(false, false);
    let before = rt.state.session().messages.len();
    typed(&mut rt, "/rev");
    assert_eq!(labels(&rt), ["/review"]);
    press(&mut rt, Key::Down);
    assert_eq!(selected(&rt).as_deref(), Some("/review "), "one row wraps onto itself");
    typed_more(&mut rt, "");
    press(&mut rt, Key::Enter);
    assert_eq!(rt.state.draft.text, "/review ");
    assert_eq!(rt.state.session().messages.len(), before, "Enter picked: nothing was sent");
    // Now the popup is closed, so Enter sends the command.
    press(&mut rt, Key::Enter);
    assert_eq!(rt.state.session().messages.len(), before + 1);
}

fn typed_more(rt: &mut Runtime<State>, text: &str) {
    if !text.is_empty() {
        press(rt, Key::Text(text.into()));
    }
}

#[test]
fn up_and_down_move_through_the_groups_as_one_list() {
    let mut rt = mock_app(false, false);
    typed(&mut rt, "@");
    let groups: Vec<Group> = rt.state.auto.popup.as_ref().unwrap().items.iter().map(|i| i.group).collect();
    // Sessions first (the three a bare `@` offers), then files and folders, then agents.
    assert_eq!(groups.iter().filter(|g| **g == Group::Session).count(), 3);
    assert!(groups.windows(2).all(|w| rank(w[0]) <= rank(w[1])), "{groups:?}");
    let first = selected(&rt);
    press(&mut rt, Key::Up);
    assert_ne!(selected(&rt), first, "Up from the first row wraps to the last");
    assert_eq!(selected(&rt), rt.state.auto.popup.as_ref().unwrap().items.last().map(|i| i.value.clone()));
    press(&mut rt, Key::Down);
    assert_eq!(selected(&rt), first);
}

fn rank(g: Group) -> u8 {
    match g {
        Group::Command => 0,
        Group::Skill => 1,
        Group::Session => 2,
        Group::File => 3,
        Group::Agent => 4,
    }
}

#[test]
fn tab_on_a_folder_opens_it_and_enter_references_a_file() {
    let mut rt = mock_app(false, false);
    typed(&mut rt, "see @do");
    let folder = rt.state.auto.popup.as_ref().unwrap().items.iter().position(|i| i.value == "@docs/ ").expect("docs/ is listed");
    for _ in 0..folder {
        press(&mut rt, Key::Down);
    }
    assert_eq!(selected(&rt).as_deref(), Some("@docs/ "));
    press(&mut rt, Key::Tab);
    assert_eq!(rt.state.draft.text, "see @docs/", "a folder opened with Tab has no space after it and isn't picked");
    assert!(labels(&rt).contains(&"guide.md".to_string()), "{:?}", labels(&rt));
    let guide = rt.state.auto.popup.as_ref().unwrap().items.iter().position(|i| i.label == "guide.md").unwrap();
    for _ in 0..guide {
        press(&mut rt, Key::Down);
    }
    press(&mut rt, Key::Enter);
    assert_eq!(rt.state.draft.text, "see @docs/guide.md ");
}

#[test]
fn escape_closes_the_popup_and_does_not_interrupt() {
    let mut rt = mock_app(false, false);
    // A working session: Esc would interrupt it.
    rt.update(|s, _| s.projects[0].sessions[2].status = crate::mock::Status::Working);
    typed(&mut rt, "/");
    assert!(rt.state.auto.popup.is_some());
    press(&mut rt, Key::Escape);
    assert!(rt.state.auto.popup.is_none());
    assert_eq!(rt.state.session().status, crate::mock::Status::Working, "Esc was taken by the popup");
    // It stays closed until the text differs; then Esc interrupts as before.
    press(&mut rt, Key::Escape);
    assert_eq!(rt.state.session().status, crate::mock::Status::Idle);
}

#[test]
fn a_dollar_that_names_no_skill_is_text_and_enter_sends_it() {
    let mut rt = mock_app(false, false);
    let before = rt.state.session().messages.len();
    typed(&mut rt, "it costs $5");
    let popup = rt.state.auto.popup.as_ref().expect("the `$` list says there is no such skill");
    assert!(popup.items.is_empty());
    assert_eq!(popup.empty_text.as_deref(), Some("No skill named “5”. It stays as text."));
    press(&mut rt, Key::Enter);
    assert_eq!(rt.state.session().messages.len(), before + 1);
}

#[test]
fn a_held_enter_picks_the_first_item_once_the_commands_arrive() {
    let mut rt = mock_app(false, false);
    let owner = rt.state.draft_owner().unwrap();
    rt.update({
        let owner = owner.clone();
        move |s, _| {
            s.finder.commands.insert(owner, crate::fleet::finder::Loaded { loading: true, ..Default::default() });
        }
    });
    typed(&mut rt, "/re");
    assert!(rt.state.auto.popup.as_ref().unwrap().loading);
    press(&mut rt, Key::Enter);
    assert_eq!(rt.state.draft.text, "/re", "nothing to pick yet: the key waits");
    rt.update(move |s, _| {
        s.finder.on_commands(&owner, Ok((200, serde_json::json!({"commands": [{"name": "review", "kind": "command"}]}))));
        s.finder.commands.get_mut(&s.draft_owner().unwrap()).unwrap().loading = false;
        s.auto.changed();
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert_eq!(rt.state.draft.text, "/review ");
}

#[test]
fn a_disabled_skill_does_nothing_on_enter_and_says_why() {
    let mut rt = mock_app(false, false);
    rt.update(|s, _| s.projects[0].sessions[2].row.harness_type = Some("claude-code".into()));
    typed(&mut rt, "$tidy-docs and $deploy-check");
    let popup = rt.state.auto.popup.as_ref().unwrap();
    assert_eq!(popup.items.len(), 1);
    assert!(popup.items[0].disabled_reason.is_some());
    press(&mut rt, Key::Enter);
    assert_eq!(rt.state.draft.text, "$tidy-docs and $deploy-check");
}

#[test]
fn escape_in_the_text_area_interrupts_when_no_popup_takes_it() {
    let mut rt = mock_app(false, false);
    rt.update(|s, _| s.projects[0].sessions[2].status = crate::mock::Status::Working);
    typed(&mut rt, "hello");
    press(&mut rt, Key::Escape);
    assert_eq!(rt.state.session().status, crate::mock::Status::Idle, "Esc reached the interrupt from the focused text area");
}

// ─── the requests, read from a fake server ──────────────────────────────────

use super::super::link::{Job, Link, Post};
use super::super::sessions;
use serde_json::{Value, json};
use std::io::Write;
use std::net::TcpStream;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Default)]
pub(super) struct Seen {
    /// Method, path and body of every call but the list, the hub and the lists the popup asks for.
    pub(super) calls: Vec<(String, String, Value)>,
    /// Paths of the `GET`s the popup made.
    pub(super) gets: Vec<String>,
    /// What `POST …/command` answers instead of 202.
    pub(super) refuse: Option<(u16, Value)>,
    pub(super) commands: Value,
    pub(super) files: Option<Value>,
}

fn serve(mut stream: TcpStream, seen: Arc<Mutex<Seen>>) {
    let heard = crate::test_http::read(&mut stream);
    if heard.method.is_empty() {
        return;
    }
    let (head, raw) = (heard.head, heard.body);
    let body: Value = serde_json::from_slice(&raw).unwrap_or(Value::Null);
    let request = head.lines().next().unwrap_or("").to_string();
    let mut parts = request.split(' ');
    let (method, path) = (parts.next().unwrap_or("").to_string(), parts.next().unwrap_or("").to_string());
    let reply = |stream: &mut TcpStream, status: &str, body: &str| {
        let _ = stream.write_all(
            format!(
                "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            )
            .as_bytes(),
        );
    };
    let mut seen = seen.lock().unwrap();
    if path.starts_with("/hubs/session-events/negotiate") {
        reply(&mut stream, "200 OK", "{\"negotiateVersion\":1,\"connectionToken\":\"tok\"}");
    } else if path.starts_with("/api/sessions?") && !path.contains("retentionStatus=all") {
        reply(&mut stream, "200 OK", &fixture("api-sessions.json"));
    } else if method == "GET" && path.ends_with("/commands") {
        seen.gets.push(path);
        reply(&mut stream, "200 OK", &seen.commands.to_string());
    } else if method == "GET" && path.contains("/find/files") {
        seen.gets.push(path);
        match seen.files.clone() {
            Some(files) => reply(&mut stream, "200 OK", &files.to_string()),
            None => reply(&mut stream, "500 Internal Server Error", "{\"error\":\"The search broke\"}"),
        }
    } else if method == "GET" && path.starts_with("/api/sessions?retentionStatus=all") {
        seen.gets.push(path);
        reply(&mut stream, "200 OK", "[]");
    } else if method == "POST" && path.ends_with("/command") {
        let refused = seen.refuse.clone();
        seen.calls.push((method, path, body));
        match refused {
            Some((code, answer)) => reply(&mut stream, &format!("{code} Refused"), &answer.to_string()),
            None => reply(&mut stream, "202 Accepted", "{}"),
        }
    } else if method == "GET" {
        // The agents, models, queue and the rest the open session asks for: empty answers.
        reply(
            &mut stream,
            "200 OK",
            if path.ends_with("/agents") { "{\"agents\":[{\"name\":\"build\",\"color\":\"#3B82F6\"}]}" } else { "[]" },
        );
    } else {
        seen.calls.push((method, path, body));
        reply(&mut stream, "200 OK", "{}");
    }
}

pub(super) struct Live {
    pub(super) rt: Runtime<State>,
    pub(super) id: String,
    jobs: std::sync::mpsc::Receiver<Job>,
    seen: Arc<Mutex<Seen>>,
}

impl Live {
    pub(super) fn start(seen: Seen) -> Live {
        let listener = crate::test_http::bind().unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(seen));
        let shared = seen.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let seen = shared.clone();
                std::thread::spawn(move || serve(stream, seen));
            }
        });
        let (tx, jobs) = channel::<Job>();
        let tx = Mutex::new(tx);
        let post = Post::new(move |job| {
            let _ = tx.lock().unwrap().send(job);
        });
        let mut rt = app();
        let link = Link::start(base, post);
        rt.update(move |s, tx| {
            s.link = Some(link);
            s.pickers.harnesses =
                vec![HarnessInfo { kind: "opencode".into(), display_name: Some("OpenCode".into()), ..Default::default() }];
            s.on_list(sessions::parse_list(&serde_json::from_str(&fixture("api-sessions.json")).unwrap(), 0), tx);
            s.projects[0].sessions[0].row.harness_type = Some("opencode".into());
            s.projects[0].sessions[0].status = crate::mock::Status::Idle;
        });
        let id = rt.state.open_id().unwrap();
        let mut live = Live { rt, id, jobs, seen };
        live.rt.frame(FRAME);
        live
    }

    pub(super) fn wait(&mut self, until: &dyn Fn(&Live) -> bool, what: &str) {
        crate::test_wait::until(
            self,
            what,
            |l| {
                if until(l) {
                    return true;
                }
                while let Ok(job) = l.jobs.try_recv() {
                    l.rt.update(job);
                }
                l.rt.frame(FRAME);
                until(l)
            },
            |l| format!("{}; {} calls reached the fake server", crate::test_wait::app_state(&l.rt.state), l.calls().len()),
        );
    }

    pub(super) fn calls(&self) -> Vec<(String, String, Value)> {
        self.seen.lock().unwrap().calls.clone()
    }

    fn gets(&self) -> Vec<String> {
        self.seen.lock().unwrap().gets.clone()
    }

    pub(super) fn idle(&mut self) {
        self.rt.update(|s, _| s.projects[0].sessions[0].status = crate::mock::Status::Idle);
    }
}

pub(super) fn commands() -> Value {
    json!({"commands": [{"name": "start-work", "description": "Begin a task", "kind": "command"}, {"name": "tidy-docs", "kind": "skill"}]})
}

#[test]
fn the_session_opens_with_its_commands_loaded_from_the_server() {
    let mut live = Live::start(Seen { commands: commands(), ..Default::default() });
    let id = live.id.clone();
    live.wait(&|l| l.rt.state.finder.commands.contains_key(&id), "the commands to load");
    live.idle();
    typed(&mut live.rt, "/");
    assert_eq!(labels(&live.rt), ["/start-work", "/tidy-docs"]);
    assert!(live.gets().iter().any(|p| p == &format!("/api/sessions/{id}/commands")));
}

#[test]
fn the_popup_says_loading_until_the_commands_arrive() {
    // A server that never got asked: nothing has arrived, so the list is loading.
    let mut live = Live::start(Seen { commands: commands(), ..Default::default() });
    let id = live.id.clone();
    live.rt.update(move |s, _| {
        s.finder.commands.remove(&id);
    });
    live.idle();
    typed(&mut live.rt, "/");
    let popup = live.rt.state.auto.popup.as_ref().unwrap();
    assert!(popup.loading && popup.items.is_empty());
    live.rt.frame(FRAME);
    assert!(live.rt.find("autocomplete-state").is_some());
}

// "routes slash commands to the command endpoint"
#[test]
fn routes_slash_commands_to_the_command_endpoint() {
    let mut live = Live::start(Seen { commands: commands(), ..Default::default() });
    live.idle();
    typed(&mut live.rt, "/start-work now");
    press(&mut live.rt, Key::Enter);
    live.wait(&|l| !l.calls().is_empty(), "the command request");
    let calls = live.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!((calls[0].0.as_str(), calls[0].1.as_str()), ("POST", &*format!("/api/sessions/{}/command", live.id)));
    assert_eq!((calls[0].2["command"].as_str(), calls[0].2["arguments"].as_str()), (Some("start-work"), Some("now")));
    assert!(!calls.iter().any(|c| c.1.ends_with("/prompt")), "a command isn't sent as a message");
    assert_eq!(live.rt.state.draft.text, "");
    assert!(live.rt.state.optimistic.is_some(), "the session shows busy at once");
}

// "puts a refused slash command back and says why"
#[test]
fn puts_a_refused_slash_command_back_and_says_why() {
    let mut live =
        Live::start(Seen { commands: commands(), refuse: Some((400, json!({"error": "Unknown command"}))), ..Default::default() });
    live.idle();
    typed(&mut live.rt, "/start-work now");
    press(&mut live.rt, Key::Enter);
    live.wait(&|l| l.rt.state.composer_error.is_some(), "the refusal");
    assert_eq!(live.rt.state.draft.text, "/start-work now");
    assert_eq!(live.rt.state.composer_error.as_deref(), Some("Unknown command"));
    assert!(live.rt.state.optimistic.is_none(), "the busy it was shown as is taken back");
    live.rt.frame(FRAME);
    assert!(live.rt.find("send-prompt-error").is_some());
}

#[test]
fn a_refused_command_goes_in_front_of_new_text() {
    let mut live = Live::start(Seen { commands: commands(), refuse: Some((500, json!({}))), ..Default::default() });
    live.idle();
    let id = live.id.clone();
    live.rt.update(move |s, _| s.on_command_answer(&id, "/old".into(), Ok((500, json!({})))));
    assert_eq!(live.rt.state.draft.text, "/old", "an empty draft gets the text back");
    assert_eq!(live.rt.state.composer_error.as_deref(), Some("HTTP 500"));
    let id = live.id.clone();
    live.rt.update(move |s, _| s.on_command_answer(&id, "/older".into(), Ok((500, json!({})))));
    assert_eq!(live.rt.state.draft.text, "/older\n\n/old", "typed since: the refused text goes in front");
}

#[test]
fn an_at_asks_the_server_for_files_after_80_ms_and_lists_the_answer() {
    let files = json!({"files": ["src/", "src/store.rs", "README.md"]});
    let mut live = Live::start(Seen { commands: commands(), files: Some(files), ..Default::default() });
    live.idle();
    typed(&mut live.rt, "look at @st");
    live.wait(&|l| l.gets().iter().any(|p| p.contains("/find/files")), "the file search");
    let path = live.gets().into_iter().find(|p| p.contains("/find/files")).unwrap();
    assert_eq!(path, format!("/api/sessions/{}/find/files?q=st", live.id));
    live.wait(
        &|l| l.rt.state.auto.popup.as_ref().is_some_and(|p| !p.loading && p.items.iter().any(|i| i.value == "@src/store.rs ")),
        "the answer",
    );
    // The agents the harness lists (shared with the picker) are on the `@` list too.
    assert!(labels(&live.rt).contains(&"@build".to_string()) || live.rt.state.finder.agents.contains_key(&live.id));
}

#[test]
fn a_failed_file_search_says_why_in_the_popup() {
    let mut live = Live::start(Seen { commands: commands(), files: None, ..Default::default() });
    live.idle();
    typed(&mut live.rt, "@zzz");
    live.wait(&|l| l.rt.state.auto.popup.as_ref().is_some_and(|p| p.error.is_some()), "the failure");
    assert_eq!(live.rt.state.auto.popup.as_ref().unwrap().error.as_deref(), Some("The search broke"));
}

#[test]
fn the_sessions_list_is_fetched_once_when_the_at_list_opens() {
    let mut live = Live::start(Seen { commands: commands(), files: Some(json!({"files": []})), ..Default::default() });
    live.idle();
    typed(&mut live.rt, "@");
    live.wait(&|l| l.gets().iter().any(|p| p.starts_with("/api/sessions?retentionStatus=all&limit=200")), "the sessions fetch");
    press(&mut live.rt, Key::Text("a".into()));
    press(&mut live.rt, Key::Text("b".into()));
    std::thread::sleep(Duration::from_millis(100));
    live.rt.frame(FRAME);
    let fetches = live.gets().iter().filter(|p| p.starts_with("/api/sessions?retentionStatus=all")).count();
    assert_eq!(fetches, 1, "typing in an open list doesn't fetch again");
}
