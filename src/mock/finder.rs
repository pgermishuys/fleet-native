//! Invented commands, skills, files, agents and sessions for the `/`, `$` and `@` lists when no server is given. Nothing
//! here comes from a real Fleet.
use crate::composer::files::narrow_file_matches;
use crate::composer::refs::{Command, SessionItem};
use crate::composer::triggers::Agent;

fn command(name: &str, description: &str, kind: &str, user_only: bool) -> Command {
    Command { name: name.into(), description: Some(description.into()), kind: Some(kind.into()), user_only }
}

/// What a harness lists: commands, and skills among them.
pub fn commands() -> Vec<Command> {
    vec![
        command("help", "Show what this session can do", "command", false),
        command("init", "Create or update the project's AGENTS.md", "command", false),
        command("review", "Review the changes in the working tree", "command", false),
        command("compact", "Summarise the conversation to free up context", "command", false),
        command("undo", "Undo the last message and its file changes", "command", false),
        command("tidy-docs", "Keep the docs in step with the code", "skill", false),
        command("release-notes", "Draft release notes from merged changes", "skill", false),
        command("deploy-check", "Run the pre-deploy checklist", "skill", true),
    ]
}

pub fn agents() -> Vec<Agent> {
    let agent = |name: &str, description: &str, mode: &str, color: &str| Agent {
        name: name.into(),
        description: Some(description.into()),
        mode: Some(mode.into()),
        color: Some(color.into()),
    };
    vec![
        agent("build", "Edits files and runs commands", "primary", "#3B82F6"),
        agent("plan", "Reads the code and proposes a plan", "primary", "#22C55E"),
        agent("explore", "Searches the code without changing it", "subagent", "#F59E0B"),
    ]
}

const FILES: [&str; 14] = [
    "docs/",
    "src/",
    "tests/",
    "README.md",
    "Cargo.toml",
    "docs/guide.md",
    "docs/setup.md",
    "src/cache/",
    "src/main.rs",
    "src/cache/store.rs",
    "src/cache/evict.rs",
    "src/cache/index.rs",
    "tests/cache.rs",
    "tests/evict.rs",
];

/// What the server's file search answers for `query`: an empty query or one ending in "/" lists that folder's children,
/// folders first; anything else is a search (`narrowFileMatches` ranks it as Fleet's server does).
pub fn find_files(query: &str) -> Vec<String> {
    let q = query.trim();
    if q.is_empty() || q.ends_with('/') {
        let mut children: Vec<String> = FILES
            .iter()
            .filter(|p| p.strip_prefix(q).is_some_and(|rest| !rest.is_empty() && !rest.trim_end_matches('/').contains('/')))
            .map(|p| p.to_string())
            .collect();
        children.sort_by_key(|p| !p.ends_with('/'));
        return children;
    }
    let all: Vec<String> = FILES.iter().map(|p| p.to_string()).collect();
    narrow_file_matches(&all, q)
}

/// Sessions the store doesn't list (archived ones, sub-agents), as `GET /api/sessions?retentionStatus=all` has them, with
/// their `updated` as milliseconds before `now`.
pub fn other_sessions(now: i64) -> Vec<SessionItem> {
    let session = |id: &str, title: &str, status: &str, minutes: i64, archived: bool, parent: Option<&str>| SessionItem {
        id: id.into(),
        title: title.into(),
        status: status.into(),
        harness_type: "opencode".into(),
        project_name: Some("harbor-api".into()),
        workspace_directory: "/work/harbor-api".into(),
        parent_session_id: parent.map(str::to_string),
        archived,
        updated: (now - minutes * 60_000) as f64,
        ..Default::default()
    };
    vec![
        session("mock-old-1", "Tune the eviction order", "idle", 4_300, true, None),
        session("mock-old-2", "Draft the release notes", "idle", 130, false, None),
    ]
}

impl crate::State {
    /// The reference scenes (component 5, piece L): the open (idle) session on Claude Code, whose first named skill goes with
    /// the message and the rest as a note, with a draft that names a file, a folder, a session and two skills; or the
    /// caret beside a picked file; or that file taken out whole.
    pub fn show_references(&mut self, state: &str) {
        use crate::composer::refs::SessionReference;
        self.set_open((0, 2));
        self.projects[0].sessions[2].row.harness_type = Some("claude-code".into());
        self.pickers.harnesses = vec![crate::composer::status::HarnessInfo {
            kind: "claude-code".into(),
            display_name: Some("Claude Code".into()),
            ..Default::default()
        }];
        let owner = self.draft_owner().unwrap_or_default();
        let draft = match state {
            "all" => {
                let picks = self.auto.picks_mut(&owner);
                picks.remember_file("@src/cache.rs");
                picks.remember_file("@docs/");
                picks.remember_session(SessionReference {
                    token: "@cache-notes".into(),
                    session_id: "mock-notes".into(),
                    title: "Cache notes".into(),
                });
                "Compare @src/cache.rs and @docs/ with @cache-notes, then run $tidy-docs and $release-notes. "
            }
            "beside" => {
                self.auto.picks_mut(&owner).remember_file("@docs/guide.md");
                "Read @docs/guide.md"
            }
            _ => "Read ",
        };
        self.show_draft(draft);
    }

    /// The popup's scenes: the open (idle) session with a draft that opens a list, and the lists' own states.
    pub fn show_autocomplete(&mut self, state: &str) {
        use crate::composer::status::HarnessInfo;
        use crate::fleet::finder::Loaded;
        self.set_open((0, 2));
        let harness = if state.starts_with("skill-") { "claude-code" } else { "opencode" };
        self.projects[0].sessions[2].row.harness_type = Some(harness.into());
        self.pickers.harnesses = vec![HarnessInfo {
            kind: harness.into(),
            display_name: Some(if harness == "claude-code" { "Claude Code" } else { "OpenCode" }.into()),
            supports_fleet_tools: true,
            ..Default::default()
        }];
        // The first frames run the clock from zero: the sessions' ages are read against a fixed one.
        self.now_ms = Some(20_000 * 86_400_000);
        let owner = self.draft_owner().unwrap_or_default();
        let draft = match state {
            "slash" => "/",
            "slash-filtered" => "/re",
            "mention" => "@",
            "mention-query" => "@re",
            "mention-folder" => "@docs/",
            "skill" => "$t",
            "skill-warning" => "$tidy-docs and $rel",
            "skill-disabled" => "$tidy-docs and $dep",
            "loading" => "/",
            "error" => "/",
            "empty" => "@zzz",
            _ => "",
        };
        match state {
            "loading" => {
                self.finder.commands.insert(owner, Loaded { loading: true, ..Default::default() });
            }
            "error" => {
                self.finder.commands.insert(owner, Loaded { error: Some("Failed to fetch".into()), ..Default::default() });
            }
            _ => {}
        }
        self.show_draft(draft);
    }
}
