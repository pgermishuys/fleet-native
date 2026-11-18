//! Component 13, piece C: terminal tabs per session. Fleet's `stores/__tests__/terminals.test.ts`,
//! `composables/__tests__/use-session-terminals.test.ts`, `lib/__tests__/terminal-mock-shell.test.ts` and the two terminals
//! cases of `session-calls-machine` / `session-events-machine`, under their names. The store and the pretend shell are plain
//! data; the load, the events, the new tab and the close run against a fake Fleet that records what it was asked.
use crate::fleet::http::Terminal;
use crate::fleet::terminals::{DEFAULT_DRAWER_HEIGHT, MIN_DRAWER_HEIGHT, TerminalEvent, Terminals};
use crate::mock::terminal::{Shell, Shells};
use crate::tests::FRAME;
use crate::tests_archive_pins::{Call, Fake, Rig};
use serde_json::{Value, json};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

fn terminal(id: &str, title: &str) -> Terminal {
    Terminal { id: id.into(), title: title.into(), running: true }
}

fn ids(store: &Terminals, session: &str) -> Vec<String> {
    store.terminals_for(session).iter().map(|t| t.id.clone()).collect()
}

fn active(store: &Terminals, session: &str) -> Option<String> {
    store.active_for(session).map(|t| t.id.clone())
}

fn opened(session: &str, id: &str, title: &str) -> TerminalEvent {
    TerminalEvent::Opened { session: session.into(), terminal: id.into(), title: title.into() }
}

fn closed(session: &str, id: &str) -> TerminalEvent {
    TerminalEvent::Closed { session: session.into(), terminal: id.into() }
}

// ---- terminals.test.ts ---------------------------------------------------------------------------------------------------------

#[test]
fn keeps_the_active_tab_when_the_list_is_reloaded_and_falls_back_to_the_first() {
    let mut store = Terminals::default();
    store.set_terminals("s1", vec![terminal("t1", "zsh"), terminal("t2", "zsh 2")]);
    store.set_active("s1", "t2");

    store.set_terminals("s1", vec![terminal("t1", "zsh"), terminal("t2", "zsh 2"), terminal("t3", "zsh 3")]);
    assert_eq!(active(&store, "s1").as_deref(), Some("t2"));

    store.set_terminals("s1", vec![terminal("t3", "zsh 3")]);
    assert_eq!(active(&store, "s1").as_deref(), Some("t3"));
}

#[test]
fn shows_a_tab_you_opened_but_not_one_another_window_opened() {
    let mut store = Terminals::default();
    store.add("s1", terminal("t1", "zsh"), true);

    store.apply_event(&opened("s1", "t2", "zsh 2"));
    assert_eq!(ids(&store, "s1"), ["t1", "t2"]);
    assert_eq!(active(&store, "s1").as_deref(), Some("t1"));

    store.add("s1", terminal("t3", "zsh 3"), true);
    assert_eq!(active(&store, "s1").as_deref(), Some("t3"));
}

#[test]
fn ignores_an_opened_event_for_a_tab_it_already_has() {
    let mut store = Terminals::default();
    store.add("s1", terminal("t1", "bash"), true);

    store.apply_event(&opened("s1", "t1", "zsh"));

    assert_eq!(store.terminals_for("s1"), [terminal("t1", "bash")]);
}

#[test]
fn shows_the_neighbouring_tab_when_the_active_one_closes() {
    let mut store = Terminals::default();
    store.set_terminals("s1", vec![terminal("t1", "zsh"), terminal("t2", "zsh 2"), terminal("t3", "zsh 3")]);
    store.set_active("s1", "t2");

    store.apply_event(&closed("s1", "t2"));
    assert_eq!(active(&store, "s1").as_deref(), Some("t3"));

    store.remove("s1", "t3");
    assert_eq!(active(&store, "s1").as_deref(), Some("t1"));

    store.remove("s1", "t1");
    assert_eq!(active(&store, "s1"), None);
}

#[test]
fn keeps_each_sessions_tabs_and_drawer_apart() {
    let mut store = Terminals::default();
    store.add("s1", terminal("t1", "zsh"), true);
    store.set_open("s1", true);

    assert!(store.terminals_for("s2").is_empty());
    assert!(store.is_open("s1"));
    assert!(!store.is_open("s2"));

    store.toggle_open("s1");
    assert!(!store.is_open("s1"));
}

