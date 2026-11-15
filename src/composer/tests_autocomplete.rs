//! Fleet's tests for the autocomplete logic, under their names, with the `describe` in the comment: `use-autocomplete.test.ts`
//! (19), `session-references.test.ts` (11), `skill-mentions.test.ts` (8), `file-matches.test.ts` (3), and what native adds.
//!
//! Fleet's own tests mount the composable against mocked requests and fake timers. Here the lists are plain data
//! (`Lists`) that a small fake finder fills the way the real one will (piece K): what the 80 ms wait and the requests
//! themselves do is K's to test, so "asks again" and "waits 80 ms" are not here.
use super::files::*;
use super::refs::*;
use super::triggers::*;

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

const HOUR: f64 = 60.0 * 60_000.0;
const NOW: f64 = 1_790_000_000_000.0;

/// The popup with a fake finder under it: the text, the caret, the lists and the picks.
struct Rig {
    text: String,
    caret: usize,
    lists: Lists,
    picks: Picks,
    ac: Autocomplete,
    /// The last answer to a file search, which `type_text` narrows while the next one is out.
    answer: Vec<String>,
}

macro_rules! ctx {
    ($rig:expr) => {
        Ctx { text: &$rig.text, caret: $rig.caret, lists: &$rig.lists, picks: &mut $rig.picks }
    };
}

fn command(name: &str, description: &str) -> Command {
    Command { name: name.into(), description: Some(description.into()), kind: None, user_only: false }
}

fn rig(text: &str, caret: usize) -> Rig {
    let mut lists = Lists { session_id: Some("instance-1".into()), draft_key: Some("instance-1".into()), ..Lists::default() };
    lists.set_commands(vec![command("help", "Show help"), command("hello", "Say hello"), command("status", "Show status")]);
    lists.agents = vec![
        Agent { name: "alpha".into(), description: Some("Planner".into()), mode: Some("primary".into()), color: Some("#ff00aa".into()) },
        Agent { name: "beta".into(), description: Some("Reviewer".into()), mode: Some("secondary".into()), color: Some("#00aaff".into()) },
    ];
    lists.harness_names.insert("claude-code".into(), "Claude Code".into());
    let mut rig = Rig { text: text.into(), caret, lists, picks: Picks::default(), ac: Autocomplete::new(), answer: vec![] };
    // The server's answer, whatever was asked (as Fleet's mock gives it).
    rig.serve(&["src/alpha.ts", "src/components/"]);
    rig
}

impl Rig {
    /// The server answered the last search.
    fn serve(&mut self, files: &[&str]) {
        self.answer = strings(files);
        self.lists.files = self.answer.clone();
        self.lists.files_loading = false;
    }

    /// Typing: the text and caret change, and until the server answers the last answer is narrowed to the new query.
    fn type_text(&mut self, text: &str) {
        self.text = text.into();
        self.caret = text.len();
        let query = self.ac.file_query(&ctx!(self)).map(str::to_string);
        if let Some(query) = query {
            self.lists.files = narrow_file_matches(&self.answer, &query);
            self.lists.files_loading = true;
        }
    }

    fn key(&mut self, key: Key) -> KeyOutcome {
        let out = self.ac.on_key(&mut ctx!(self), key);
        self.apply(out.edit.clone());
        out
    }

    fn select(&mut self, value: &str) -> Option<Edit> {
        let edit = self.ac.on_select(&mut ctx!(self), value);
        self.apply(edit.clone());
        edit
    }

    fn poll(&mut self) {
        let edit = self.ac.poll(&mut ctx!(self));
        self.apply(edit);
    }

    fn apply(&mut self, edit: Option<Edit>) {
        if let Some(edit) = edit {
            self.text = edit.text;
            self.caret = edit.caret;
        }
    }

    fn labels(&self) -> Vec<String> {
        self.ac
            .items(&Ctx { text: &self.text, caret: self.caret, lists: &self.lists, picks: &mut self.picks.clone() })
            .into_iter()
            .map(|i| i.label)
            .collect()
    }

    fn items(&self) -> Vec<Item> {
        self.ac.items(&Ctx { text: &self.text, caret: self.caret, lists: &self.lists, picks: &mut self.picks.clone() })
    }

    fn open(&self) -> bool {
        self.ac.is_open(&Ctx { text: &self.text, caret: self.caret, lists: &self.lists, picks: &mut self.picks.clone() })
    }
}

// ── use-autocomplete.test.ts ─────────────────────────────────────────────

// useAutocomplete: "shows slash commands, filters them, and replaces the input on Enter"
#[test]
fn shows_slash_commands_filters_them_and_replaces_the_input_on_enter() {
    let mut r = rig("/", 1);
    assert!(r.open());
    assert_eq!(r.labels(), ["/help", "/hello", "/status"]);
    r.type_text("/he");
    assert_eq!(r.labels(), ["/help", "/hello"]);
    let out = r.key(Key::Enter);
    assert!(out.consumed && out.swallowed);
    assert_eq!(r.text, "/help ");
    assert_eq!(r.caret, 6);
}

