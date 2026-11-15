//! Fleet's tests for this logic, under their names (the Vue sentence in the comment), and the ones native adds.
use super::drafts::*;
use super::history::*;
use super::route::*;
use super::status::*;

const STEERS: Caps = Caps {
    can_steer: true,
    steers_by_default: true,
    steer_delivery: None,
    supports_shell: true,
    supports_side: true,
    supports_images: true,
};

fn harness(kind: &str, steering: bool, by_default: bool, shell: bool, side: bool, images: bool) -> HarnessInfo {
    HarnessInfo {
        kind: kind.into(),
        supports_steering: steering,
        steers_by_default: by_default,
        steer_delivery: if kind == "opencode2" { Some(SteerDelivery::NextStep) } else { None },
        supports_shell_commands: shell,
        supports_side_conversations: side,
        supports_image_attachments: images,
    }
}

// composerStatus: "reads busy, waiting and idle"
#[test]
fn composer_status_reads_busy_waiting_and_idle() {
    assert_eq!(composer_status(Some("busy"), false), ComposerStatus::Busy);
    assert_eq!(composer_status(Some("delegating"), false), ComposerStatus::Busy);
    assert_eq!(composer_status(Some("retry"), false), ComposerStatus::Busy);
    assert_eq!(composer_status(Some("waiting_input"), false), ComposerStatus::WaitingInput);
    assert_eq!(composer_status(Some("idle"), false), ComposerStatus::Idle);
    assert_eq!(composer_status(None, true), ComposerStatus::Busy);
}

fn harnesses() -> Vec<HarnessInfo> {
    vec![
        harness("opencode2", true, true, true, true, true),
        harness("opencode", true, false, true, false, true),
        harness("claude-code", true, true, false, false, true),
        harness("pi", false, false, false, false, false),
    ]
}

// harnessCapabilities: "steers by default where the harness says so"
#[test]
fn harness_capabilities_steers_by_default_where_the_harness_says_so() {
    let h = harnesses();
    assert_eq!(harness_capabilities(Some("opencode2"), &h), Caps { steer_delivery: Some(SteerDelivery::NextStep), ..STEERS });
    let opencode = harness_capabilities(Some("opencode"), &h);
    assert!(opencode.can_steer && !opencode.steers_by_default && !opencode.supports_side);
    let claude = harness_capabilities(Some("claude-code"), &h);
    assert!(claude.can_steer && claude.steers_by_default && !claude.supports_shell);
    assert_eq!(harness_capabilities(Some("pi"), &h), Caps::default());
    // No session, or a harness the list doesn't have: nothing harness-specific is on.
    assert_eq!(harness_capabilities(None, &h), Caps::default());
    assert_eq!(harness_capabilities(Some("other"), &h), Caps::default());
}

// harnessCapabilities: "offers images only where the harness passes them on"
#[test]
fn harness_capabilities_offers_images_only_where_the_harness_passes_them_on() {
    let h = harnesses();
    assert!(harness_capabilities(Some("claude-code"), &h).supports_images);
    assert!(!harness_capabilities(Some("pi"), &h).supports_images);
}

fn cx(status: ComposerStatus, steer: bool, caps: Caps, side_open: bool) -> DraftContext {
    DraftContext { status, steer, caps, side_open }
}
const IDLE: ComposerStatus = ComposerStatus::Idle;
const BUSY: ComposerStatus = ComposerStatus::Busy;

// routeDraft: "sends when idle and queues while working"
#[test]
fn route_draft_sends_when_idle_and_queues_while_working() {
    assert_eq!(route_draft("fix it", &cx(IDLE, false, STEERS, false)), Route::Prompt { queue: false, steer: false });
    assert_eq!(route_draft("fix it", &cx(BUSY, false, STEERS, false)), Route::Prompt { queue: true, steer: false });
}

// routeDraft: "steers only when asked and the harness can"
#[test]
fn route_draft_steers_only_when_asked_and_the_harness_can() {
    assert_eq!(route_draft("look at the logs first", &cx(BUSY, true, STEERS, false)), Route::Prompt { queue: false, steer: true });
    assert_eq!(route_draft("look at the logs first", &cx(BUSY, true, Caps::default(), false)), Route::Prompt { queue: true, steer: false });
}

// routeDraft: "routes ! to the shell, queued while working, where the harness runs commands"
#[test]
fn route_draft_routes_bang_to_the_shell_queued_while_working_where_the_harness_runs_commands() {
    assert_eq!(route_draft("!git status", &cx(IDLE, false, STEERS, false)), Route::Shell { command: "git status".into(), queue: false });
    assert_eq!(route_draft("!git status", &cx(BUSY, false, STEERS, false)), Route::Shell { command: "git status".into(), queue: true });
    assert!(matches!(route_draft("!git status", &cx(IDLE, false, Caps::default(), false)), Route::Prompt { .. }));
}