#[test]
fn never_lets_the_drawer_get_shorter_than_the_minimum() {
    let mut store = Terminals::default();
    assert_eq!(store.height(), DEFAULT_DRAWER_HEIGHT);

    store.set_height(40.0);
    assert_eq!(store.height(), MIN_DRAWER_HEIGHT);

    store.set_height(333.4);
    assert_eq!(store.height(), 333);
}

#[test]
fn forgets_a_sessions_tabs_and_closes_its_drawer() {
    let mut store = Terminals::default();
    store.add("s1", terminal("t1", "zsh"), true);
    store.set_open("s1", true);

    store.forget_session("s1");

    assert!(store.terminals_for("s1").is_empty());
    assert!(!store.is_open("s1"));
}

// 13.39: what is remembered across runs.
#[test]
fn the_open_drawers_and_the_height_are_what_the_settings_file_holds() {
    let mut store = Terminals::default();
    store.set_open("s1", true);
    store.set_height(300.0);
    assert!(store.take_dirty());
    assert!(!store.take_dirty(), "one write per change");

    let mut sidebar = crate::fleet::settings::Sidebar::default();
    sidebar.save_terminals(store.open_sessions().cloned().collect(), store.height());
    let json = sidebar.to_json();
    assert!(json.contains("terminalOpen") && json.contains("terminalHeight"), "{json}");
    let back = crate::fleet::settings::Sidebar::parse(&json);

    let mut fresh = Terminals::default();
    fresh.restore(back.terminal_open().to_vec(), back.terminal_height());
    assert!(fresh.is_open("s1") && !fresh.is_open("s2"));
    assert_eq!(fresh.height(), 300);
    assert!(!fresh.take_dirty(), "reading the file is not a change");
    // A stored height under the minimum is not one (`readHeight`).
    fresh.restore([], Some(100));
    assert_eq!(fresh.height(), DEFAULT_DRAWER_HEIGHT);
}

// ---- terminal-mock-shell.test.ts -----------------------------------------------------------------------------------------------