// useAutocomplete: "asks for the session's commands and agents again when its harness says they changed"
// The request is K's; the logic is that a new list replaces the old one in the popup, which stays up meanwhile.
#[test]
fn asks_for_the_sessions_commands_and_agents_again_when_its_harness_says_they_changed() {
    let mut r = rig("/", 1);
    assert_eq!(r.labels(), ["/help", "/hello", "/status"]);
    // The reload is under way: no loading flag, as in Fleet ("the old one stays up meanwhile"), so the list stays.
    assert!(!r.lists.commands_loading);
    assert_eq!(r.labels(), ["/help", "/hello", "/status"]);
    r.lists.set_commands(vec![command("help", "Show help"), command("deploy", "Ship it")]);
    assert_eq!(r.labels(), ["/help", "/deploy"]);
}

// useAutocomplete: "shows mention suggestions after whitespace and debounces server-side file search"
// (the 80 ms wait is K's)
#[test]
fn shows_mention_suggestions_after_whitespace_and_debounces_server_side_file_search() {
    let mut r = rig("hello @", 7);
    r.lists.files = vec![];
    assert!(r.open());
    assert!(r.labels().contains(&"@alpha".to_string()));
    assert!(r.labels().contains(&"@beta".to_string()));
    r.type_text("hello @al");
    assert_eq!(r.ac.file_query(&ctx!(r)), Some("al"));
    r.serve(&["src/alpha.ts", "src/components/"]);
    assert_eq!(r.labels(), ["alpha.ts", "components/", "@alpha"]);
    let descriptions: Vec<_> = r.items().into_iter().map(|i| i.description.unwrap()).collect();
    assert_eq!(descriptions, ["src/", "src/", "Planner"]);
    r.type_text("hello@al");
    assert!(!r.open());
}

// useAutocomplete: "lists the session folder as soon as @ is typed, files and folders before agents"
#[test]
fn lists_the_session_folder_as_soon_as_at_is_typed_files_and_folders_before_agents() {
    let mut r = rig("look at @", 9);
    assert_eq!(r.ac.file_query(&ctx!(r)), Some(""));
    let groups: Vec<_> = r.items().into_iter().map(|i| i.group).collect();
    assert_eq!(groups, [Group::File, Group::File, Group::Agent, Group::Agent]);
}

// useAutocomplete: "opens a folder on Tab and references it on Enter"
#[test]
fn opens_a_folder_on_tab_and_references_it_on_enter() {
    let mut r = rig("look at @comp", 13);
    r.key(Key::Down);
    assert_eq!(r.ac.selected_value(&ctx!(r)).as_deref(), Some("@src/components/ "));
    let tab = r.key(Key::Tab);
    assert!(tab.consumed);
    assert_eq!(r.text, "look at @src/components/");
    // Opened to look inside, not picked.
    assert!(r.picks.files().is_empty());
    assert_eq!(r.caret, r.text.len());
    assert!(r.open());
    assert_eq!(r.ac.file_query(&ctx!(r)), Some("src/components/"));
    r.key(Key::Down);
    r.key(Key::Enter);
    assert_eq!(r.text, "look at @src/components/ ");
    assert!(!r.open());
    assert_eq!(r.picks.files(), ["@src/components/"]);
    // Backspace over the space after it: the reference is done, so the list stays shut.
    r.type_text("look at @src/components/");
    assert!(!r.open());
}

// useAutocomplete: "holds Enter pressed before anything matches, and picks the first match when the answer comes"
#[test]
fn holds_enter_pressed_before_anything_matches_and_picks_the_first_match_when_the_answer_comes() {
    let mut r = rig("look at @", 9);
    r.type_text("look at @zeta");
    assert!(r.items().is_empty());
    let enter = r.key(Key::Enter);
    assert!(enter.consumed);
    assert_eq!(r.text, "look at @zeta");
    assert!(r.ac.has_pending());
    // The answer comes (the same two files, whatever was asked).
    r.serve(&["src/alpha.ts", "src/components/"]);
    r.poll();
    assert_eq!(r.text, "look at @src/alpha.ts ");
}

// useAutocomplete: "narrows the last answer as you type, so the list and Enter follow each key before the server answers"
#[test]
fn narrows_the_last_answer_as_you_type_so_the_list_and_enter_follow_each_key_before_the_server_answers() {
    let mut r = rig("look at @", 9);
    let files: Vec<_> = r.items().into_iter().filter(|i| i.group == Group::File).map(|i| i.label).collect();
    assert_eq!(files, ["alpha.ts", "components/"]);
    r.type_text("look at @comp");
    assert!(r.ac.is_loading(&ctx!(r)));
    assert_eq!(r.labels(), ["components/"]);
    r.key(Key::Enter);
    assert_eq!(r.text, "look at @src/components/ ");
    assert_eq!(r.picks.files(), ["@src/components/"]);
}

// useAutocomplete: "doesn't search files while typing a slash command"
#[test]
fn doesnt_search_files_while_typing_a_slash_command() {
    let mut r = rig("/he", 3);
    assert_eq!(r.ac.file_query(&ctx!(r)), None);
}

