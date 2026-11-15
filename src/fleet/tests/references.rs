//! References in the draft (docs/parity/05-composer.md, piece L): "marks @ references behind the text once they're typed,
//! and sends them as plain text" and "edits a file picked from the @ list as one piece: Backspace takes it out whole, the
//! arrows step over it" (of `Composer.test.ts`), and the requests that carry `sessionReferences` and `skills`.
use super::autocomplete::{Live, Seen, commands, press, typed};
use super::reconnect::app;
use crate::State;
use crate::composer::refs::SessionReference;
use rune::{Key, Runtime};
use serde_json::json;

const FRAME: f64 = 1.0 / 60.0;

fn mock_app() -> Runtime<State> {
    let mut rt = app();
    rt.update(|s, _| s.set_open((0, 2)));
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

fn drawn(rt: &Runtime<State>) -> Vec<String> {
    let text = &rt.state.draft.text;
    rt.state.draft_references().into_iter().map(|r| text[r.range].to_string()).collect()
}

// "marks @ references behind the text once they're typed, and sends them as plain text"
#[test]
fn marks_at_references_behind_the_text_once_theyre_typed_and_sends_them_as_plain_text() {
    let mut rt = mock_app();
    typed(&mut rt, "look at @src/ap");
    assert!(drawn(&rt).is_empty(), "the one the caret is typing stays plain");
    press(&mut rt, Key::Text("p.ts and @docs/ please".into()));
    assert_eq!(drawn(&rt), ["@src/app.ts", "@docs/"]);
    let before = rt.state.session().messages.len();
    press(&mut rt, Key::Enter);
    let sent = rt.state.session().messages.last().unwrap();
    assert_eq!(rt.state.session().messages.len(), before + 1);
    assert!(
        matches!(&sent.parts[..], [crate::view::conversation::Part::Text { text, .. }] if text == "look at @src/app.ts and @docs/ please"),
        "sent as typed"
    );
}

// "edits a file picked from the @ list as one piece: Backspace takes it out whole, the arrows step over it"
#[test]
fn edits_a_file_picked_from_the_at_list_as_one_piece_backspace_takes_it_out_whole_the_arrows_step_over_it() {
    let mut rt = mock_app();
    typed(&mut rt, "look at @guide");
    press(&mut rt, Key::Enter);
    assert_eq!(rt.state.draft.text, "look at @docs/guide.md ");
    let owner = rt.state.draft_owner().unwrap();
    assert_eq!(rt.state.auto.picks_of(&owner).unwrap().files(), ["@docs/guide.md"]);

    // The first Backspace takes the space; the reference stays one, the caret at its end.
    press(&mut rt, Key::Backspace);
    assert_eq!(rt.state.draft.text, "look at @docs/guide.md");
    assert_eq!(drawn(&rt), ["@docs/guide.md"]);
    let end = rt.state.draft.text.len();

    press(&mut rt, Key::Left);
    assert_eq!(rt.state.draft.range(), (8, 8), "the left arrow steps over it");
    press(&mut rt, Key::Right);
    assert_eq!(rt.state.draft.range(), (end, end), "the right arrow steps back over it");

    // A click inside lands on the nearer edge.
    rt.update(|s, _| s.draft.set_caret(11));
    rt.frame(FRAME);
    assert_eq!(rt.state.draft.range(), (8, 8));

    // Backspace after it takes it whole.
    rt.update(move |s, _| s.draft.set_caret(end));
    press(&mut rt, Key::Backspace);
    assert_eq!(rt.state.draft.text, "look at ");
    assert!(rt.state.auto.picks_of(&owner).unwrap().files().is_empty(), "once it's gone, typing the path again is plain typing");
    press(&mut rt, Key::Text("@docs/guide.md ".into()));
    assert!(drawn(&rt).len() == 1, "typed again it is drawn as a typed file once the caret leaves");
    assert!(rt.state.auto.picks_of(&owner).unwrap().files().is_empty());
}

#[test]
fn undo_puts_a_reference_that_went_whole_back() {
    let mut rt = mock_app();
    typed(&mut rt, "see @guide");
    press(&mut rt, Key::Enter);
    press(&mut rt, Key::Backspace);
    press(&mut rt, Key::Backspace);
    assert_eq!(rt.state.draft.text, "see ");
    rt.key(rune::KeyEvent { key: Key::Command('z'), mods: rune::Mods::default() });
    rt.frame(FRAME);
    assert_eq!(rt.state.draft.text, "see @docs/guide.md");
}

#[test]
fn a_shell_command_has_no_references() {
    let mut rt = mock_app();
    rt.update(|s, _| {
        s.projects[0].sessions[2].row.harness_type = Some("opencode".into());
        s.pickers.harnesses =
            vec![crate::composer::status::HarnessInfo { kind: "opencode".into(), supports_shell_commands: true, ..Default::default() }];
    });
    typed(&mut rt, "!cat @src/app.ts now");
    assert!(drawn(&rt).is_empty());
}

fn pick_notes(rt: &mut Runtime<State>, id: &str) {
    let id = id.to_string();
    rt.update(move |s, _| {
        let owner = s.draft_owner().unwrap();
        s.auto.picks_mut(&owner).remember_session(SessionReference {
            token: "@cache-notes".into(),
            session_id: id,
            title: "Cache notes".into(),
        });
    });
}

#[test]
fn a_prompt_carries_the_sessions_still_in_the_text_and_the_skills_it_names() {
    let mut live = Live::start(Seen { commands: commands(), ..Default::default() });
    let id = live.id.clone();
    live.wait(&|l| l.rt.state.finder.commands.contains_key(&id), "the commands to load");
    live.idle();
    pick_notes(&mut live.rt, "other-session");
    typed(&mut live.rt, "use $tidy-docs with @cache-notes and $nope now");
    assert_eq!(drawn(&live.rt), ["$tidy-docs", "@cache-notes"]);
    press(&mut live.rt, Key::Enter);
    live.wait(&|l| l.calls().iter().any(|c| c.1.ends_with("/prompt")), "the prompt");
    let calls = live.calls();
    let body = &calls.iter().find(|c| c.1.ends_with("/prompt")).unwrap().2;
    assert_eq!(body["text"], "use $tidy-docs with @cache-notes and $nope now");
    assert_eq!(body["sessionReferences"], json!([{"token": "@cache-notes", "sessionId": "other-session"}]));
    assert_eq!(body["skills"], json!(["tidy-docs"]));
}

#[test]
fn a_session_whose_token_left_the_text_isnt_sent() {
    let mut live = Live::start(Seen { commands: commands(), ..Default::default() });
    live.idle();
    pick_notes(&mut live.rt, "other-session");
    typed(&mut live.rt, "plain words");
    press(&mut live.rt, Key::Enter);
    live.wait(&|l| l.calls().iter().any(|c| c.1.ends_with("/prompt")), "the prompt");
    let body = live.calls().iter().find(|c| c.1.ends_with("/prompt")).unwrap().2.clone();
    assert!(body.get("sessionReferences").is_none() && body.get("skills").is_none(), "{body}");
}

#[test]
fn a_queued_message_carries_them_too() {
    let mut live = Live::start(Seen { commands: commands(), ..Default::default() });
    let id = live.id.clone();
    live.wait(&|l| l.rt.state.finder.commands.contains_key(&id), "the commands to load");
    live.rt.update(|s, _| s.projects[0].sessions[0].status = crate::mock::Status::Working);
    pick_notes(&mut live.rt, "other-session");
    typed(&mut live.rt, "after that, $tidy-docs and @cache-notes please");
    press(&mut live.rt, Key::Enter);
    live.wait(&|l| l.calls().iter().any(|c| c.1.ends_with("/queue")), "the queue request");
    let body = live.calls().iter().find(|c| c.1.ends_with("/queue")).unwrap().2.clone();
    assert_eq!(body["kind"], "prompt");
    assert_eq!(body["sessionReferences"], json!([{"token": "@cache-notes", "sessionId": "other-session"}]));
    assert_eq!(body["skills"], json!(["tidy-docs"]));
}