/// Output with colours removed, so tests read like the screen.
fn plain(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes).to_string();
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.peek() == Some(&'[') {
            for n in chars.by_ref() {
                if n.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// What a terminal shows after typing: its replay, which is everything it printed.
fn screen(shell: &Shell) -> String {
    plain(shell.replay())
}

#[test]
fn the_pretend_shell_starts_the_first_terminal_with_some_output_and_a_prompt() {
    let shell = Shell::new("mock-a-term-1");

    assert!(screen(&shell).contains("git status --short"));
    assert!(screen(&shell).contains("src/glow/mod.rs"));
    assert!(screen(&shell).trim_end().ends_with('❯'));
}

#[test]
fn the_pretend_shell_echoes_typing_and_runs_the_commands_it_knows() {
    let mut shell = Shell::new("mock-b-term-2");

    shell.write(b"pwd\r");
    shell.write(b"cargo test\r");
    shell.write(b"nope\r");

    assert!(screen(&shell).contains("/home/dev/lantern"));
    assert!(screen(&shell).contains("assertion `left == right` failed"));
    assert!(screen(&shell).contains("zsh: command not found: nope"));
}

#[test]
fn the_pretend_shell_keeps_a_dev_server_running_until_ctrl_c() {
    let mut shell = Shell::new("mock-c-term-2");

    shell.write(b"npm run dev\r");
    shell.write(b"ignored\r");
    assert!(screen(&shell).contains("http://localhost:4100/"));
    assert!(!screen(&shell).contains("ignored"));

    shell.write(b"\x03");
    shell.write(b"echo back\r");
    assert!(screen(&shell).contains("back"));
}

#[test]
fn the_pretend_shell_replays_its_output_when_opened_again() {
    let mut shells = Shells::default();
    shells.open("mock-d-term-2").write(b"echo remembered\r");

    assert!(screen(shells.open("mock-d-term-2")).contains("remembered"));
}

#[test]
fn the_pretend_shell_ends_on_exit() {
    let mut shell = Shell::new("mock-e-term-2");
    assert_eq!(shell.exit_code(), None);

    shell.write(b"exit\r");

    assert_eq!(shell.exit_code(), Some(0));
    assert!(shell.write(b"echo late\r").is_empty(), "an ended shell prints nothing");
}

#[test]
fn the_pretend_shell_clears_on_ctrl_l_and_backspace_takes_a_letter_back() {
    let mut shell = Shell::new("mock-f-term-2");
    let out = shell.write(b"lx\x7f");
    assert_eq!(plain(&out), "lx\x08 \x08");
    let out = shell.write(b"s\x0c");
    assert!(String::from_utf8_lossy(&out).contains("\x1b[2J\x1b[H"), "Ctrl+L redraws the prompt and what is typed");
    assert!(plain(&out).ends_with("❯ ls"));
}

// ---- use-session-terminals.test.ts, against a fake Fleet -----------------------------------------------------------------------

/// What was asked of the session's terminals: the list, the create and the close (not the shells' sockets).
pub(crate) fn asked(fake: &Fake) -> Vec<Call> {
    fake.calls("/terminals").into_iter().filter(|c| !c.1.contains("/socket")).collect()
}

pub(crate) fn entry(id: &str, title: &str) -> Value {
    json!({"id": id, "title": title, "status": "running", "createdAt": "2026-11-18T10:00:00Z"})
}

/// A Fleet that lists `listed` and answers the other terminal calls with `answer`.
pub(crate) fn serving(listed: Vec<Value>, other: impl Fn(&str, &str) -> (u16, String) + Send + Sync + 'static) -> Fake {
    Fake::start(move |method, path| {
        if method == "GET" && path.ends_with("/terminals") { (200, Value::Array(listed.clone()).to_string()) } else { other(method, path) }
    })
}

pub(crate) fn rig(fake: &Fake) -> Rig {
    let mut r = Rig::home(fake);
    r.rt.update(|s, _| s.set_session_id((0, 0), "s1"));
    r.rt.frame(FRAME);
    r
}

pub(crate) fn enable(r: &mut Rig, on: bool) {
    r.rt.update(move |s, _| {
        s.terminals_allowed = Some(on);
        s.terminal.session = s.open_id();
        if let Some(id) = s.open_id() {
            s.load_terminals(&id);
        }
    });
}

pub(crate) fn tabs(r: &Rig, session: &str) -> Vec<String> {
    ids(&r.rt.state.terminals, session)
}

fn event(r: &mut Rig, topic_session: &str, e: crate::fleet::events::Event) {
    let topic = crate::fleet::state::Topic::Session(topic_session.to_string());
    r.rt.update(move |s, _| s.dispatch(&topic, &[e]));
}

fn terminal_event(kind: crate::fleet::events::Kind, session: &str, id: &str, title: &str) -> crate::fleet::events::Event {
    crate::fleet::events::Event::Other(kind, json!({"sessionId": session, "terminalId": id, "title": title}))
}

fn no_server_loads(r: &mut Rig) {
    r.wait("nothing in flight", |s| s.terminal_load_idle());
}

#[test]
fn loads_the_sessions_terminals_and_applies_its_terminal_events() {
    use crate::fleet::events::Kind;
    let fake = serving(vec![entry("t1", "zsh")], |_, _| (204, String::new()));
    let mut r = rig(&fake);
    enable(&mut r, true);
    r.wait("the list", |s| ids(&s.terminals, "s1") == ["t1"]);
    let calls = asked(&fake);
    assert_eq!((calls[0].0.as_str(), calls[0].1.as_str()), ("GET", "/api/sessions/s1/terminals"));

    event(&mut r, "s1", terminal_event(Kind::TerminalOpened, "s1", "t2", "zsh 2"));
    // Another session's terminal on this session's topic is not this one's.
    event(&mut r, "s1", terminal_event(Kind::TerminalOpened, "other", "t9", "zsh"));
    assert_eq!(tabs(&r, "s1"), ["t1", "t2"]);
    assert!(tabs(&r, "other").is_empty());
}

#[test]
fn closes_the_drawer_when_the_last_terminal_goes() {
    use crate::fleet::events::Kind;
    let fake = serving(vec![entry("t1", "zsh")], |_, _| (204, String::new()));
    let mut r = rig(&fake);
    enable(&mut r, true);
    r.wait("the list", |s| ids(&s.terminals, "s1") == ["t1"]);
    r.rt.update(|s, _| s.terminals.set_open("s1", true));

    event(&mut r, "s1", terminal_event(Kind::TerminalClosed, "s1", "t1", "zsh"));

    assert!(!r.rt.state.terminals.is_open("s1"));
}

#[test]
fn closes_a_drawer_left_open_on_a_session_with_no_terminals_left() {
    let fake = serving(vec![], |_, _| (204, String::new()));
    let mut r = rig(&fake);
    // Remembered from the last run: open before the list is read.
    r.rt.update(|s, _| s.terminals.set_open("s1", true));
    enable(&mut r, true);
    r.wait("the list", |s| s.terminals.is_loaded("s1"));

    assert!(!r.rt.state.terminals.is_open("s1"));
}

#[test]
fn keeps_a_drawer_opened_while_the_empty_list_was_loading_so_it_starts_a_shell() {
    let hold = Arc::new(AtomicBool::new(true));
    let gate = hold.clone();
    let fake = Fake::start(move |method, path| match (method, path.ends_with("/terminals")) {
        ("GET", true) => {
            while gate.load(Ordering::SeqCst) {
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            (200, "[]".to_string())
        }
        ("POST", true) => (201, entry("t-new", "zsh").to_string()),
        _ => (204, String::new()),
    });
    let mut r = rig(&fake);
    enable(&mut r, true);

    // The user asks for a shell while the list is on its way.
    r.rt.update(|s, _| s.terminals.set_open("s1", true));
    hold.store(false, Ordering::SeqCst);
    r.wait("the list", |s| s.terminals.is_loaded("s1"));
    assert!(r.rt.state.terminals.is_open("s1"));
    r.wait("a shell to start", |s| ids(&s.terminals, "s1") == ["t-new"]);
    let posts: Vec<_> = asked(&fake).into_iter().filter(|c| c.0 == "POST").collect();
    assert_eq!(posts.len(), 1);
    // The box under the open drawer may have been laid out before the answer came: then its size, else 120 × 14.
    let known = r.rt.state.terminal.fitted.map(|(c, rows)| json!({"cols": c, "rows": rows}));
    assert!(posts[0].2 == json!({"cols": 120, "rows": 14}) || Some(posts[0].2.clone()) == known, "{:?}", posts[0].2);
}

#[test]
fn does_nothing_when_fleet_has_terminals_turned_off() {
    let fake = serving(vec![entry("t1", "zsh")], |_, _| (204, String::new()));
    let mut r = rig(&fake);
    enable(&mut r, false);
    no_server_loads(&mut r);
    // The command is off, the key does nothing, and a late event is not applied.
    r.rt.update(|s, tx| s.toggle_terminal(tx));
    event(&mut r, "s1", terminal_event(crate::fleet::events::Kind::TerminalOpened, "s1", "t2", "zsh"));
    r.rt.frame(FRAME);
    assert!(!r.rt.state.terminals.is_open("s1"));
    assert!(tabs(&r, "s1").is_empty());
    assert!(asked(&fake).is_empty(), "{:?}", asked(&fake));
}

#[test]
fn reloads_after_a_reconnect() {
    let lists = Arc::new(Mutex::new(0));
    let counted = lists.clone();
    let fake = Fake::start(move |method, path| {
        if method == "GET" && path.ends_with("/terminals") {
            let mut n = counted.lock().unwrap();
            *n += 1;
            let list = if *n == 1 { vec![entry("t1", "zsh")] } else { vec![entry("t1", "zsh"), entry("t2", "zsh 2")] };
            return (200, Value::Array(list).to_string());
        }
        (204, String::new())
    });
    let mut r = rig(&fake);
    enable(&mut r, true);
    r.wait("the first list", |s| ids(&s.terminals, "s1") == ["t1"]);

    r.rt.update(|s, _| s.on_reconnected());

    r.wait("the list after the reconnect", |s| ids(&s.terminals, "s1").len() == 2);
}

#[test]
fn opens_a_new_terminal_shows_it_and_opens_the_drawer() {
    let fake = serving(vec![], |method, path| {
        if method == "POST" && path.ends_with("/terminals") { (201, entry("t7", "zsh").to_string()) } else { (204, String::new()) }
    });
    let mut r = rig(&fake);
    r.rt.update(|s, _| {
        s.terminals_allowed = Some(true);
        s.terminal.session = s.open_id();
        s.terminal.fitted = Some((120, 30));
        s.open_new_terminal();
    });

    r.wait("the new tab", |s| active(&s.terminals, "s1").as_deref() == Some("t7"));

    let posts: Vec<_> = asked(&fake).into_iter().filter(|c| c.0 == "POST").collect();
    assert_eq!(posts[0].1, "/api/sessions/s1/terminals");
    assert_eq!(posts[0].2, json!({"cols": 120, "rows": 30}));
    assert!(r.rt.state.terminals.is_open("s1"));
}

#[test]
fn returns_the_servers_message_when_a_terminal_cant_be_opened() {
    let message = "This session already has 8 terminals. Close one to open another.";
    let fake = serving(
        vec![],
        move |method, _| if method == "POST" { (409, json!({ "error": message }).to_string()) } else { (204, String::new()) },
    );
    let mut r = rig(&fake);
    r.rt.update(|s, _| {
        s.terminals_allowed = Some(true);
        s.terminal.session = s.open_id();
        s.open_new_terminal();
    });

    r.wait("the refusal", |s| s.terminal.error.is_some());

    assert_eq!(r.rt.state.terminal.error.as_deref(), Some(message));
    assert!(tabs(&r, "s1").is_empty());
}

#[test]
fn closes_a_tab_at_once_and_puts_it_back_if_the_server_refuses() {
    let fake = serving(vec![entry("t1", "zsh")], |method, _| {
        if method == "DELETE" { (500, json!({"error": "boom"}).to_string()) } else { (204, String::new()) }
    });
    let mut r = rig(&fake);
    enable(&mut r, true);
    r.wait("the list", |s| ids(&s.terminals, "s1") == ["t1"]);
    // The drawer is open with no connection to the fake (it only speaks HTTP), which is all this needs.
    r.rt.update(|s, _| {
        s.terminals.set_open("s1", true);
        s.close_terminal_tab("t1");
    });

    // At once: gone, and the drawer with it, before the server has answered.
    assert!(tabs(&r, "s1").is_empty());
    assert!(!r.rt.state.terminals.is_open("s1"));
    r.wait("the tab to come back", |s| ids(&s.terminals, "s1") == ["t1"]);
    let deletes: Vec<_> = asked(&fake).into_iter().filter(|c| c.0 == "DELETE").collect();
    assert_eq!(deletes.len(), 1);
    assert_eq!(deletes[0].1, "/api/sessions/s1/terminals/t1");
}

// ---- ours --------------------------------------------------------------------------------------------------------------------

#[test]
fn a_shell_that_ended_takes_its_tab_and_the_neighbour_shows() {
    let mut store = Terminals::default();
    store.set_terminals("s1", vec![terminal("t1", "zsh"), terminal("t2", "zsh 2")]);
    store.set_active("s1", "t1");
    store.remove("s1", "t1");
    assert_eq!(active(&store, "s1").as_deref(), Some("t2"));
}

#[test]
fn the_toggle_does_nothing_without_a_session_or_with_terminals_off() {
    let mut rt = crate::tests::app();
    rt.update(|s, tx| s.toggle_terminal(tx));
    assert!(!rt.state.terminal_open());
    rt.update(|s, tx| {
        s.set_session_id((0, 0), "s1");
        s.terminals_allowed = Some(false);
        s.toggle_terminal(tx);
    });
    assert!(!rt.state.terminals.is_open("s1"), "terminals are off when the server says so");
}

#[test]
fn the_pretend_shell_answers_typing_into_the_grid_and_exit_takes_its_tab() {
    let mut rt = crate::tests::app();
    rt.update(|s, _| s.show_terminal_mock(true));
    rt.frame(FRAME);
    let rows = |rt: &rune::Runtime<crate::State>| crate::term::text_of(&rt.state.terminal.live.as_ref().unwrap().term);
    assert!(rows(&rt).iter().any(|r| r.contains("git status --short")));
    rt.update(|s, _| s.terminal_write(b"pwd\r".to_vec()));
    assert!(rows(&rt).iter().any(|r| r == "/home/dev/lantern"), "{:?}", rows(&rt));
    rt.update(|s, _| s.terminal_write(b"exit\r".to_vec()));
    rt.frame(FRAME);
    assert!(rt.state.terminal.live.is_none());
}