// useAutocomplete: "wraps arrow navigation, selects on Tab, and reopens after Escape when typing resumes"
#[test]
fn wraps_arrow_navigation_selects_on_tab_and_reopens_after_escape_when_typing_resumes() {
    let mut r = rig("/", 1);
    r.key(Key::Up);
    assert_eq!(r.ac.selected_index(&ctx!(r)), 2);
    assert_eq!(r.ac.selected_value(&ctx!(r)).as_deref(), Some("/status "));
    r.key(Key::Down);
    assert_eq!(r.ac.selected_index(&ctx!(r)), 0);
    assert_eq!(r.ac.selected_value(&ctx!(r)).as_deref(), Some("/help "));
    r.key(Key::Down);
    assert_eq!(r.ac.selected_index(&ctx!(r)), 1);
    assert_eq!(r.ac.selected_value(&ctx!(r)).as_deref(), Some("/hello "));
    let esc = r.key(Key::Escape);
    assert!(esc.consumed && esc.stop);
    assert!(!r.open());
    r.type_text("/h");
    assert!(r.open());
    let tab = r.key(Key::Tab);
    assert!(tab.consumed);
    assert_eq!(r.text, "/help ");
}

// useAutocomplete: "reloads session-scoped suggestions when the session id changes"
// The fetch is K's. The logic: another session has its own commands, agents and skills, and none until they load.
#[test]
fn reloads_session_scoped_suggestions_when_the_session_id_changes() {
    let mut r = rig("/", 1);
    assert_eq!(r.labels().len(), 3);
    r.lists.session_id = Some("instance-2".into());
    r.lists.set_commands(vec![]);
    r.lists.commands_loading = true;
    r.lists.agents = vec![];
    r.lists.agents_loading = true;
    assert!(r.open());
    assert!(r.items().is_empty());
    assert!(r.ac.is_loading(&ctx!(r)));
    r.lists.set_commands(vec![command("other", "Another session's")]);
    r.lists.commands_loading = false;
    assert_eq!(r.labels(), ["/other"]);
}

fn item(id: &str, title: &str, hours_ago: f64) -> SessionItem {
    SessionItem {
        id: id.into(),
        title: title.into(),
        status: "idle".into(),
        harness_type: "claude-code".into(),
        workspace_directory: "/work/tidytempo".into(),
        updated: NOW - hours_ago * HOUR,
        ..SessionItem::default()
    }
}

fn sessions() -> Vec<SessionItem> {
    vec![
        item("instance-1", "t3code in this session", 0.0),
        item("ses-a", "t3code: what can we learn?", 1.0),
        item("ses-b", "Survey t3code features", 2.0),
        item("ses-c", "Fleet MCP server spike", 3.0),
        item("ses-d", "Onboarding time zone fixes", 4.0),
    ]
}

// useAutocomplete > sessions: "offers matching sessions before files, never the session being written to"
#[test]
fn offers_matching_sessions_before_files_never_the_session_being_written_to() {
    let mut r = rig("Use @t3c", 8);
    r.lists.sessions = sessions();
    r.serve(&["src/alpha.ts", "src/components/"]);
    let groups: Vec<_> = r.items().into_iter().map(|i| i.group).collect();
    assert_eq!(groups, [Group::Session, Group::Session, Group::File, Group::File]);
    let first: Vec<_> = r.items().into_iter().take(2).map(|i| (i.label, i.description.unwrap())).collect();
    assert_eq!(
        first,
        [
            ("t3code: what can we learn?".to_string(), "tidytempo · Claude Code".to_string()),
            ("Survey t3code features".to_string(), "tidytempo · Claude Code".to_string()),
        ]
    );
    assert_eq!(r.ac.query(&ctx!(r)), "t3c");
    assert!(r.ac.is_mention_open(&ctx!(r)));
}

// useAutocomplete > sessions: "offers only the three newest for a bare @, which is mostly for files"
#[test]
fn offers_only_the_three_newest_for_a_bare_at_which_is_mostly_for_files() {
    let mut r = rig("look at @", 9);
    r.lists.sessions = sessions();
    let ids: Vec<_> = r.items().into_iter().filter(|i| i.group == Group::Session).map(|i| i.id).collect();
    assert_eq!(ids, ["session:ses-a", "session:ses-b", "session:ses-c"]);
}

// useAutocomplete > sessions: "attaches a session on Tab as a token from its title, which goes with the message"
#[test]
fn attaches_a_session_on_tab_as_a_token_from_its_title_which_goes_with_the_message() {
    let mut r = rig("Use the mapping from @t3c", 25);
    r.lists.sessions = sessions();
    let tab = r.key(Key::Tab);
    assert!(tab.consumed);
    assert_eq!(r.text, "Use the mapping from @t3code-what-can-we-learn ");
    assert!(!r.open());
    assert_eq!(
        r.picks.session_references_in(&r.text),
        [SessionReference {
            token: "@t3code-what-can-we-learn".into(),
            session_id: "ses-a".into(),
            title: "t3code: what can we learn?".into()
        }]
    );
}

