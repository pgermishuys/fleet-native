//! `components/session/__tests__/ExportMarkdownDialog.test.ts`, under the same names, and the command's open
//! (`ActivityStream.commands.test.ts`). Fleet's test mocks `loadWholeHistory`; here the dialog is handed a canned list.
//! Saving writes only into a temporary folder.
use crate::State;
use crate::conv::export::ExportToolMode;
use crate::conv::export_dialog::{Load, Pane};
use crate::conv::export_history::HistoryError;
use crate::fleet::model::{Msg, PartData};
use crate::tests::{FRAME, app, click};
use crate::view::conversation::Role;
use rune::{FileDialogKind, Runtime};
use serde_json::json;

fn chat(with_reasoning: bool) -> Vec<Msg> {
    let mut user = Msg::new("m1", "s1", Role::User);
    user.parts = vec![PartData::Text { id: "a".into(), text: "Add retries".into() }];
    let mut bot = Msg::new("m2", "s1", Role::Assistant);
    if with_reasoning {
        bot.parts.push(PartData::Reasoning { id: "r".into(), text: "think".into(), summary: None });
    }
    bot.parts.push(PartData::Tool {
        id: "t".into(),
        tool: "bash".into(),
        call_id: "c".into(),
        state: json!({"status": "completed", "input": {"command": "env"}, "output": "API_KEY=abcdef123456789"}),
    });
    bot.parts.push(PartData::Text { id: "b".into(), text: "Done.".into() });
    vec![user, bot]
}

/// The dialog open and waiting, as it is while the first page is on its way.
fn waiting() -> Runtime<State> {
    let mut rt = app();
    rt.update(|s, _| s.show_export(Load::Loading(0)));
    rt.frame(FRAME);
    rt
}

fn generation(rt: &Runtime<State>) -> u64 {
    rt.state.export.dialog.as_ref().unwrap().generation
}

fn finish(rt: &mut Runtime<State>, messages: Vec<Msg>) {
    let g = generation(rt);
    rt.update(move |s, _| s.export_done(g, Ok(messages)));
    rt.frame(FRAME);
}

// "shows how many messages it has loaded, then the file"
#[test]
fn shows_how_many_messages_it_has_loaded_then_the_file() {
    let mut rt = waiting();
    let g = generation(&rt);
    rt.update(move |s, _| s.export_progress(g, 120));
    rt.frame(FRAME);
    assert!(rt.find("export-loading").is_some());
    assert_eq!(rt.state.export.dialog.as_ref().unwrap().load, Load::Loading(120));
    let (save, copy) = (rt.find("export-save").unwrap(), rt.find("export-copy").unwrap());
    // Disabled until built: a press does nothing.
    rt.pointer_down(copy.0, copy.1);
    rt.pointer_up(copy.0, copy.1);
    rt.pointer_down(save.0, save.1);
    rt.pointer_up(save.0, save.1);
    rt.frame(FRAME);
    assert!(rt.state.export.dialog.as_ref().unwrap().copied_until.is_none() && rt.file_dialog().is_none());

    finish(&mut rt, chat(false));
    assert!(rt.find("export-source").is_some() && rt.find("export-loading").is_none());
    let d = rt.state.export.dialog.as_ref().unwrap();
    assert!(d.markdown().contains("## You"));
    assert!(d.size_label().unwrap().starts_with("2 turns"));
    assert!(rt.find("export-size").is_some());
}

// "says so, with a way to try again, when a page fails"
#[test]
fn says_so_with_a_way_to_try_again_when_a_page_fails() {
    let mut rt = waiting();
    let g = generation(&rt);
    let why = "Fleet could not load the conversation (error 502 after 200 messages).";
    rt.update(move |s, _| s.export_done(g, Err(HistoryError::Failed(why.into()))));
    rt.frame(FRAME);
    assert!(rt.find("export-error").is_some());
    assert_eq!(rt.state.export.dialog.as_ref().unwrap().load, Load::Failed(why.into()));
    click(&mut rt, "export-retry");
    // With no Fleet the invented session's messages are the history, so the retry finishes at once.
    assert!(rt.find("export-error").is_none());
    assert!(rt.find("export-source").is_some());
}

// "changes the file at once when tool output is switched on, and warns about keys"
#[test]
fn changes_the_file_at_once_when_tool_output_is_switched_on_and_warns_about_keys() {
    let mut rt = waiting();
    finish(&mut rt, chat(false));
    assert!(!rt.state.export.dialog.as_ref().unwrap().markdown().contains("API_KEY"));
    assert!(rt.find("export-output-warning").is_none() && rt.find("export-key-lines").is_none());

    click(&mut rt, "export-tools-output");
    let d = rt.state.export.dialog.as_ref().unwrap();
    assert!(d.markdown().contains("API_KEY=abcdef123456789"));
    assert_eq!(d.key_lines_note().unwrap(), "1 line looks like a key. This is a hint, not a check. Read the file before you share it.");
    assert!(rt.find("export-output-warning").is_some() && rt.find("export-key-lines").is_some());

    click(&mut rt, "export-tools-off");
    assert!(rt.find("export-output-warning").is_none());
    assert!(!rt.state.export.dialog.as_ref().unwrap().markdown().contains("Bash"));
}