// routeDraft: "routes /btw and anything typed while the side is open to the side"
#[test]
fn route_draft_routes_btw_and_anything_typed_while_the_side_is_open_to_the_side() {
    assert_eq!(
        route_draft("/btw what's a flaky test?", &cx(BUSY, false, STEERS, false)),
        Route::Side { question: "what's a flaky test?".into() }
    );
    assert_eq!(route_draft("and another", &cx(IDLE, false, STEERS, true)), Route::Side { question: "and another".into() });
}

// routeDraft: "routes slash commands as commands, queued while working"
#[test]
fn route_draft_routes_slash_commands_as_commands_queued_while_working() {
    assert_eq!(
        route_draft("/review src", &cx(IDLE, false, STEERS, false)),
        Route::Command { command: "review".into(), args: "src".into(), queue: false }
    );
    assert!(matches!(route_draft("/review src", &cx(BUSY, true, STEERS, false)), Route::Command { queue: true, .. }));
}

// routeDraft: "ignores an empty draft"
#[test]
fn route_draft_ignores_an_empty_draft() {
    assert_eq!(route_draft("   ", &cx(IDLE, false, STEERS, false)), Route::Empty);
}

// canInterruptSession: "allows stopping a running turn and a session waiting on a question or permission"
#[test]
fn can_interrupt_allows_stopping_a_running_turn_and_a_session_waiting_on_a_question_or_permission() {
    for activity in ["busy", "retry", "delegating", "waiting_input"] {
        assert!(can_interrupt(composer_status(Some(activity), false), false), "{activity}");
    }
}

// canInterruptSession: "offers nothing when idle or already aborting"
#[test]
fn can_interrupt_offers_nothing_when_idle_or_already_aborting() {
    assert!(!can_interrupt(composer_status(Some("idle"), false), false));
    assert!(!can_interrupt(ComposerStatus::Busy, true));
}

// shell-commands.test.ts: "is shell mode only when the draft starts with !"
#[test]
fn is_shell_mode_only_when_the_draft_starts_with_bang() {
    assert!(is_shell_draft("!ls"));
    assert!(is_shell_draft("!"));
    assert!(!is_shell_draft(" !ls"));
    assert!(!is_shell_draft("ls !"));
}

// shell-commands.test.ts: "runs what follows the !"
#[test]
fn runs_what_follows_the_bang() {
    assert_eq!(shell_draft_command("!  git status  "), "git status");
    assert_eq!(shell_draft_command("!"), "");
    assert_eq!(shell_draft_command("git status"), "");
}

// side-conversation.test.ts: "reads the question after /btw"
#[test]
fn reads_the_question_after_btw() {
    assert_eq!(parse_side_question("/btw why?").as_deref(), Some("why?"));
    assert_eq!(parse_side_question("  /btw   why?\nand more  ").as_deref(), Some("why?\nand more"));
}

// side-conversation.test.ts: "gives an empty question for a bare /btw"
#[test]
fn gives_an_empty_question_for_a_bare_btw() {
    assert_eq!(parse_side_question("/btw").as_deref(), Some(""));
    assert_eq!(parse_side_question("/btw   ").as_deref(), Some(""));
}

// side-conversation.test.ts: "isn't fooled by other commands or text"
#[test]
fn isnt_fooled_by_other_commands_or_text() {
    assert_eq!(parse_side_question("/btwx hi"), None);
    assert_eq!(parse_side_question("/review"), None);
    assert_eq!(parse_side_question("please /btw this"), None);
}

// parseSlashCommand (slash-command-utils)
#[test]
fn parses_a_slash_command() {
    assert_eq!(parse_slash_command("/review src/a.rs "), Some(("review".into(), "src/a.rs".into())));
    assert_eq!(parse_slash_command("  /init"), Some(("init".into(), String::new())));
    assert_eq!(parse_slash_command("/"), None);
    assert_eq!(parse_slash_command("/ x"), None);
    assert_eq!(parse_slash_command("not /a command"), None);
}

// 12.4: Enter queues and Ctrl+Enter steers, unless the harness steers by default.
#[test]
fn the_key_decides_steer_by_what_the_harness_does_by_default() {
    let plain = Caps { steers_by_default: false, ..STEERS };
    assert!(!steer_for_key(&plain, false));
    assert!(steer_for_key(&plain, true));
    assert!(steer_for_key(&STEERS, false));
    assert!(!steer_for_key(&STEERS, true));
}