fn new_session(text: &str) -> Rig {
    let mut lists = Lists { session_id: None, draft_key: Some("new-session-draft-1".into()), sessions: sessions(), ..Lists::default() };
    lists.files = strings(&["src/billing/retry-policy.ts", "docs/billing/"]);
    Rig {
        text: text.into(),
        caret: text.len(),
        lists,
        picks: Picks::default(),
        ac: Autocomplete::new(),
        answer: strings(&["src/billing/retry-policy.ts", "docs/billing/"]),
    }
}

// useAutocomplete > sessions > before the session exists: "offers sessions, then the files in the folder, and nothing that needs a session"
#[test]
fn before_the_session_exists_offers_sessions_then_the_files_in_the_folder_and_nothing_that_needs_a_session() {
    let r = new_session("Pick up where @");
    let groups: Vec<_> = r.items().into_iter().map(|i| i.group).collect();
    assert_eq!(groups, [Group::Session, Group::Session, Group::Session, Group::File, Group::File]);
    let ids: Vec<_> = r.items().into_iter().filter(|i| i.group == Group::Session).map(|i| i.id).collect();
    assert_eq!(ids, ["session:instance-1", "session:ses-a", "session:ses-b"]);
    // No agents and no commands to ask for: the lists hold none and `skills()` is empty without a session.
    assert!(r.lists.agents.is_empty() && r.lists.commands.is_empty() && r.lists.skills().is_empty());
}

// useAutocomplete > sessions > before the session exists: "opens no / list"
#[test]
fn before_the_session_exists_opens_no_slash_list() {
    let r = new_session("/hel");
    assert!(!r.open());
}

// useAutocomplete > sessions > before the session exists: "keeps its picks under the draft's key"
#[test]
fn before_the_session_exists_keeps_its_picks_under_the_drafts_key() {
    let mut r = new_session("Finish @retry");
    r.select("@src/billing/retry-policy.ts ");
    r.type_text(&format!("{}after @Survey", r.text));
    r.select("session:ses-b");
    assert_eq!(r.text, "Finish @src/billing/retry-policy.ts after @survey-t3code-features ");
    assert_eq!(r.picks.files(), ["@src/billing/retry-policy.ts"]);
    assert_eq!(
        r.picks.session_references_in(&r.text),
        [SessionReference { token: "@survey-t3code-features".into(), session_id: "ses-b".into(), title: "Survey t3code features".into() }]
    );
}

fn skill_rig(text: &str, harness: Option<&str>) -> Rig {
    let mut r = rig(text, text.len());
    let skill = |name: &str, description: &str, user_only: bool, kind: &str| Command {
        name: name.into(),
        description: Some(description.into()),
        kind: Some(kind.into()),
        user_only,
    };
    r.lists.set_commands(vec![
        skill("fleet-debug", "Find the root cause", false, "skill"),
        skill("fleet-simplify", "Tidy the change", false, "skill"),
        skill("release-notes", "Draft release notes", true, "skill"),
        skill("review", "A command", false, "command"),
    ]);
    r.lists.harness_type = harness.map(String::from);
    r
}

// useAutocomplete > $ skills: "lists the session's skills after a $ anywhere, not its commands, and puts the pick in as $name"
#[test]
fn lists_the_sessions_skills_after_a_dollar_anywhere_not_its_commands_and_puts_the_pick_in_as_name() {
    let mut r = skill_rig("Fix it, then use $sim", None);
    assert!(r.ac.is_skill_open(&ctx!(r)));
    assert_eq!(r.labels(), ["fleet-simplify"]);
    r.type_text("Fix it, then use $");
    assert_eq!(r.labels(), ["fleet-debug", "fleet-simplify", "release-notes"]);
    r.key(Key::Enter);
    assert_eq!(r.text, "Fix it, then use $fleet-debug ");
}

// useAutocomplete > $ skills: "opens after a /command at the start too, and not inside a word or backticks"
#[test]
fn opens_after_a_command_at_the_start_too_and_not_inside_a_word_or_backticks() {
    let mut r = skill_rig("/fleet-review then $fl", None);
    assert!(r.ac.is_skill_open(&ctx!(r)));
    for text in ["costs US$5", "run `echo $HO"] {
        r.type_text(text);
        assert!(!r.ac.is_skill_open(&ctx!(r)), "{text}");
    }
}

// useAutocomplete > $ skills: "on Claude Code, warns that a second skill goes as a note and greys out one the model can't load"
#[test]
fn on_claude_code_warns_that_a_second_skill_goes_as_a_note_and_greys_out_one_the_model_cant_load() {
    let mut r = skill_rig("Use $fleet-debug then $", Some("claude-code"));
    let items = r.items();
    let by = |name: &str| items.iter().find(|i| i.label == name).unwrap().clone();
    assert_eq!(by("fleet-debug").warning, None);
    assert_eq!(
        by("fleet-simplify").warning.as_deref(),
        Some("This message already runs fleet-debug. Claude Code runs one skill per message, so this one goes as a note.")
    );
    assert_eq!(
        by("release-notes").disabled_reason.as_deref(),
        Some("The model can't load release-notes, so it only runs first in a message, and this one already runs fleet-debug.")
    );
    // Enter on the greyed-out row neither picks it nor sends the message.
    r.key(Key::Down);
    r.key(Key::Down);
    let enter = r.key(Key::Enter);
    assert!(enter.consumed && enter.swallowed && enter.edit.is_none());
    assert_eq!(r.text, "Use $fleet-debug then $");
}