// "turns Thinking off, with the reason, when the session has none"
#[test]
fn turns_thinking_off_with_the_reason_when_the_session_has_none() {
    let mut rt = waiting();
    finish(&mut rt, chat(false));
    assert!(!rt.state.export.dialog.as_ref().unwrap().thinking_available());
    click(&mut rt, "export-thinking");
    let d = rt.state.export.dialog.as_ref().unwrap();
    assert!(!d.options.thinking, "a press on the disabled box does nothing");

    let mut again = waiting();
    finish(&mut again, chat(true));
    assert!(again.state.export.dialog.as_ref().unwrap().thinking_available());
    click(&mut again, "export-thinking");
    assert!(again.state.export.dialog.as_ref().unwrap().markdown().contains("> **Thinking**"));
}

// "copies the Markdown on Copy"
#[test]
fn copies_the_markdown_on_copy() {
    let mut rt = waiting();
    finish(&mut rt, chat(false));
    click(&mut rt, "export-copy");
    let d = rt.state.export.dialog.as_ref().unwrap();
    assert!(d.copied_until.is_some(), "the label says Copied");
    assert!(d.markdown().contains("# "));
    // It ends by itself after two seconds. Headless runs never write the system clipboard.
    for _ in 0..(2.3 / FRAME) as usize {
        rt.frame(FRAME);
    }
    assert!(rt.state.export.dialog.as_ref().unwrap().copied_until.is_none());
}

// "saves <title>.md on Save"
#[test]
fn saves_title_md_on_save() {
    let mut rt = waiting();
    finish(&mut rt, chat(false));
    click(&mut rt, "export-save");
    let (kind, options) = rt.file_dialog().expect("Save opens the system's save dialog");
    assert_eq!(kind, FileDialogKind::Save);
    let name = options.file_name.clone().unwrap();
    assert!(name.ends_with(".md"), "{name}");
    let folder = std::env::temp_dir().join(format!("fleet-native-export-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let path = folder.join(&name);
    rt.resolve_file_dialog(Some(path.clone())).unwrap();
    for _ in 0..200 {
        rt.frame(FRAME);
        if path.exists() && rt.state.ui.toast.is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let written = std::fs::read_to_string(&path).unwrap();
    assert_eq!(written, rt.state.export.dialog.as_ref().unwrap().markdown());
    assert!(written.contains("## You"));
    std::fs::remove_dir_all(&folder).unwrap();
}

// "switches to the rendered preview"
#[test]
fn switches_to_the_rendered_preview() {
    let mut rt = waiting();
    finish(&mut rt, chat(false));
    click(&mut rt, "export-pane-preview");
    assert_eq!(rt.state.export.dialog.as_ref().unwrap().pane, Pane::Preview);
    assert!(rt.find("export-source").is_none() && rt.find("export-rendered").is_some());
    click(&mut rt, "export-pane-source");
    assert!(rt.find("export-source").is_some());
}

// ActivityStream.commands.test.ts: "export-conversation opens the dialog for the session"
#[test]
fn the_palette_command_opens_the_dialog_for_the_open_session_and_esc_closes_it() {
    let mut rt = app();
    assert!(rt.state.export.dialog.is_none());
    rt.update(|s, _| s.export_open());
    rt.frame(FRAME);
    let d = rt.state.export.dialog.as_ref().unwrap();
    assert_eq!(d.title, rt.state.session().title);
    assert!(rt.find("export-markdown-dialog").is_some());
    rt.key(rune::KeyEvent { key: rune::Key::Escape, mods: rune::Mods::default() });
    rt.frame(FRAME);
    assert!(rt.state.export.dialog.is_none(), "Esc closes it and stops the load");
}

// An answer for a load that was closed or restarted is dropped.
#[test]
fn an_old_answer_is_dropped() {
    let mut rt = waiting();
    let old = generation(&rt);
    rt.update(|s, _| s.export_close());
    rt.update(move |s, _| s.export_done(old, Ok(chat(false))));
    assert!(rt.state.export.dialog.is_none());
    rt.update(|s, _| s.show_export(Load::Loading(0)));
    rt.update(move |s, _| s.export_done(old, Ok(chat(false))));
    assert_eq!(rt.state.export.dialog.as_ref().unwrap().load, Load::Loading(0));
    rt.update(|s, _| s.export_set_tools(ExportToolMode::Off));
}