// 1.9 and 15.1
#[test]
fn a_session_that_cant_take_a_prompt_disables_the_composer() {
    assert!(can_send(None));
    let running = SessionGate { archived: false, can_prompt: None, lifecycle: "running" };
    assert!(can_send(Some(running)));
    assert!(!can_send(Some(SessionGate { lifecycle: "error", ..running })));
    assert!(!can_send(Some(SessionGate { archived: true, ..running })));
    // The row's own `canPrompt` wins over the lifecycle.
    assert!(can_send(Some(SessionGate { can_prompt: Some(true), lifecycle: "stopped", ..running })));
    assert!(!can_send(Some(SessionGate { can_prompt: Some(false), ..running })));
}

#[test]
fn send_has_content_for_text_or_a_shell_command_and_not_while_a_bang_draft_waits() {
    assert!(!has_content("   ", false, false, 0));
    assert!(has_content("hi", false, false, 0));
    assert!(has_content("", false, false, 1));
    assert!(!has_content("hi", false, true, 0));
    assert!(!has_content("!  ", true, false, 0));
    assert!(has_content("!ls", true, false, 0));
}

// ─── message history ────────────────────────────────────────────────────────

#[test]
fn history_keeps_ten_trimmed_newest_first_without_repeats() {
    let mut h = History::default();
    assert!(!h.push("   "));
    for n in 1..=12 {
        h.push(&format!(" message {n} "));
    }
    assert_eq!(h.entries.len(), MAX_HISTORY);
    assert_eq!(h.entries[0], "message 12");
    assert_eq!(h.entries[9], "message 3");
    // A repeat moves to the front.
    h.push("message 5");
    assert_eq!(h.entries[0], "message 5");
    assert_eq!(h.entries.iter().filter(|e| *e == "message 5").count(), 1);
    assert_eq!(h.entries.len(), MAX_HISTORY);
}

#[test]
fn history_opens_on_the_newest_and_walks_without_wrapping() {
    let mut h = History::default();
    h.open();
    assert!(!h.open, "nothing happens with no entries");
    for t in ["one", "two", "three"] {
        h.push(t);
    }
    h.open();
    assert_eq!(h.selected, Some(0));
    h.newer();
    assert_eq!(h.selected, Some(0));
    h.older();
    h.older();
    h.older();
    assert_eq!(h.selected, Some(2));
    h.newer();
    assert_eq!(h.confirm().as_deref(), Some("two"));
    assert!(!h.open && h.selected.is_none());
    assert_eq!(h.confirm(), None);
}