// ── session-references.test.ts ───────────────────────────────────────────

// session reference tokens: "makes a token from the title"
#[test]
fn makes_a_token_from_the_title() {
    assert_eq!(session_reference_slug("t3code: what can we learn?"), "t3code-what-can-we-learn");
    assert_eq!(session_reference_slug("Port t3code's limit recovery"), "port-t3code-s-limit-recovery");
    assert_eq!(session_reference_slug("Café résumé"), "cafe-resume");
    assert_eq!(session_reference_slug("???"), "session");
    assert!(session_reference_slug("a very long title that keeps going well past forty characters").chars().count() <= 40);
}

// session reference tokens: "gives a second session with the same title its own token, and a session picked again its old one"
#[test]
fn gives_a_second_session_with_the_same_title_its_own_token_and_a_session_picked_again_its_old_one() {
    let mut picks = Picks::default();
    let first = picks.session_token("s1", "Notes");
    picks.remember_session(SessionReference { token: first.clone(), session_id: "s1".into(), title: "Notes".into() });
    assert_eq!(first, "@notes");
    assert_eq!(picks.session_token("s2", "Notes"), "@notes-2");
    assert_eq!(picks.session_token("s1", "Notes"), "@notes");
}

fn reference(token: &str, id: &str, title: &str) -> SessionReference {
    SessionReference { token: token.into(), session_id: id.into(), title: title.into() }
}

// session reference tokens: "sends only the picks still in the text, in the order they appear"
#[test]
fn sends_only_the_picks_still_in_the_text_in_the_order_they_appear() {
    let mut picks = Picks::default();
    picks.remember_session(reference("@notes", "s1", "Notes"));
    picks.remember_session(reference("@plan", "s2", "Plan"));
    picks.remember_session(reference("@gone", "s3", "Gone"));
    let ids: Vec<_> = picks.session_references_in("Use @plan, then @notes.").into_iter().map(|r| r.session_id).collect();
    assert_eq!(ids, ["s2", "s1"]);
    // Another draft has its own picks.
    assert!(Picks::default().session_references_in("Use @plan").is_empty());
}

// session reference tokens: "keeps the picks with the draft across a reload"
// (the file is piece L's; what is kept is `sessions()` and `from_parts`)
#[test]
fn keeps_the_picks_with_the_draft_across_a_reload() {
    let mut picks = Picks::default();
    picks.remember_session(reference("@notes", "s1", "Notes"));
    let kept = Picks::from_parts(picks.files().to_vec(), picks.sessions().to_vec());
    assert_eq!(kept.session_references_in("see @notes"), [reference("@notes", "s1", "Notes")]);
    assert!(kept.remembered_session_tokens().contains(&"@notes"));
}

// session reference tokens: "finds a token only where it stands on its own"
#[test]
fn finds_a_token_only_where_it_stands_on_its_own() {
    assert!(text_has_token("from @notes.", "@notes"));
    assert!(text_has_token("(@notes)", "@notes"));
    assert!(!text_has_token("@notes-2", "@notes"));
    assert!(!text_has_token("@notes.md", "@notes"));
    assert!(!text_has_token("me@notes", "@notes"));
}

const SENT: &str = "Use the mapping from @t3code-what-can-we-learn\n\n<fleet-session-references>\nFleet sessions the user referenced with @ in the message above. They are context…\n<session ref=\"@t3code-what-can-we-learn\" id=\"ses-1\" title=\"t3code: what &amp; how?\" />\n<session ref=\"@notes\" id=\"ses-2\" title=\"Notes\">\nIt &lt;mapped&gt; subagents.\n</session>\n</fleet-session-references>";

// the block in a sent message: "splits what was typed from the sessions the block names"
#[test]
fn splits_what_was_typed_from_the_sessions_the_block_names() {
    let (text, references) = parse_session_references(SENT);
    assert_eq!(text, "Use the mapping from @t3code-what-can-we-learn");
    assert_eq!(references, [reference("@t3code-what-can-we-learn", "ses-1", "t3code: what & how?"), reference("@notes", "ses-2", "Notes")]);
}

// the block in a sent message: "leaves a message without a block as it is"
#[test]
fn leaves_a_message_without_a_block_as_it_is() {
    let body = "Hello <fleet-session-references> in prose";
    assert_eq!(parse_session_references(body), (body.to_string(), vec![]));
}

