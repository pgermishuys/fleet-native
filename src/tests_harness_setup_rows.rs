//! Piece O: the harnesses step's rows (`HarnessSetupRows.test.ts`, rows C13.1-C13.73), against a fake Fleet that answers the harness
//! list, its client config and the setup terminal, and records every request. The cases keep Fleet's names. The terminal's widget is
//! component 13's (piece I): the fake speaks the terminal's WebSocket too, so "the terminal shows the command" is the shell hearing
//! it typed.
use crate::fleet::harness_setup::Step;
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::new_session::harness_choice::Load;
use crate::fleet::terminal_socket::Status;
use crate::tests::{FRAME, app, click};
use crate::{State, test_wait};
use rune::Runtime;
use serde_json::{Value, json};
use std::io::Write;
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Fake {
    harnesses: Vec<Value>,
    terminals_on: bool,
    /// What `POST /api/setup/terminals` answers: (status, body).
    terminal_answer: Option<(u16, Value)>,
    log: Vec<(String, String, String)>,
    /// The paths of the setup terminal's sockets, one per connection.
    sockets: Vec<String>,
    /// What the shell was typed, over all connections.
    input: Vec<u8>,
    /// Lines for the shell's socket to act on: `cut` drops the connection, anything else is sent as a text frame.
    script: std::collections::VecDeque<String>,
    /// While set, a socket is dropped before the upgrade.
    refuse_sockets: bool,
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

/// The request path, looked at without taking the request: a socket's upgrade is read by `accept` itself.
fn peek_path(stream: &std::net::TcpStream) -> String {
    let mut buf = [0u8; 4096];
    loop {
        let n = stream.peek(&mut buf).unwrap_or(0);
        if n == 0 {
            return String::new();
        }
        let text = String::from_utf8_lossy(&buf[..n]).to_string();
        if let Some(end) = text.find("\r\n\r\n") {
            return text[..end].lines().next().unwrap_or("").split(' ').nth(1).unwrap_or("").to_string();
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

/// `TerminalSocket.cs`'s side, as far as the setup terminal needs: `ready` at once, input heard, the script acted on.
fn shell_socket(stream: std::net::TcpStream, path: String, fake: Shared) {
    {
        let mut f = fake.lock().unwrap();
        f.sockets.push(path);
        if f.refuse_sockets {
            return;
        }
    }
    let Ok(mut ws) = tungstenite::accept(stream) else { return };
    let _ = ws.send(tungstenite::Message::text(r#"{"type":"ready"}"#));
    let _ = ws.get_ref().set_read_timeout(Some(std::time::Duration::from_millis(5)));
    loop {
        let next = fake.lock().unwrap().script.pop_front();
        match next.as_deref() {
            Some("cut") => return,
            Some(line) => {
                let _ = ws.send(tungstenite::Message::text(line.to_string()));
            }
            None => {}
        }
        match ws.read() {
            Ok(tungstenite::Message::Binary(b)) => fake.lock().unwrap().input.extend_from_slice(&b),
            Ok(tungstenite::Message::Close(_)) | Err(tungstenite::Error::ConnectionClosed) => return,
            Err(tungstenite::Error::Io(e)) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
            Err(_) => return,
            Ok(_) => {}
        }
    }
}

fn answer(mut stream: std::net::TcpStream, fake: Shared) {
    let path = peek_path(&stream);
    if path.contains("/socket?") {
        return shell_socket(stream, path, fake);
    }
    let req = crate::test_http::read(&mut stream);
    if req.method.is_empty() {
        return;
    }
    let body = req.body;
    let (method, path) = (req.method, req.path);
    let mut f = fake.lock().unwrap();
    f.log.push((method.clone(), path.clone(), String::from_utf8_lossy(&body).to_string()));
    let (status, reply) = if method == "GET" && path.starts_with("/api/harnesses") {
        (200, Value::Array(f.harnesses.clone()))
    } else if method == "GET" && path == "/api/preferences" {
        (200, json!({}))
    } else if method == "GET" && path == "/api/config/client" {
        (200, json!({ "terminalEnabled": f.terminals_on }))
    } else if method == "POST" && path == "/api/setup/terminals" {
        f.terminal_answer.clone().unwrap_or((200, json!({ "id": "t_setup", "title": "zsh", "status": "running" })))
    } else {
        (200, json!({}))
    };
    let reply = reply.to_string();
    let out = format!(
        "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}",
        reply.len()
    );
    let _ = stream.write_all(out.as_bytes());
}

/// A harness as `GET /api/harnesses` sends it.
fn harness(kind: &str, name: &str, state: &str, enabled: bool, setup: Value) -> Value {
    json!({ "type": kind, "displayName": name, "available": state == "ready", "userEnabled": enabled, "state": state,
        "reason": format!("{name} isn't installed."), "setup": setup })
}

fn opencode(state: &str) -> Value {
    let mut h = harness("opencode", "OpenCode", state, true, json!({ "installCommand": "curl -fsSL https://opencode.ai/install | bash" }));
    h["presentation"] = json!({ "icon": "terminal", "pitch": "Open source. Includes free models, or sign in to your own provider." });
    h["isDefault"] = json!(true);
    h
}

fn claude_code(state: &str, enabled: bool) -> Value {
    harness(
        "claude-code",
        "Claude Code",
        state,
        enabled,
        json!({ "installCommand": "curl -fsSL https://claude.ai/install.sh | bash", "signInCommand": "/usr/local/bin/claude auth login" }),
    )
}

const SEPARATE: &str = "curl -fsSL https://opencode.ai/v2/install | HOME=/data/opencode2 bash -s -- --no-modify-path";
const MAIN: &str = "curl -fsSL https://opencode.ai/v2/install | bash";

fn opencode2_choices() -> Value {
    json!({ "installCommand": SEPARATE, "mode": "In its own folder", "notes": [],
        "installChoices": [
            { "id": "separate", "label": "In its own folder", "description": "OpenCode 1 can be installed next to it at any time.", "command": SEPARATE, "folders": [], "recommended": true },
            { "id": "default", "label": "As your main opencode", "description": "Installing OpenCode 1 later replaces OpenCode 2.", "command": MAIN, "folders": [], "recommended": false },
        ] })
}

fn opencode2(setup: Value) -> Value {
    harness("opencode2", "OpenCode 2", "not-installed", false, setup)
}

fn copilot(phase: Option<&str>) -> Value {
    let mut h = harness("copilot", "GitHub Copilot", if phase == Some("succeeded") { "ready" } else { "not-installed" }, true, Value::Null);
    let plan = "Fleet downloads weave-copilot 0.1.0 and the Copilot CLI 1.0.98 from GitHub.";
    h["update"] = match phase {
        Some(p) => {
            json!({ "updateAvailable": false, "canInstall": p == "running", "installPlan": plan, "job": { "phase": p, "message": "Downloading… 30%", "output": "", "workingSessions": 0 } })
        }
        None => json!({ "updateAvailable": false, "canInstall": true, "installPlan": plan }),
    };
    h
}

struct Rig {
    rt: Runtime<State>,
    queue: Receiver<Job>,
    fake: Shared,
}

/// A Fleet with these harnesses (terminals on), connected, the wizard open at its harnesses step.
fn rig(harnesses: Vec<Value>) -> Rig {
    let fake: Shared = Arc::new(Mutex::new(Fake { harnesses, terminals_on: true, ..Fake::default() }));
    let base = serve(fake.clone());
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let link = Link::start(
        base,
        Post::new(move |job| {
            let _ = jobs.lock().unwrap().send(job);
        }),
    );
    let mut rt = app();
    rt.update(move |s, _| {
        s.link = Some(link);
        s.feeds.harnesses.clear();
        s.pickers.prefs = Default::default();
        s.load_pickers();
    });
    let mut r = Rig { rt, queue, fake };
    r.wait("the harness list", |s| s.pickers.prefs.fetched && s.setup_harnesses().load == Load::Done);
    // Setup opened by itself when none is ready: the done flag and the step are the test's to set.
    r.rt.update(|s, _| s.open_harness_setup());
    r.settle();
    r
}

impl Rig {
    fn wait(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        test_wait::drive(&mut self.rt, &self.queue, what, done, |s| {
            format!("rows {:?}, wizard {:?}", s.harness_setup.rows, s.harness_setup.step)
        });
    }

    fn settle(&mut self) {
        for _ in 0..3 {
            self.rt.frame(FRAME);
        }
    }

    fn set_harnesses(&self, harnesses: Vec<Value>) {
        self.fake.lock().unwrap().harnesses = harnesses;
    }

    fn all_keys(&self) -> Vec<String> {
        self.rt.elements().iter().filter_map(|e| e.key.clone()).collect()
    }

    fn keys_with(&self, prefix: &str) -> Vec<String> {
        self.all_keys().into_iter().filter(|k| k.starts_with(prefix)).collect()
    }

    fn has(&self, key: &str) -> bool {
        self.rt.find(key).is_some()
    }

    /// What the element's text says, joined.
    fn words(&self, key: &str) -> String {
        let all = self.rt.elements();
        let it = all.iter().find(|e| e.key.as_deref() == Some(key)).unwrap_or_else(|| panic!("no element keyed {key:?}"));
        all.iter().filter(|e| e.path.starts_with(&it.path)).filter_map(|e| e.text.clone()).collect::<Vec<_>>().join(" ")
    }

    fn label(&self, key: &str) -> String {
        self.rt.elements().iter().find(|e| e.key.as_deref() == Some(key)).and_then(|e| e.label.clone()).unwrap_or_default()
    }

    fn log(&self) -> Vec<(String, String, String)> {
        self.fake.lock().unwrap().log.clone()
    }

    fn count(&self, method: &str, path: &str) -> usize {
        self.log().iter().filter(|(m, p, _)| m == method && p == path).count()
    }

    fn fresh_reads(&self) -> usize {
        self.count("GET", "/api/harnesses?fresh=true")
    }

    /// Waits until the terminal for the open row has an id (the shell was made).
    fn wait_terminal(&mut self) {
        self.wait("the setup terminal to start", |s| s.harness_setup.rows.active.as_ref().is_some_and(|a| a.terminal_id.is_some()));
        self.settle();
    }

    /// The shell has been typed exactly `command` (no Enter, nothing else).
    fn wait_typed(&mut self, command: &str) {
        let (fake, want) = (self.fake.clone(), command.as_bytes().to_vec());
        self.wait("the installer typed into the shell", move |_| fake.lock().unwrap().input == want);
        self.settle();
    }

    fn wait_request(&mut self, method: &str, path: &str) {
        let (fake, method, path) = (self.fake.clone(), method.to_string(), path.to_string());
        self.wait(&format!("{method} {path}"), move |_| fake.lock().unwrap().log.iter().any(|(m, p, _)| *m == method && *p == path));
    }

    /// Runs the clock `seconds` in one go, then lets what it asked for come back.
    fn after(&mut self, seconds: f64) {
        self.rt.frame(seconds);
        self.settle();
    }

    fn wait_fresh_reads(&mut self, at_least: usize) {
        let fake = self.fake.clone();
        self.wait("the harnesses to be read again", move |_| {
            fake.lock().unwrap().log.iter().filter(|(m, p, _)| m == "GET" && p == "/api/harnesses?fresh=true").count() >= at_least
        });
        self.settle();
    }
}

fn row_keys(r: &Rig) -> Vec<String> {
    r.keys_with("harness-setup-row-")
}

#[test]
fn lists_the_harnesses_fleet_can_install_the_default_first() {
    let pi = harness("pi", "Pi", "not-installed", false, json!({}));
    let r = rig(vec![claude_code("not-installed", false), pi, opencode("not-installed")]);
    assert_eq!(row_keys(&r), ["harness-setup-row-opencode", "harness-setup-row-claude-code"]);
    assert_eq!(r.label("harness-setup-install-opencode"), "Install OpenCode");
    // The pitch the harness sends; Claude Code here sends none, so its reason shows.
    assert!(r.words("harness-setup-row-opencode").contains("Open source. Includes free models"));
    assert!(r.words("harness-setup-row-claude-code").contains("Claude Code isn't installed."));
}

#[test]
fn offers_github_copilot_which_fleet_downloads_itself_with_an_install_button_under_its_row() {
    let r = rig(vec![opencode("not-installed"), copilot(None)]);
    assert_eq!(row_keys(&r), ["harness-setup-row-opencode", "harness-setup-row-copilot"]);
    assert!(r.words("harness-setup-row-copilot").contains("Fleet downloads weave-copilot 0.1.0"));
    assert!(r.has("harness-update-copilot"));
    // Its install is the strip's, not a command typed in a terminal.
    assert!(!r.has("harness-setup-install-copilot"));
}

#[test]
fn follows_github_copilots_install_while_it_runs_and_stops_once_its_done() {
    let mut r = rig(vec![opencode("not-installed"), copilot(Some("running"))]);
    let before = r.fresh_reads();
    r.after(2.1);
    r.after(2.1);
    r.wait_fresh_reads(before + 1);
    // It finished: the list says so on the next read, and the checking ends.
    r.set_harnesses(vec![opencode("not-installed"), copilot(Some("succeeded"))]);
    r.after(2.1);
    r.wait("the finished install", |s| s.setup_harnesses().all.iter().any(|h| h.setup.update_phase.as_deref() == Some("succeeded")));
    r.settle();
    let reads = r.fresh_reads();
    r.after(4.0);
    r.after(4.0);
    assert!(r.rt.state.harness_setup.rows.update_due.is_none(), "no more checks once nothing waits or runs");
    assert_eq!(r.fresh_reads(), reads);
}

#[test]
fn says_where_opencode_2_goes_and_what_to_know_before_installing_it() {
    let notes =
        ["OpenCode 1 is installed here, so OpenCode 2 goes into a folder of its own.", "OpenCode 2 keeps its own provider sign-ins here."];
    let r = rig(vec![opencode2(json!({ "installCommand": SEPARATE, "mode": "In its own folder", "notes": notes }))]);
    assert_eq!(r.label("harness-setup-install-opencode2"), "Install OpenCode 2");
    let said = r.words("harness-setup-notes-opencode2");
    assert!(said.contains(notes[0]) && said.contains(notes[1]), "{said}");
}

#[test]
fn lets_the_user_pick_where_opencode_2_goes_and_types_the_picked_places_installer() {
    let mut r = rig(vec![opencode2(opencode2_choices())]);
    let said = r.words("harness-setup-choices-opencode2");
    assert!(said.contains("In its own folder Recommended"), "{said}");
    assert!(said.contains("Installing OpenCode 1 later replaces OpenCode 2."));
    // The picked radio is the state's: Rune doesn't report a radio's checked flag to a test.
    let checked = |r: &Rig, id: &str| {
        let list = r.rt.state.setup_harnesses();
        let h = list.all.iter().find(|h| h.kind == "opencode2").unwrap();
        r.rt.state.harness_setup.rows.picked_choice(h).is_some_and(|c| c.id == id)
    };
    assert!(checked(&r, "separate") && !checked(&r, "default"));
    click(&mut r.rt, "harness-setup-choice-opencode2-default");
    assert!(checked(&r, "default") && !checked(&r, "separate"));
    click(&mut r.rt, "harness-setup-install-opencode2");
    r.wait_terminal();
    r.wait_typed(MAIN);
}

#[test]
fn offers_no_choice_once_opencode_2_is_installed() {
    let mut h = opencode2(opencode2_choices());
    h["state"] = json!("ready");
    h["available"] = json!(true);
    h["version"] = json!("2.0.9");
    let r = rig(vec![h]);
    assert!(!r.has("harness-setup-choices-opencode2"));
}

#[test]
fn links_to_the_download_when_theres_no_installer_to_type() {
    let note = "Save opencode.exe as opencode2.exe.";
    let r = rig(vec![opencode2(json!({ "installCommand": null, "downloadUrl": "https://opencode.ai/v2/docs", "notes": [note] }))]);
    assert!(r.has("harness-setup-download-opencode2"));
    assert_eq!(r.label("harness-setup-download-opencode2"), "Download OpenCode 2");
    assert!(!r.has("harness-setup-install-opencode2"));
    assert!(r.words("harness-setup-row-opencode2").contains(note));
}

#[test]
fn leaves_the_notes_out_once_the_harness_is_ready() {
    let mut h = opencode2(json!({ "installCommand": SEPARATE, "notes": ["Something to know."] }));
    h["state"] = json!("ready");
    h["available"] = json!(true);
    h["version"] = json!("2.0.9");
    h["executablePath"] = json!("/x/opencode2");
    let r = rig(vec![h]);
    assert!(!r.has("harness-setup-notes-opencode2"));
    // Ready shows where it runs from, in the row's second line.
    assert_eq!(r.words("harness-setup-sub-opencode2"), "2.0.9 · /x/opencode2");
}

#[test]
fn types_the_installer_into_a_setup_terminal_and_turns_on_a_harness_that_was_off() {
    let mut r = rig(vec![opencode("not-installed"), claude_code("not-installed", false)]);
    click(&mut r.rt, "harness-setup-install-claude-code");
    r.wait_terminal();
    let create = r.log().into_iter().find(|(m, p, _)| m == "POST" && p == "/api/setup/terminals").expect("the shell was asked for");
    assert_eq!(serde_json::from_str::<Value>(&create.2).unwrap(), json!({ "cols": 100, "rows": 14 }));
    r.wait_typed("curl -fsSL https://claude.ai/install.sh | bash");
    assert!(r.words("harness-setup-terminal").contains("Check the command, then press Enter to run it. Fleet won't run it for you."));
    assert!(r.words("harness-setup-terminal").contains("Install Claude Code · home folder"));
    r.wait_request("PUT", "/api/preferences/claude-code.enabled");
    let put = r.log().into_iter().find(|(m, p, _)| m == "PUT" && p == "/api/preferences/claude-code.enabled").unwrap();
    assert_eq!(serde_json::from_str::<Value>(&put.2).unwrap(), json!({ "value": "true" }));
    // Its button is off while its terminal is open and nothing was found.
    let opacity = r.rt.elements().iter().find(|e| e.key.as_deref() == Some("harness-setup-install-claude-code")).map(|e| e.opacity);
    assert!(opacity.is_some_and(|o| o < 0.9));
}

#[test]
fn checks_the_harnesses_every_few_seconds_while_the_terminal_is_open_and_says_when_it_finds_one() {
    let mut r = rig(vec![opencode("not-installed")]);
    click(&mut r.rt, "harness-setup-install-opencode");
    r.wait_terminal();
    let before = r.fresh_reads();
    r.after(3.1);
    r.wait_fresh_reads(before + 1);
    assert!(!r.has("harness-setup-found"));
    let mut found = opencode("ready");
    found["version"] = json!("1.18.30");
    found["executablePath"] = json!("/usr/local/bin/opencode");
    r.set_harnesses(vec![found]);
    r.after(3.1);
    r.wait("the found banner", |s| s.harness_setup.rows.found(&s.setup_harnesses()).is_some());
    r.settle();
    assert_eq!(r.words("harness-setup-found"), "Found OpenCode 1.18.30 at /usr/local/bin/opencode. You don't need to restart Fleet.");
}

#[test]
fn offers_the_sign_in_once_claude_code_is_installed() {
    let mut claude = claude_code("sign-in-required", true);
    claude["version"] = json!("2.1.276");
    let mut r = rig(vec![claude]);
    assert!(r.words("harness-setup-sub-claude-code").contains("Installed. Sign in once so sessions can use your account."));
    assert_eq!(r.label("harness-setup-sign-in-claude-code"), "Sign in to Claude Code");
    click(&mut r.rt, "harness-setup-sign-in-claude-code");
    r.wait_terminal();
    r.wait_typed("/usr/local/bin/claude auth login");
    assert!(r.words("harness-setup-terminal").contains("Sign in to Claude Code · home folder"));
}

#[test]
fn offers_to_update_a_harness_thats_too_old_with_its_installer() {
    let mut old = opencode("update-needed");
    old["version"] = json!("1.12.0");
    old["reason"] = json!("Fleet needs OpenCode 1.15.10 or newer. You have 1.12.0.");
    let mut r = rig(vec![old]);
    assert!(r.words("harness-setup-sub-opencode").contains("Fleet needs OpenCode 1.15.10 or newer. You have 1.12.0."));
    assert_eq!(r.label("harness-setup-install-opencode"), "Update OpenCode");
    click(&mut r.rt, "harness-setup-install-opencode");
    r.wait_terminal();
    r.wait_typed("curl -fsSL https://opencode.ai/install | bash");
}

#[test]
fn shows_the_command_to_run_elsewhere_when_terminals_are_off() {
    let mut r = rig(vec![opencode("not-installed")]);
    r.fake.lock().unwrap().terminals_on = false;
    click(&mut r.rt, "harness-setup-install-opencode");
    r.wait("the failure", |s| s.harness_setup.rows.failure.is_some());
    r.settle();
    assert_eq!(r.count("POST", "/api/setup/terminals"), 0);
    assert!(r.label("harness-setup-failure").contains("Terminals are turned off in this Fleet."));
    assert_eq!(r.words("harness-setup-failure-command"), "curl -fsSL https://opencode.ai/install | bash");
    assert!(!r.has("harness-setup-terminal"));
}

#[test]
fn ends_the_setup_terminal_when_it_goes_away() {
    let mut r = rig(vec![opencode("not-installed")]);
    click(&mut r.rt, "harness-setup-install-opencode");
    r.wait_terminal();
    r.rt.update(|s, _| s.leave_harness_setup());
    r.wait_request("DELETE", "/api/setup/terminals/t_setup");
    assert!(r.rt.state.harness_setup.rows.active.is_none());
}

// ---- beyond Fleet's names: the rows the TypeScript test leaves to the browser ---------------------------------------------------------

#[test]
fn a_refused_terminal_says_the_servers_words_and_the_copy_button_says_copied_for_a_moment() {
    let mut r = rig(vec![opencode("not-installed")]);
    r.fake.lock().unwrap().terminal_answer = Some((409, json!({ "error": "Another setup terminal is busy." })));
    click(&mut r.rt, "harness-setup-install-opencode");
    r.wait("the failure", |s| s.harness_setup.rows.failure.is_some());
    r.settle();
    assert!(r.label("harness-setup-failure").starts_with("Another setup terminal is busy. Run this in your own terminal instead"));
    assert!(r.rt.state.harness_setup.rows.active.is_none(), "a refused terminal leaves no terminal open");
    assert_eq!(r.words("harness-setup-copy"), "Copy");
    click(&mut r.rt, "harness-setup-copy");
    assert_eq!(r.words("harness-setup-copy"), "Copied");
    r.rt.frame(1.6);
    r.rt.frame(FRAME);
    assert_eq!(r.words("harness-setup-copy"), "Copy");
    // The next start clears the failure (C13.73).
    r.fake.lock().unwrap().terminal_answer = None;
    click(&mut r.rt, "harness-setup-install-opencode");
    assert!(r.rt.state.harness_setup.rows.failure.is_none());
    r.wait_terminal();
}

#[test]
fn starting_another_harness_ends_the_first_ones_terminal_and_a_late_answer_is_ended_too() {
    let mut r = rig(vec![opencode("not-installed"), claude_code("not-installed", true)]);
    click(&mut r.rt, "harness-setup-install-opencode");
    r.wait_terminal();
    click(&mut r.rt, "harness-setup-install-claude-code");
    r.wait_request("DELETE", "/api/setup/terminals/t_setup");
    r.wait_terminal();
    assert_eq!(r.rt.state.harness_setup.rows.active.as_ref().map(|a| a.kind.as_str()), Some("claude-code"));
    // Closing with the X ends the shell too, and the row has no terminal.
    click(&mut r.rt, "harness-setup-terminal-close");
    assert!(!r.has("harness-setup-terminal"));
    assert_eq!(r.rt.state.harness_setup.step, Step::Harnesses);
}

#[test]
fn a_finished_sign_in_stops_the_checking_and_says_so() {
    let mut r = rig(vec![claude_code("sign-in-required", true)]);
    click(&mut r.rt, "harness-setup-sign-in-claude-code");
    r.wait_terminal();
    r.set_harnesses(vec![claude_code("ready", true)]);
    r.after(3.1);
    r.wait("the sign-in to be found", |s| s.harness_setup.rows.found(&s.setup_harnesses()).is_some());
    r.settle();
    assert_eq!(r.words("harness-setup-found"), "Claude Code is signed in and ready.");
    let reads = r.fresh_reads();
    r.after(3.1);
    r.after(3.1);
    assert!(r.rt.state.harness_setup.rows.check_due.is_none(), "checking stops once the sign-in is found");
    assert_eq!(r.fresh_reads(), reads);
}

#[test]
fn the_state_is_the_servers_word_or_ready_when_available_else_not_working() {
    use crate::fleet::new_session::harness_choice::HarnessEntry;
    let h = |state: &str, available| HarnessEntry { state: state.into(), available, ..HarnessEntry::default() };
    assert_eq!(h("", true).setup_state(), "ready");
    assert_eq!(h("", false).setup_state(), "not-working");
    assert_eq!(h("update-needed", true).setup_state(), "update-needed");
}

// ---- piece I: the setup terminal is the Terminal component's view (13.80 to 13.82) ----------------------------------------------------

// 13.80: the view is hosted under `/api/setup/terminals`, session id `setup`: the socket is the setup shell's.
#[test]
fn the_setup_terminal_connects_under_the_setup_base_path_and_shows_the_shell() {
    let mut r = rig(vec![opencode("not-installed")]);
    click(&mut r.rt, "harness-setup-install-opencode");
    r.wait_terminal();
    r.wait_typed("curl -fsSL https://opencode.ai/install | bash");
    let sockets = r.fake.lock().unwrap().sockets.clone();
    assert!(sockets[0].starts_with("/api/setup/terminals/t_setup/socket?cols=100&rows=14"), "{sockets:?}");
    assert!(r.has("harness-setup-terminal-body") && r.has("terminal-view"), "the grid is drawn in the row's body");
    assert!(!r.has("terminal-drawer"), "no drawer and no tabs around it");
    // The setup row's terminal takes the keyboard when it opens (`:shown="true"`).
    assert!(r.rt.state.setup_terminal_shown());
}

// 13.81: `initialInput` is typed on the first `ready` only: a reconnect is ready again and the command may have run.
#[test]
fn the_command_is_typed_once_on_the_first_ready_and_not_again_after_a_reconnect() {
    let mut r = rig(vec![opencode("not-installed")]);
    click(&mut r.rt, "harness-setup-install-opencode");
    r.wait_terminal();
    let command = "curl -fsSL https://opencode.ai/install | bash";
    r.wait_typed(command);
    r.fake.lock().unwrap().script.push_back("cut".into());
    let fake = r.fake.clone();
    r.wait("the second connection", move |_| fake.lock().unwrap().sockets.len() == 2);
    r.wait("the second connection to be ready", |s| s.terminal.status() == Some(Status::Open));
    // What the user types now goes after the command and nothing is typed in between.
    r.rt.update(|s, _| s.terminal_write(b"x".to_vec()));
    let (fake, want) = (r.fake.clone(), format!("{command}x").into_bytes());
    r.wait("the probe key", move |_| fake.lock().unwrap().input == want);
}

// 13.82: the shell's `exit` is `ended`, which closes the setup terminal (DELETE, and checking stops).
#[test]
fn the_shell_ending_closes_the_setup_terminal() {
    let mut r = rig(vec![opencode("not-installed")]);
    click(&mut r.rt, "harness-setup-install-opencode");
    r.wait_terminal();
    r.fake.lock().unwrap().script.push_back(r#"{"type":"exit","exitCode":0}"#.into());
    r.wait_request("DELETE", "/api/setup/terminals/t_setup");
    r.wait("the terminal to go", |s| s.harness_setup.rows.active.is_none() && !s.setup_terminal_shown());
    r.settle();
    assert!(!r.has("harness-setup-terminal"));
    assert_eq!(r.rt.state.harness_setup.step, Step::Harnesses, "the wizard stays where it was");
}

// 13.82: a terminal that can't be reached says so, and its Close tab closes the setup terminal the same way.
#[test]
fn close_tab_on_an_unreachable_setup_terminal_closes_it() {
    let mut r = rig(vec![opencode("not-installed")]);
    r.fake.lock().unwrap().refuse_sockets = true;
    click(&mut r.rt, "harness-setup-install-opencode");
    r.wait("the terminal to be given up on", |s| s.terminal.status() == Some(Status::Failed));
    // The socket thread asks for a redraw as it gives up; here that is an update.
    r.rt.update(|_, _| {});
    r.settle();
    assert!(r.has("terminal-failed"));
    click(&mut r.rt, "terminal-close-tab");
    r.wait_request("DELETE", "/api/setup/terminals/t_setup");
    assert!(r.rt.state.harness_setup.rows.active.is_none() && !r.rt.state.setup_terminal_shown());
}

// Leaving the wizard (or starting another harness) lets go of the socket; the session's drawer is back afterwards.
#[test]
fn closing_the_setup_terminal_gives_the_slot_back() {
    let mut r = rig(vec![opencode("not-installed")]);
    click(&mut r.rt, "harness-setup-install-opencode");
    r.wait_terminal();
    assert!(r.rt.state.setup_terminal_shown());
    click(&mut r.rt, "harness-setup-terminal-close");
    assert!(!r.rt.state.setup_terminal_shown() && r.rt.state.terminal.live.is_none());
}