// ─── drafts ─────────────────────────────────────────────────────────────────

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("fleet-native-drafts-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

// draft-storage.test.ts: "keeps a draft and forgets an empty one"
#[test]
fn keeps_a_draft_and_forgets_an_empty_one() {
    let mut d = Drafts::default();
    d.write("session.a", "half a thought", 0.0);
    assert_eq!(d.read("session.a"), Some("half a thought"));
    d.write("session.a", "", 0.0);
    assert_eq!(d.read("session.a"), None);
    assert!(d.keys("").is_empty());
}

// draft-storage.test.ts: "lists keys by their start"
#[test]
fn lists_keys_by_their_start() {
    let mut d = Drafts::default();
    d.write(&side_draft_key("s1", "x"), "one", 0.0);
    d.write(&side_draft_key("s1", "y"), "two", 0.0);
    d.write(&side_draft_key("s2", "z"), "three", 0.0);
    assert_eq!(d.keys("side.s1."), ["side.s1.x", "side.s1.y"]);
    // A deleted session takes all its drafts with it.
    d.write(&session_draft_key("s1"), "own", 0.0);
    d.write(&side_main_draft_key("s1"), "main", 0.0);
    d.clear_of("s1", 0.0);
    assert_eq!(d.keys(""), ["side.s2.z"]);
}

// draft-storage.test.ts: "works in memory when storage throws"
#[test]
fn works_in_memory_when_storage_throws() {
    // The data folder is a file, so nothing can be created under it.
    let dir = temp_dir("blocked");
    std::fs::create_dir_all(&dir).unwrap();
    let blocker = dir.join("not-a-folder");
    std::fs::write(&blocker, "x").unwrap();
    assert!(write_file(&blocker.join("inside"), "{}").is_err());
    assert_eq!(load(&blocker.join("inside")), Stored::default());
    let mut d = Drafts::default();
    d.set_dir(Some(blocker.join("inside")));
    d.write("session.blocked", "still here", 0.0);
    let (folder, json) = d.take_save().unwrap();
    assert!(write_file(&folder, &json).is_err());
    assert_eq!(d.text_of("blocked"), "still here");
    let _ = std::fs::remove_dir_all(&dir);
}

// draft-storage.test.ts, "composer drafts": "are kept in the browser, and cleared on send" (here: in the file)
#[test]
fn drafts_are_kept_in_the_file_and_cleared_on_send() {
    let dir = temp_dir("kept");
    let mut d = Drafts::default();
    d.set_dir(Some(dir.clone()));
    d.write(&session_draft_key("kept"), "what I was typing", 1.0);
    assert!(!d.save_due(1.1), "a change waits a moment, so typing writes once");
    assert!(d.save_due(1.0 + SAVE_AFTER));
    let (folder, json) = d.take_save().unwrap();
    write_file(&folder, &json).unwrap();
    assert_eq!(load(&dir).drafts.get("session.kept").map(String::as_str), Some("what I was typing"));
    assert!(d.take_save().is_some() && !d.unsaved());

    d.write(&session_draft_key("kept"), "", 2.0);
    let (folder, json) = d.take_save().unwrap();
    write_file(&folder, &json).unwrap();
    assert!(load(&dir).drafts.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

// draft-storage.test.ts, "composer drafts": "come back after a reload" (here: a restart, with the history)
#[test]
fn drafts_come_back_after_a_restart() {
    let dir = temp_dir("restart");
    let mut first = Drafts::default();
    first.set_dir(Some(dir.clone()));
    first.write(&session_draft_key("a"), "before the restart", 0.0);
    first.write(&session_draft_key("mock.0.1"), "invented", 0.0);
    first.push_history("a", "an earlier prompt", 0.0);
    let (folder, json) = first.take_save().unwrap();
    write_file(&folder, &json).unwrap();

    let mut second = Drafts::default();
    // Typed before the file was read: kept over what the file has.
    second.write(&session_draft_key("b"), "typed first", 0.0);
    second.merge(load(&dir));
    assert_eq!(second.text_of("a"), "before the restart");
    assert_eq!(second.text_of("b"), "typed first");
    assert_eq!(second.history("a").map(|h| h.entries.clone()), Some(vec!["an earlier prompt".to_string()]));
    assert_eq!(second.text_of("mock.0.1"), "", "the mock data's drafts are never written down");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_damaged_file_is_an_empty_store() {
    assert_eq!(parse("{ not json"), Stored::default());
    assert_eq!(parse("[1,2]"), Stored::default());
    assert_eq!(parse(r#"{"drafts":{"session.a":5,"session.b":"ok"}}"#).drafts.len(), 1);
}

#[test]
fn the_apps_folder_is_its_own_and_never_fleets() {
    let env =
        |pairs: &'static [(&'static str, &'static str)]| move |k: &str| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string());
    assert_eq!(app_dir(env(&[("FLEET_NATIVE_DATA_DIR", "/data/here")])), Some("/data/here".into()));
    assert_eq!(app_dir(env(&[])), None);
    if cfg!(target_os = "linux") {
        assert_eq!(app_dir(env(&[("HOME", "/h")])), Some("/h/.local/share/fleet-native".into()));
        assert_eq!(app_dir(env(&[("HOME", "/h"), ("XDG_DATA_HOME", "/x")])), Some("/x/fleet-native".into()));
    }
    let dir = app_dir(env(&[("HOME", "/h"), ("APPDATA", "/h/roaming"), ("FLEET_DESKTOP_DATA_DIR", "/h/.weave")])).unwrap();
    assert!(!dir.to_string_lossy().contains(".weave"));
}

#[test]
fn the_composer_reads_busy_after_a_send_until_the_session_has_been_busy_and_stops() {
    let mut o = Optimistic::new(10.0);
    assert!(o.keep(false, 11.0), "the gap before the server's first busy");
    assert!(o.keep(true, 12.0));
    assert!(!o.keep(false, 13.0), "busy and then not: the turn is over");
}

#[test]
fn an_optimistic_busy_gives_up_on_a_prompt_that_never_became_a_turn() {
    let mut o = Optimistic::new(10.0);
    assert!(o.keep(false, 24.9));
    assert!(!o.keep(false, 25.1));
}

#[test]
fn the_draft_is_a_shell_command_only_where_the_harness_runs_them_and_the_side_conversation_wins() {
    let plain = Caps::default();
    assert_eq!(mode_of("!ls", &plain, false), Mode::Prompt);
    assert_eq!(mode_of("!ls", &STEERS, false), Mode::Shell);
    assert_eq!(mode_of("ls", &STEERS, false), Mode::Prompt);
    assert_eq!(mode_of("!ls", &STEERS, true), Mode::Side);
    assert_eq!((send_label(Mode::Prompt), send_label(Mode::Shell)), ("Send", "Run command"));
    assert_eq!(send_label(Mode::Side), "Ask in the side conversation");
}