fn listed(id: &str, title: &str, hours_ago: f64) -> SessionItem {
    SessionItem {
        id: id.into(),
        title: title.into(),
        status: "idle".into(),
        harness_type: "opencode2".into(),
        workspace_directory: "/work/weave-fleet".into(),
        updated: NOW - hours_ago * HOUR,
        ..SessionItem::default()
    }
}

fn the_list() -> Vec<SessionItem> {
    vec![
        SessionItem { archived: true, ..listed("old", "Port t3code's limit recovery", 72.0) },
        SessionItem {
            harness_type: "claude-code".into(),
            project_name: Some("tidytempo".into()),
            ..listed("parent", "t3code: what can we learn?", 1.0)
        },
        SessionItem { parent_session_id: Some("parent".into()), ..listed("child", "Survey t3code features", 1.0) },
        listed("here", "Capture subagents with t3code", 1.0),
        listed("other", "Fleet MCP server spike", 1.0),
    ]
}

// the sessions the @ list offers: "ranks titles that start with the text, then words that do, then the rest; newest first within each"
#[test]
fn ranks_titles_that_start_with_the_text_then_words_that_do_then_the_rest_newest_first_within_each() {
    let names = |kind: &str| match kind {
        "claude-code" => Some("Claude Code".to_string()),
        "opencode2" => Some("OpenCode 2".to_string()),
        _ => None,
    };
    let offered = match_referable_sessions(&the_list(), "t3c", Some("here"), 5, &names);
    let ids: Vec<_> = offered.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["parent", "child", "old"]);
    let descriptions: Vec<_> = offered.iter().map(|s| s.description.as_str()).collect();
    assert_eq!(descriptions, ["tidytempo · Claude Code", "subagent of t3code: what can we learn?", "weave-fleet · OpenCode 2 · archived"]);
}

// the sessions the @ list offers: "offers the newest for a bare @, never the session being written to"
#[test]
fn offers_the_newest_for_a_bare_at_never_the_session_being_written_to() {
    let offered = match_referable_sessions(&the_list(), "", Some("here"), 3, &|_| None);
    assert_eq!(offered.len(), 3);
    assert!(!offered.iter().any(|s| s.id == "here"));
}

// the sessions the @ list offers: "names a harness the way the caller does"
#[test]
fn names_a_harness_the_way_the_caller_does() {
    let offered = match_referable_sessions(&the_list(), "fleet", None, 5, &|kind| Some(format!("<{kind}>")));
    assert_eq!(offered[0].description, "weave-fleet · <opencode2>");
}

// the sessions the @ list offers: "names a harness the caller doesn't know by its type, without guessing a name"
#[test]
fn names_a_harness_the_caller_doesnt_know_by_its_type_without_guessing_a_name() {
    let offered = match_referable_sessions(&the_list(), "fleet", None, 5, &|_| None);
    assert_eq!(offered[0].description, "weave-fleet · opencode2");
}

// ── skill-mentions.test.ts ───────────────────────────────────────────────

const SKILLS: [&str; 2] = ["fleet-debug", "fleet-simplify"];

// skillTrigger: "opens on a $ at the start of a word, anywhere in the draft"
#[test]
fn skill_trigger_opens_on_a_dollar_at_the_start_of_a_word_anywhere_in_the_draft() {
    assert_eq!(skill_trigger("$", 1), Some((0, String::new())));
    assert_eq!(skill_trigger("Then tidy it with $fleet-s", 26), Some((18, "fleet-s".into())));
}

// skillTrigger: "doesn't open inside a word, after a space, or inside backticks"
#[test]
fn skill_trigger_doesnt_open_inside_a_word_after_a_space_or_inside_backticks() {
    assert_eq!(skill_trigger("US$5", 4), None);
    assert_eq!(skill_trigger("$fleet-debug now", 16), None);
    assert_eq!(skill_trigger("run `echo $HOME", 15), None);
    assert_eq!(skill_trigger("run `echo` $fl", 14), Some((11, "fl".into())));
}

// skillNamesIn: "finds the session's skills once each, in order, with sentence punctuation after them"
#[test]
fn skill_names_in_finds_the_sessions_skills_once_each_in_order_with_sentence_punctuation_after_them() {
    assert_eq!(
        skill_names_in("Use $fleet-simplify, then $fleet-debug. Again: $fleet-simplify", &SKILLS),
        ["fleet-simplify", "fleet-debug"]
    );
}

// skillNamesIn: "leaves unknown names, longer tokens and code alone"
#[test]
fn skill_names_in_leaves_unknown_names_longer_tokens_and_code_alone() {
    assert!(skill_names_in("Why is $HOME empty? $5, $fleet-debug-2, `$fleet-debug`, a$fleet-debug", &SKILLS).is_empty());
}

// skillDeliveries: "attaches every skill on OpenCode 2, only the first on Claude Code, and notes them elsewhere"
#[test]
fn skill_deliveries_attaches_every_skill_on_opencode_2_only_the_first_on_claude_code_and_notes_them_elsewhere() {
    let names = strings(&SKILLS);
    let of = |harness: &str| skill_deliveries(Some(harness), &names).into_iter().map(|s| s.delivery).collect::<Vec<_>>();
    assert_eq!(of("opencode2"), [SkillDelivery::Attached, SkillDelivery::Attached]);
    assert_eq!(of("claude-code"), [SkillDelivery::Attached, SkillDelivery::Note]);
    assert_eq!(of("opencode"), [SkillDelivery::Note, SkillDelivery::Note]);
}

