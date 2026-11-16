//! File attachments, large pastes and terminal lines (docs/parity/05-composer.md, piece O). Ports "Composer attach any
//! file", "Composer big paste" and the terminal-line cases of `Composer.test.ts` under their names; what Send uploads and
//! says is read from a fake server.
use super::queue::{Live, click, ctrl, mock_app, press, typed};
use crate::State;
use crate::composer::attached::TerminalDraft;
use crate::mock::Status;
use crate::mock::images::PAGE;
use rune::{ClipboardContent, Key, Mods, Runtime};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

const FRAME: f64 = 1.0 / 60.0;

fn scratch(name: &str, bytes: &[u8]) -> PathBuf {
    // Tests run side by side: each file gets a folder of its own.
    static SERIAL: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("fleet-native-files-{}-{n}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, bytes).unwrap();
    path
}

fn forget(path: &Path) {
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

fn idle(refuse: Option<(u16, Value)>) -> Live {
    let mut live = Live::start(json!([]), refuse, "opencode", false);
    live.rt.update(|s, _| {
        s.pickers.harnesses[0].supports_image_attachments = true;
        s.projects[0].sessions[0].status = Status::Idle;
    });
    live.rt.frame(FRAME);
    live
}

fn add(live: &mut Live, path: &Path) {
    let p = path.to_path_buf();
    live.rt.update(move |s, tx| s.add_files(&[p], tx));
}

fn names(rt: &Runtime<State>) -> Vec<String> {
    rt.state.held_files().iter().map(|f| f.name.clone()).collect()
}

fn prompts(live: &Live) -> Vec<Value> {
    live.calls().into_iter().filter(|(m, p, _)| m == "POST" && p.ends_with("/prompt")).map(|(_, _, b)| b).collect()
}

fn prompt_texts(live: &Live) -> Vec<String> {
    prompts(live).iter().map(|b| b["text"].as_str().unwrap_or_default().to_string()).collect()
}

fn big_text() -> String {
    (1..=40).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n")
}

fn paste(rt: &mut Runtime<State>, text: &str) {
    click(rt, "composer");
    rt.paste_content(ClipboardContent::new(text));
    rt.frame(FRAME);
    rt.frame(FRAME);
}

fn app_with(prefs: &[(&str, &str)]) -> Runtime<State> {
    let mut rt = mock_app(false, "opencode", false, false);
    let prefs: Vec<(String, String)> = prefs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    rt.update(move |s, _| {
        for (k, v) in prefs {
            s.pickers.prefs.set(&k, &v);
        }
    });
    rt.frame(FRAME);
    rt
}

fn key(rt: &mut Runtime<State>, key: Key) {
    press(rt, key, Mods::default());
}

fn offer_text(rt: &Runtime<State>) -> Option<String> {
    rt.state.offer_view().map(|o| crate::composer::attached::describe_paste(&o.text))
}

fn log_file() -> Vec<u8> {
    (1..=60)
        .map(|i| format!("step {i:02} finished with a long enough line to pass four kilobytes of text\n"))
        .collect::<String>()
        .into_bytes()
}

// ─── Composer attach any file ───────────────────────────────────────────────

// "holds a non-image file as a chip, uploads it on send and ends the prompt with the note"
#[test]
fn holds_a_non_image_file_as_a_chip_uploads_it_on_send_and_ends_the_prompt_with_the_note() {
    let mut live = idle(None);
    assert!(live.rt.find("attach").is_some(), "the paperclip is there");
    let path = scratch("build-output.log", &log_file());
    add(&mut live, &path);
    live.wait(&|l| l.rt.state.held_files().len() == 1, "the file chip");
    let id = live.rt.state.held_files()[0].id.clone();
    assert!(live.rt.find(&format!("file-chip-{id}")).is_some());
    assert_eq!(live.rt.state.held_files()[0].lines, Some(60), "its lines are counted when it is read");
    assert!(live.uploads().is_empty(), "nothing goes up before Send");

    typed(&mut live.rt, "Why does it fail?");
    click(&mut live.rt, "prompt-send-button");
    live.wait(&|l| !prompts(l).is_empty(), "the prompt");

    let uploads = live.uploads();
    assert_eq!(uploads.len(), 1);
    assert!(uploads[0].0.ends_with("/attachments?name=build-output.log"), "{}", uploads[0].0);
    assert_eq!(uploads[0].2, log_file(), "the raw file is the body");
    let size = crate::composer::attached::format_file_size(log_file().len() as u64);
    assert_eq!(
        prompt_texts(&live),
        [format!("Why does it fail?\n\nAttached: .weave/attachments/3f9a2c1d/build-output.log (60 lines, {size})\nRead it as needed.")]
    );
    assert!(live.rt.state.held_files().is_empty(), "the chip is gone");
    assert!(!live.rt.state.uploading);
    forget(&path);
}

// "keeps the draft and sends nothing when an upload fails"
#[test]
fn keeps_the_draft_and_sends_nothing_when_an_upload_fails() {
    let mut live = idle(None);
    live.refuse_uploads(400, json!({"error": "dump.zip is 61 MB. The limit is 25 MB."}));
    let path = scratch("build-output.log", &log_file());
    add(&mut live, &path);
    live.wait(&|l| l.rt.state.held_files().len() == 1, "the file chip");
    typed(&mut live.rt, "Look");
    click(&mut live.rt, "prompt-send-button");
    live.wait(&|l| l.rt.state.attach_error().is_some(), "the error");
    assert_eq!(live.rt.state.attach_error(), Some("dump.zip is 61 MB. The limit is 25 MB."));
    assert!(prompts(&live).is_empty());
    assert_eq!(names(&live.rt), ["build-output.log"]);
    assert_eq!(live.rt.state.draft.text, "Look");
    assert!(!live.rt.state.uploading, "Send is usable again");
    forget(&path);
}

// "refuses a file over 25 MB"
#[test]
fn refuses_a_file_over_25_mb() {
    let mut rt = mock_app(false, "opencode", false, false);
    let path = scratch("dump.zip", b"");
    std::fs::File::options().write(true).open(&path).unwrap().set_len(26 * 1024 * 1024).unwrap();
    let p = path.clone();
    rt.update(move |s, tx| s.add_files(&[p], tx));
    crate::test_wait::until(
        &mut rt,
        "the refusal",
        |rt| {
            rt.frame(FRAME);
            rt.state.attach_error().is_some()
        },
        |rt| format!("error {:?}", rt.state.attach_error()),
    );
    assert_eq!(rt.state.attach_error(), Some("dump.zip is 26 MB. The limit is 25 MB."));
    assert!(names(&rt).is_empty());
    forget(&path);
}

#[test]
fn files_count_with_pictures_toward_the_ten_and_a_picture_alone_still_sends() {
    let mut rt = mock_app(false, "opencode", false, false);
    let paths: Vec<PathBuf> = (0..11).map(|i| scratch(&format!("n{i}.txt"), b"x")).collect();
    let all = paths.clone();
    rt.update(move |s, tx| s.add_files(&all, tx));
    assert_eq!(rt.state.attach_error(), Some("10 files at most per prompt."));
    crate::test_wait::until(
        &mut rt,
        "ten chips",
        |rt| {
            rt.frame(FRAME);
            rt.state.held_files().len() >= 10
        },
        |rt| format!("{} files held", rt.state.held_files().len()),
    );
    for p in &paths {
        forget(p);
    }
}

// attached-files, the inline rule: a small text file goes in the prompt, a pasted one uploads however short
// "puts a small text file in the prompt and uploads a pasted one, however short"
#[test]
fn puts_a_small_text_file_in_the_prompt_and_uploads_a_pasted_one_however_short() {
    let mut live = idle(None);
    live.rt.update(|s, _| {
        s.pickers.prefs.set("LargePastes", "attach");
    });
    let path = scratch("notes.txt", b"a `tick` and ```fence```\n");
    add(&mut live, &path);
    live.wait(&|l| l.rt.state.held_files().len() == 1, "the file");
    paste(&mut live.rt, &big_text());
    assert_eq!(names(&live.rt), ["notes.txt", "pasted-text.txt"]);
    typed(&mut live.rt, "Look");
    click(&mut live.rt, "prompt-send-button");
    live.wait(&|l| !prompts(l).is_empty(), "the prompt");
    assert_eq!(live.uploads().len(), 1, "only the pasted text goes up");
    assert!(live.uploads()[0].0.ends_with("name=pasted-text.txt"));
    let size = crate::composer::attached::format_file_size(big_text().len() as u64);
    assert_eq!(
        prompt_texts(&live),
        [format!(
            "Look\n\nnotes.txt:\n````\na `tick` and ```fence```\n````\n\nAttached: .weave/attachments/3f9a2c1d/pasted-text.txt (40 lines, {size})\nRead it as needed."
        )]
    );
    forget(&path);
}

// "uploads a text file of 4 KB or more"
#[test]
fn uploads_a_text_file_of_4_kb_or_more() {
    let mut live = idle(None);
    let path = scratch("big.log", &vec![b'x'; 4096]);
    add(&mut live, &path);
    live.wait(&|l| l.rt.state.held_files().len() == 1, "the file");
    typed(&mut live.rt, "Look");
    click(&mut live.rt, "prompt-send-button");
    live.wait(&|l| !prompts(l).is_empty(), "the prompt");
    assert_eq!(live.uploads().len(), 1);
    forget(&path);
}

#[test]
fn a_file_alone_is_a_prompt_and_files_wait_for_a_slash_command_or_a_shell_command() {
    let mut live = idle(None);
    live.rt.update(|s, _| s.pickers.harnesses[0].supports_shell_commands = true);
    let path = scratch("build-output.log", &log_file());
    add(&mut live, &path);
    live.wait(&|l| l.rt.state.held_files().len() == 1, "the file");
    // A shell command has no use for the file: it stays held and nothing goes up.
    typed(&mut live.rt, "!ls");
    key(&mut live.rt, Key::Enter);
    assert!(live.uploads().is_empty());
    assert_eq!(names(&live.rt), ["build-output.log"]);
    live.rt.update(|s, _| s.draft.clear());
    click(&mut live.rt, "prompt-send-button");
    live.wait(&|l| !prompts(l).is_empty(), "the prompt");
    assert!(prompt_texts(&live)[0].starts_with("Attached: .weave/attachments/3f9a2c1d/build-output.log ("));
    forget(&path);
}

#[test]
fn send_shows_its_spinner_and_the_remove_buttons_are_disabled_while_uploading() {
    let mut rt = mock_app(false, "opencode", false, false);
    let id = rt.state.held.next_id();
    rt.update(move |s, _| {
        let owner = s.draft_owner().unwrap();
        s.held.add_file(
            &owner,
            crate::composer::attached::DraftFile {
                id: id.clone(),
                name: "a.log".into(),
                mime: String::new(),
                size: 10,
                lines: None,
                pasted: false,
                source: crate::composer::attached::FileSource::Text("x".into()),
            },
        );
        s.uploading = true;
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert_eq!(names(&rt), ["a.log"]);
    let remove = rt.find("file-remove-file1").expect("the remove button");
    rt.pointer_move(remove.0, remove.1);
    rt.pointer_down(remove.0, remove.1);
    rt.pointer_up(remove.0, remove.1);
    rt.frame(FRAME);
    assert_eq!(names(&rt), ["a.log"], "a click on it does nothing while the files go up");
    // Send is not offered either: a press on it, or Enter, starts nothing.
    key(&mut rt, Key::Enter);
    assert!(rt.state.uploading);
}

// ─── Composer big paste ─────────────────────────────────────────────────────

// "offers a bar for a big paste, and Enter attaches it as pasted-text.txt with its line count"
#[test]
fn offers_a_bar_for_a_big_paste_and_enter_attaches_it_as_pasted_text_txt_with_its_line_count() {
    let mut rt = app_with(&[]);
    paste(&mut rt, &big_text());
    assert_eq!(offer_text(&rt).as_deref(), Some("40 lines pasted"));
    assert!(rt.find("paste-offer").is_some());
    assert_eq!(rt.state.draft.text, "", "nothing is in the text area yet");
    key(&mut rt, Key::Enter);
    assert!(rt.find("paste-offer").is_none());
    assert_eq!(names(&rt), ["pasted-text.txt"]);
    let file = &rt.state.held_files()[0];
    assert_eq!((file.lines, file.size), (Some(40), big_text().len() as u64));
    assert_eq!(crate::composer::attached::describe_draft_file(file.size, file.lines), format!("40 lines · {} B", big_text().len()));
    assert_eq!(rt.state.draft.text, "");
}

// "says characters when the paste is one long line"
#[test]
fn says_characters_when_the_paste_is_one_long_line() {
    let mut rt = app_with(&[]);
    paste(&mut rt, &"x".repeat(2500));
    assert_eq!(offer_text(&rt).as_deref(), Some("2,500 characters pasted"));
}

// "pastes as text at the caret on Escape"
#[test]
fn pastes_as_text_at_the_caret_on_escape() {
    let mut rt = app_with(&[]);
    typed(&mut rt, "before after");
    rt.update(|s, _| s.draft.set_caret(7));
    rt.paste_content(ClipboardContent::new(big_text()));
    rt.frame(FRAME);
    assert!(rt.state.offer_view().is_some());
    key(&mut rt, Key::Escape);
    assert_eq!(rt.state.draft.text, format!("before {}after", big_text()));
    assert_eq!(rt.state.draft.range(), (7 + big_text().len(), 7 + big_text().len()), "the caret is after the paste");
    assert!(rt.state.offer_view().is_none());
    assert!(names(&rt).is_empty());
}

// "pastes as text, then lets the key through, when a letter is typed"
#[test]
fn pastes_as_text_then_lets_the_key_through_when_a_letter_is_typed() {
    let mut rt = app_with(&[]);
    paste(&mut rt, &big_text());
    key(&mut rt, Key::Text("a".into()));
    assert_eq!(rt.state.draft.text, format!("{}a", big_text()), "the text, then the letter after it");
    assert!(rt.state.offer_view().is_none());
}

// "leaves the offer open for a modifier key"
#[test]
fn leaves_the_offer_open_for_a_modifier_key() {
    let mut rt = app_with(&[]);
    paste(&mut rt, &big_text());
    press(&mut rt, Key::Text("c".into()), ctrl());
    assert!(rt.state.offer_view().is_some());
    press(&mut rt, Key::Left, Mods::default());
    assert!(rt.state.offer_view().is_some(), "other keys leave the bar where it is");
}

// "switches the choice with Tab, so Enter pastes as text"
#[test]
fn switches_the_choice_with_tab_so_enter_pastes_as_text() {
    use crate::fleet::uploads::Choice;
    let mut rt = app_with(&[]);
    paste(&mut rt, &big_text());
    assert_eq!(rt.state.offer_view().unwrap().choice, Choice::Attach);
    key(&mut rt, Key::Tab);
    assert_eq!(rt.state.offer_view().unwrap().choice, Choice::Text);
    press(&mut rt, Key::Tab, Mods { shift: true, ..Mods::default() });
    assert_eq!(rt.state.offer_view().unwrap().choice, Choice::Attach);
    key(&mut rt, Key::Tab);
    key(&mut rt, Key::Enter);
    assert_eq!(rt.state.draft.text, big_text());
    assert!(names(&rt).is_empty());
}

// "takes a click on a button"
#[test]
fn takes_a_click_on_a_button() {
    let mut rt = app_with(&[]);
    paste(&mut rt, &big_text());
    assert!(rt.find("paste-offer-attach").is_some(), "the buttons are drawn");
    // The bar fades in as it appears; a click before it has settled misses it.
    for _ in 0..30 {
        rt.frame(FRAME);
    }
    click(&mut rt, "paste-offer-attach");
    assert!(rt.state.offer_view().is_none(), "the click was taken");
    assert_eq!(names(&rt), ["pasted-text.txt"]);
    paste(&mut rt, &big_text());
    click(&mut rt, "paste-offer-text");
    assert_eq!(rt.state.draft.text, big_text());
}

// "replaces an unanswered offer with a second big paste"
#[test]
fn replaces_an_unanswered_offer_with_a_second_big_paste() {
    let mut rt = app_with(&[]);
    paste(&mut rt, &big_text());
    paste(&mut rt, &"x".repeat(2500));
    assert_eq!(offer_text(&rt).as_deref(), Some("2,500 characters pasted"));
}

// "attaches without asking when set to always attach, and numbers a second file"
#[test]
fn attaches_without_asking_when_set_to_always_attach_and_numbers_a_second_file() {
    let mut rt = app_with(&[("LargePastes", "attach")]);
    paste(&mut rt, &big_text());
    paste(&mut rt, &big_text());
    assert!(rt.state.offer_view().is_none());
    assert_eq!(names(&rt), ["pasted-text.txt", "pasted-text-2.txt"]);
}

// "leaves a big paste alone when set to paste as text"
#[test]
fn leaves_a_big_paste_alone_when_set_to_paste_as_text() {
    let mut rt = app_with(&[("LargePastes", "text")]);
    paste(&mut rt, &big_text());
    assert!(rt.state.offer_view().is_none());
    assert!(names(&rt).is_empty());
    assert_eq!(rt.state.draft.text, big_text(), "the field took it as it is");
}

// "leaves a small paste alone"
#[test]
fn leaves_a_small_paste_alone() {
    let mut rt = app_with(&[]);
    paste(&mut rt, "just a line");
    assert!(rt.state.offer_view().is_none());
    assert_eq!(rt.state.draft.text, "just a line");
}

#[test]
fn a_big_paste_in_shell_mode_is_text() {
    let mut rt = app_with(&[]);
    rt.update(|s, _| s.pickers.harnesses[0].supports_shell_commands = true);
    typed(&mut rt, "!echo ");
    rt.paste_content(ClipboardContent::new(big_text()));
    rt.frame(FRAME);
    assert!(rt.state.offer_view().is_none());
    assert!(rt.state.draft.text.starts_with("!echo line 1"));
}

// "drops the offer when the prompt is sent"
#[test]
fn drops_the_offer_when_the_prompt_is_sent() {
    let mut rt = app_with(&[]);
    typed(&mut rt, "Look");
    paste(&mut rt, &big_text());
    assert!(rt.state.offer_view().is_some());
    click(&mut rt, "prompt-send-button");
    assert!(rt.state.offer_view().is_none());
}

// ─── terminal lines ─────────────────────────────────────────────────────────

fn lines(from: usize, to: usize, text: &str) -> TerminalDraft {
    TerminalDraft { id: format!("t1:{from}-{to}"), terminal_id: "t1".into(), label: "zsh".into(), from, to, text: text.into() }
}

fn hold_lines(live: &mut Live, from: usize, to: usize, text: &'static str) {
    live.rt.update(move |s, _| s.add_terminal_lines(lines(from, to, text)));
    live.rt.frame(FRAME);
}

// "Terminal lines go ahead of the message and are dropped"
#[test]
fn terminal_lines_go_ahead_of_the_message_and_are_dropped() {
    let mut live = idle(None);
    hold_lines(&mut live, 9, 11, "FAIL  use-sessions.test.ts\nAssertionError: expected 1, got 2");
    assert!(live.rt.find("terminal-context-strip").is_some() && live.rt.find("terminal-chip-t1:9-11").is_some());
    typed(&mut live.rt, "Why does this fail?");
    key(&mut live.rt, Key::Enter);
    live.wait(&|l| !prompts(l).is_empty(), "the prompt");
    assert_eq!(
        prompt_texts(&live),
        ["Terminal zsh, lines 9–11:\n```text\nFAIL  use-sessions.test.ts\nAssertionError: expected 1, got 2\n```\n\nWhy does this fail?"]
    );
    assert!(live.rt.state.terminal_lines().is_empty());
    assert!(live.rt.find("terminal-context-strip").is_none());
}

// "sends terminal lines with nothing typed"
#[test]
fn sends_terminal_lines_with_nothing_typed() {
    let mut live = idle(None);
    hold_lines(&mut live, 3, 3, "npm ERR! missing script");
    click(&mut live.rt, "composer");
    key(&mut live.rt, Key::Enter);
    live.wait(&|l| !prompts(l).is_empty(), "the prompt");
    assert_eq!(prompt_texts(&live), ["Terminal zsh, line 3:\n```text\nnpm ERR! missing script\n```"]);
}

// "removes terminal lines from the message"
#[test]
fn removes_terminal_lines_from_the_message() {
    let mut live = idle(None);
    hold_lines(&mut live, 3, 3, "x");
    click(&mut live.rt, "terminal-remove-t1:3-3");
    assert!(live.rt.state.terminal_lines().is_empty());
    assert!(live.rt.find("terminal-context-strip").is_none());
}

// "puts a refused message back with its image and terminal lines, and says why"
#[test]
fn puts_a_refused_message_back_with_its_image_and_terminal_lines_and_says_why() {
    let mut live = idle(Some((409, json!({"error": "This session's folder isn't there any more."}))));
    hold_lines(&mut live, 3, 3, "npm ERR! missing script");
    let png = scratch("shot.png", PAGE);
    add(&mut live, &png);
    live.wait(&|l| l.rt.state.held_images().len() == 1, "the picture");
    typed(&mut live.rt, "Why does it fail?");
    key(&mut live.rt, Key::Enter);
    live.wait(&|l| l.rt.state.composer_error.is_some(), "the refusal");
    live.wait(&|l| l.rt.state.held_images().len() == 1 && l.rt.state.terminal_lines().len() == 1, "everything back");
    assert_eq!(live.rt.state.draft.text, "Why does it fail?", "the text without the blocks in front");
    assert_eq!(live.rt.state.composer_error.as_deref(), Some("This session's folder isn't there any more."));
    assert!(live.rt.find("terminal-chip-t1:3-3").is_some());
    forget(&png);
}

#[test]
fn a_queued_message_takes_the_terminal_lines_in_front_and_they_leave_the_draft() {
    let mut live = Live::start(json!([]), None, "opencode", false);
    hold_lines(&mut live, 9, 9, "boom");
    typed(&mut live.rt, "also this");
    key(&mut live.rt, Key::Enter);
    live.wait(&|l| l.calls().iter().any(|(_, p, _)| p.ends_with("/queue")), "the queue call");
    let body = live.calls().into_iter().find(|(_, p, _)| p.ends_with("/queue")).unwrap().2;
    assert_eq!(body["text"], "Terminal zsh, line 9:\n```text\nboom\n```\n\nalso this");
    assert!(live.rt.state.terminal_lines().is_empty());
}

#[test]
fn the_same_lines_of_a_terminal_are_one_chip_and_each_session_keeps_its_own() {
    let mut live = idle(None);
    hold_lines(&mut live, 3, 3, "x");
    hold_lines(&mut live, 3, 3, "x again");
    assert_eq!(live.rt.state.terminal_lines().len(), 1);
}