// session skills: "keeps the skills among a session's commands, and those the harness can't tell apart"
#[test]
fn session_skills_keeps_the_skills_among_a_sessions_commands_and_those_the_harness_cant_tell_apart() {
    let kind = |name: &str, kind: Option<&str>, user_only: bool| Command {
        name: name.into(),
        description: None,
        kind: kind.map(String::from),
        user_only,
    };
    let skills =
        skills_among(&[kind("fleet-debug", Some("skill"), false), kind("review", Some("command"), false), kind("deploy", None, true)]);
    assert_eq!(skills.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["fleet-debug", "deploy"]);
    assert_eq!(skills[1], SessionSkill { name: "deploy".into(), description: None, user_only: true });
    // A session with no commands loaded has none.
    let mut lists = Lists { session_id: Some("s1".into()), ..Lists::default() };
    lists.set_commands(vec![kind("fleet-debug", Some("skill"), false)]);
    assert_eq!(lists.skills().len(), 1);
    lists.session_id = None;
    assert!(lists.skills().is_empty());
}

// ── file-matches.test.ts ─────────────────────────────────────────────────

fn entries() -> Vec<String> {
    strings(&["src/", "src/components/", "src/components/Composer.vue", "src/app.ts", "docs/composer.md", "README.md"])
}

// narrowFileMatches: "ranks like the server: name is the query, starts with it, contains it, then the path, then fuzzy"
#[test]
fn narrow_file_matches_ranks_like_the_server_name_is_the_query_starts_with_it_contains_it_then_the_path_then_fuzzy() {
    let e = entries();
    assert_eq!(narrow_file_matches(&e, "composer"), ["docs/composer.md", "src/components/Composer.vue"]);
    assert_eq!(narrow_file_matches(&e, "comp"), ["src/components/", "docs/composer.md", "src/components/Composer.vue"]);
    assert_eq!(narrow_file_matches(&e, "cmpsr"), ["docs/composer.md", "src/components/Composer.vue"]);
    assert!(narrow_file_matches(&e, "xyz").is_empty());
}

// narrowFileMatches: "matches the whole path once the query has a slash"
#[test]
fn narrow_file_matches_matches_the_whole_path_once_the_query_has_a_slash() {
    assert_eq!(narrow_file_matches(&entries(), "src/comp"), ["src/components/", "src/components/Composer.vue"]);
}

// narrowFileMatches: "lists a folder's children, folders first, for an empty query or one ending in /"
#[test]
fn narrow_file_matches_lists_a_folders_children_folders_first_for_an_empty_query_or_one_ending_in_slash() {
    assert_eq!(narrow_file_matches(&entries(), ""), ["src/", "README.md"]);
    assert_eq!(narrow_file_matches(&entries(), "src/"), ["src/components/", "src/app.ts"]);
}

// ── What native adds ─────────────────────────────────────────────────────

// native: the Latin letters under accents fold the way NFKD does for what titles have; the table has one letter per code point
#[test]
fn the_slug_folds_accented_latin_letters() {
    assert_eq!(session_reference_slug("Ångström Œuvre Łódź Naïve Ñandú"), "angstrom-œuvre-łodz-naive-nandu");
    assert_eq!(session_reference_slug("ZŽžŹ ūŭů"), "zzzz-uuu");
    // Already decomposed: the combining mark goes.
    assert_eq!(session_reference_slug("Cafe\u{301}"), "cafe");
}

// native: a title with one word longer than 40 characters is cut to 40
#[test]
fn a_single_long_word_is_cut_to_forty_characters() {
    assert_eq!(session_reference_slug(&"x".repeat(60)).len(), 40);
}

// native: the file row: the folder keeps its slash, a long parent is cut from the left to its last 47 characters after "…"
#[test]
fn file_rows_label_and_describe_the_path() {
    assert_eq!(file_row("src/components/"), FileRow { label: "components/".into(), description: Some("src/".into()), is_directory: true });
    assert_eq!(file_row("README.md"), FileRow { label: "README.md".into(), description: None, is_directory: false });
    let long = format!("{}/x.ts", "a".repeat(60));
    let row = file_row(&long);
    let description = row.description.unwrap();
    assert_eq!(description.chars().count(), 48);
    assert!(description.starts_with('…'));
}

// native: Enter with no item and no load running does nothing and isn't held
#[test]
fn enter_with_nothing_to_pick_and_no_load_does_nothing() {
    let mut r = rig("look at @zeta", 13);
    r.lists.files = vec![];
    r.lists.files_loading = false;
    let out = r.key(Key::Enter);
    assert!(!out.consumed && out.edit.is_none());
    assert!(!r.ac.has_pending());
    // The popup is open, so the key still doesn't reach the composer.
    assert!(out.swallowed);
}

// native: "costs $5": a `$` list with no items that isn't loading counts as closed, so Enter reaches the composer
#[test]
fn a_dollar_list_with_no_match_that_isnt_loading_lets_enter_send() {
    let mut r = skill_rig("it costs $5", None);
    assert!(r.ac.is_skill_open(&ctx!(r)));
    let out = r.key(Key::Enter);
    assert!(!out.swallowed && !out.consumed);
    // Loading: it counts as open, and Enter is held.
    r.lists.commands_loading = true;
    let out = r.key(Key::Enter);
    assert!(out.swallowed && out.consumed);
    assert!(r.ac.has_pending());
}

// native: a held Enter is dropped when the draft changes before the answer
#[test]
fn a_held_enter_is_dropped_when_the_draft_changes() {
    let mut r = rig("look at @", 9);
    r.type_text("look at @zeta");
    r.key(Key::Enter);
    assert!(r.ac.has_pending());
    r.type_text("look at @zetax");
    r.serve(&["src/alpha.ts"]);
    r.poll();
    assert!(!r.ac.has_pending());
    assert_eq!(r.text, "look at @zetax");
}

// native: Esc keeps the popup closed at that text only, and another key at the same text keeps it closed
#[test]
fn escape_keeps_the_popup_closed_until_the_text_differs() {
    let mut r = rig("/he", 3);
    r.key(Key::Escape);
    assert!(!r.open());
    assert!(!r.key(Key::Other).consumed);
    assert!(!r.open());
    r.type_text("/hel");
    r.key(Key::Other);
    assert!(r.open());
}

// native: a `/` pick replaces the whole text, even with text after the caret; `@` keeps what follows it
#[test]
fn a_slash_pick_replaces_the_whole_text_and_an_at_pick_keeps_what_follows() {
    let mut r = rig("/he and more", 3);
    r.select("/help ");
    assert_eq!((r.text.as_str(), r.caret), ("/help ", 6));
    let mut r = rig("see @al and more", 7);
    r.select("@alpha ");
    assert_eq!((r.text.as_str(), r.caret), ("see @alpha  and more", 11));
}

// native: a draft that starts with `/` never opens `@`; fleet commands come first and hide a harness command of the same name
#[test]
fn a_draft_that_starts_with_slash_never_opens_at_and_fleet_commands_come_first() {
    let mut r = rig("/btw hi @", 9);
    assert!(!r.open());
    r.lists.fleet_commands = vec![command("btw", "Ask a side question without disturbing the session")];
    r.lists.set_commands(vec![command("help", "Show help"), command("btw", "the harness's")]);
    r.type_text("/");
    assert_eq!(r.labels(), ["/btw", "/help"]);
    assert_eq!(r.items()[0].description.as_deref(), Some("Ask a side question without disturbing the session"));
}

// native: the skill hint says how the harness delivers it
#[test]
fn the_skill_list_hint_says_how_the_harness_delivers_a_skill() {
    assert_eq!(skill_list_hint(Some("opencode2"), "OpenCode 2"), "OpenCode 2 loads it with the message");
    assert_eq!(skill_list_hint(Some("claude-code"), "Claude Code"), "Claude Code runs the first skill in a message");
    assert_eq!(skill_list_hint(None, "Codex"), "Codex gets a note naming it, then loads it");
}

// native: skills are tinted by how they go: the tokens' ranges, and a `$` after a skill with no space is one token
#[test]
fn skill_tokens_report_where_each_one_is() {
    assert_eq!(
        skill_tokens("Use $fleet-debug, then $fleet-simplify.", &SKILLS),
        [("fleet-debug".to_string(), 4, 16), ("fleet-simplify".to_string(), 23, 38)]
    );
    assert!(skill_tokens("$fleet-debug$fleet-simplify", &SKILLS).is_empty());
}

// native: the New Session draft has no skills, so `$` is plain there
#[test]
fn the_new_session_draft_has_no_dollar_list() {
    let r = new_session("costs $5");
    assert!(!r.open());
}

// native: the picks hold at most 50 sessions, the oldest going first
#[test]
fn the_picks_hold_at_most_fifty_sessions() {
    let mut picks = Picks::default();
    for n in 0..55 {
        picks.remember_session(reference(&format!("@s{n}"), &format!("id{n}"), "t"));
    }
    assert_eq!(picks.sessions().len(), MAX_REMEMBERED);
    assert_eq!(picks.sessions()[0].token, "@s5");
}

// native: picked files are forgotten when their token leaves the text (files on every change)
#[test]
fn picked_files_are_forgotten_when_their_token_leaves_the_text() {
    let mut picks = Picks::default();
    picks.remember_file("@src/app.ts");
    picks.remember_file("@src/app.ts");
    picks.remember_file("@docs/");
    assert_eq!(picks.files().len(), 2);
    picks.forget_files_not_in("look at @src/app.ts.");
    assert_eq!(picks.files(), ["@src/app.ts"]);
    picks.forget_files(Some(&["@src/app.ts"]));
    assert!(picks.files().is_empty());
}
