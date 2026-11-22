//! Fleet's command list (`composables/use-commands.ts`, shortcuts from `lib/keybinding-types.ts`).
//! A command either runs here, waits on the component that owns its target, or opens a level.
use crate::desktop::menu::{self, Item};
use crate::icons::*;
use crate::{Rail, State, Tx};
use rune::Icon;
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Category {
    Session,
    Navigation,
    View,
    Fleet,
}

impl Category {
    /// `CATEGORY_ORDER` in `CommandPalette.vue`.
    pub const ORDER: [Category; 4] = [Category::Session, Category::Navigation, Category::View, Category::Fleet];
    pub fn name(self) -> &'static str {
        match self {
            Category::Session => "Session",
            Category::Navigation => "Navigation",
            Category::View => "View",
            Category::Fleet => "Fleet",
        }
    }
}

pub type Action = Rc<dyn Fn(&mut State, &mut Tx)>;

#[derive(Clone)]
pub enum Run {
    Now(Action),
    /// Listed, and says which component builds its target (`docs/PORT.md`).
    Waiting(u8),
    /// Drills into a level (`subCommands`).
    Level(Vec<Command>),
}

#[derive(Clone)]
pub struct Command {
    pub id: String,
    pub label: String,
    pub description: String,
    pub icon: &'static Icon,
    pub category: Category,
    pub shortcut: Option<String>,
    pub keywords: &'static [&'static str],
    pub disabled: bool,
    pub run: Run,
}

/// `Ctrl+` on Windows and Linux, `⌘` on macOS (`formatGlobalShortcut`).
fn md(keys: &str) -> String {
    if cfg!(target_os = "macos") { format!("⌘{}", keys.replace("Shift+", "⇧")) } else { format!("Ctrl+{keys}") }
}

#[allow(clippy::too_many_arguments)]
fn c(
    id: &str,
    label: &str,
    description: &str,
    icon: &'static Icon,
    category: Category,
    shortcut: Option<String>,
    keywords: &'static [&'static str],
    run: Run,
) -> Command {
    Command {
        id: id.into(),
        label: label.into(),
        description: description.into(),
        icon,
        category,
        shortcut,
        keywords,
        disabled: false,
        run,
    }
}

fn now(f: impl Fn(&mut State, &mut Tx) + 'static) -> Run {
    Run::Now(Rc::new(f))
}

pub fn all(s: &State) -> Vec<Command> {
    use Category::*;
    let open = s.has_session();
    let mut list = vec![
        c(
            "nav-fleet",
            "Go to Sessions",
            "Open the sessions workspace.",
            &MESSAGE_SQUARE,
            Navigation,
            None,
            &["sessions", "home"],
            now(|s, _| s.rail = Rail::Sessions),
        ),
        c(
            "nav-board",
            "Go to Board",
            "Open the fleet board view.",
            &LAYOUT_GRID,
            Navigation,
            None,
            &["kanban", "overview", "board"],
            now(|s, _| s.enter_labs(Rail::Board)),
        ),
        c(
            "nav-analytics",
            "Go to Analytics",
            "Open analytics and reporting.",
            &CHART_COLUMN,
            Navigation,
            None,
            &["usage", "stats"],
            now(|s, _| s.enter_analytics()),
        ),
        c(
            "nav-settings",
            "Go to Settings",
            "Open workspace settings.",
            &SETTINGS,
            Navigation,
            None,
            &["preferences", "config"],
            now(|s, _| s.enter_settings()),
        ),
        c(
            "add-machine",
            "Add a machine",
            "Connect another machine to this Fleet.",
            &MONITOR,
            Navigation,
            None,
            &["node", "pair", "remote"],
            now(|s, _| s.open_add_machine()),
        ),
        c(
            "new-session",
            "New session",
            "Create and open a new session.",
            &PLUS,
            Session,
            Some(md("N")),
            &["create", "start"],
            now(|s, _| s.open_new_session_page(None)),
        ),
        c(
            "refresh-sessions",
            "Refresh sessions",
            "Reload the current sessions list.",
            &REFRESH_CW,
            Session,
            None,
            &["reload"],
            Run::Waiting(2),
        ),
    ];
    let file = |id: &str, label: &str, d: &str, icon: &'static Icon, keys: &str| {
        let mut cmd = c(id, label, d, icon, Session, Some(md(keys)), &["file", "search"], Run::Waiting(11));
        cmd.disabled = !open;
        cmd
    };
    let mut go = file("go-to-file", "Go to file…", "Open a file from this session by name.", &FILE_SEARCH, "P");
    go.run = now(|s, tx| s.go_to_file_open(tx));
    go.keywords = &["file", "open", "find", "quick open", "editor"];
    if !open {
        go.description = "Open a session to find its files.".into();
    }
    list.push(go);
    let mut find = file("find-in-files", "Find in files…", "Search inside the files of this session's folder.", &TEXT_SEARCH, "Shift+F");
    find.run = now(|s, _| s.find_in_files_ask());
    if !open {
        find.description = "Open a session to search its files.".into();
    }
    find.keywords = &["search", "grep", "text", "contents", "inside", "files"];
    list.push(find);
    let mut focus = c(
        "focus-prompt",
        "Focus prompt",
        "Focus the active session prompt.",
        &MESSAGE_SQUARE,
        Session,
        None,
        &["composer", "message", "input"],
        now(|_, tx| tx.focus("composer", false)),
    );
    focus.disabled = !open;
    list.push(focus);
    // Go to session…: a level with every session but the open one.
    let mut goto = c(
        "nav-go-to-session",
        "Go to session…",
        "Jump to any session.",
        &MESSAGE_SQUARE,
        Session,
        None,
        &["switch", "open"],
        Run::Level(session_level(s)),
    );
    goto.disabled = s.projects.iter().all(|p| p.sessions.is_empty());
    list.push(goto);
    let many = s.projects.iter().map(|p| p.sessions.len()).sum::<usize>() > 1;
    for (id, label, by, keys, icon) in [
        ("nav-next-session", "Next session", 1isize, "]", &CHEVRON_RIGHT),
        ("nav-prev-session", "Previous session", -1, "[", &CHEVRON_LEFT),
    ] {
        let mut cmd = c(
            id,
            label,
            if many { "Switch to the next or previous session." } else { "A second session is required." },
            icon,
            Session,
            Some(md(keys)),
            &[],
            now(move |s, tx| s.step_session(by, tx)),
        );
        cmd.disabled = !many;
        list.push(cmd);
    }
    let mut interrupt = c(
        "interrupt-session",
        "Interrupt session",
        if open { "Abort the active session." } else { "No active session selected." },
        &OCTAGON_X,
        Session,
        Some("Esc".into()),
        &["abort", "stop", "cancel"],
        now(|s, _| crate::interrupt(s)),
    );
    interrupt.disabled = !open;
    list.push(interrupt);
    let session_commands: [(&str, &str, &str, &'static rune::Icon, Run); 3] = [
        ("copy-session-id", "Copy session ID", "Copy the active session ID.", &COPY, now(|s, tx| s.copy_session_id(tx))),
        ("fork-session", "Fork session", "Branch the active session into a new one.", &GIT_BRANCH_PLUS, now(|s, tx| s.toolbar_fork(tx))),
        (
            "export-raw-messages",
            "Export raw messages (JSON)",
            "Download the loaded messages as JSON.",
            &DOWNLOAD,
            now(|s, tx| s.export_raw_messages(tx)),
        ),
    ];
    for (id, label, d, icon, run) in session_commands {
        let mut cmd = c(id, label, d, icon, Session, None, &[], run);
        cmd.disabled = !open;
        list.push(cmd);
    }
    let mut in_folder = c(
        "new-session-in-folder",
        "New session in this folder",
        if open { "An empty session in the active session's folder, on the same harness." } else { "No active session selected." },
        &FOLDER_PLUS,
        Session,
        None,
        &["new", "session", "folder", "same", "empty"],
        now(|s, _| s.new_session_in_folder()),
    );
    in_folder.disabled = !open;
    list.push(in_folder);
    let mut export = c(
        "export-conversation",
        "Export as Markdown",
        "Copy or save the active conversation as Markdown.",
        &DOWNLOAD,
        Session,
        None,
        &["export", "download", "conversation", "markdown", "copy", "transcript"],
        now(|s, _| s.export_open()),
    );
    export.disabled = !open;
    list.push(export);
    let mut top = c(
        "scroll-to-top",
        "Scroll to top",
        "Scroll the activity stream to the top.",
        &ARROW_UP_TO_LINE,
        Session,
        None,
        &[],
        now(|s, tx| s.scroll_top(tx)),
    );
    top.disabled = !open;
    list.push(top);
    let mut bottom = c(
        "scroll-to-bottom",
        "Scroll to bottom",
        "Scroll the activity stream to the latest message.",
        &ARROW_DOWN_TO_LINE,
        Session,
        None,
        &[],
        now(|s, tx| s.follow_end(tx)),
    );
    bottom.disabled = !open;
    list.push(bottom);
    let mut clear = c(
        "clear-conversation",
        "Clear draft",
        "Clear the current composer draft.",
        &ERASER,
        Session,
        None,
        &[],
        now(|s, _| s.draft.clear()),
    );
    clear.disabled = !open;
    list.push(clear);
    list.extend([
        c(
            "toggle-sidebar",
            if s.sidebar_open { "Hide sidebar" } else { "Show sidebar" },
            if s.sidebar_open { "Collapse the left context panel." } else { "Expand the left context panel." },
            &PANEL_LEFT_CLOSE,
            View,
            Some(md("B")),
            &["panel", "list"],
            now(|s, _| s.set_sidebar_open(!s.sidebar_open)),
        ),
        c(
            "toggle-right-panel",
            if s.content_open { "Hide right panel" } else { "Show right panel" },
            "Collapse or expand the right detail panel.",
            &PANEL_RIGHT_CLOSE,
            View,
            Some(md("Shift+B")),
            &["content", "detail"],
            now(|s, _| s.content_open = !s.content_open),
        ),
        {
            // Fleet turns it off while no session is open ("Open a session to use its terminal.").
            let mut terminal = c(
                "toggle-terminal",
                "Show terminal",
                if open { "Show or hide the terminal under the chat." } else { "Open a session to use its terminal." },
                &SQUARE_TERMINAL,
                View,
                Some(md("J")),
                &["shell", "console"],
                now(|s, tx| s.toggle_terminal(tx)),
            );
            terminal.disabled = !open;
            terminal
        },
        c(
            "toggle-diff-view",
            "Show inline tool diffs",
            "Show edits as diffs in the conversation.",
            &SQUARE_DASHED_BOTTOM,
            View,
            Some(md("D")),
            &[],
            Run::Waiting(3),
        ),
        c("toggle-activity-filter", "Show all sessions", "List archived sessions too.", &ARCHIVE, View, None, &[], Run::Waiting(6)),
        c(
            "cycle-theme",
            "Cycle theme",
            "Switch between System, Dark and Light.",
            &MOON_STAR,
            View,
            Some(md("T")),
            &["appearance", "dark", "light"],
            now(|s, _| {
                use crate::fleet::settings::appearance::ThemeChoice::{Dark, Light, System};
                let next = match s.dark {
                    None => Dark,
                    Some(true) => Light,
                    Some(false) => System,
                };
                s.set_theme(next);
            }),
        ),
        c(
            "toggle-dark-light",
            if s.drawn_dark.get() { "Switch to light mode" } else { "Switch to dark mode" },
            "Flip the colour scheme.",
            &SUN,
            View,
            None,
            &["theme"],
            now(|s, _| s.toggle_theme()),
        ),
        c(
            "toggle-fullscreen",
            "Toggle full screen",
            "Enter or exit fullscreen mode.",
            &MAXIMIZE_2,
            View,
            None,
            &[],
            now(|s, tx| s.menu_run(Item::ToggleFullScreen, tx)),
        ),
        c("zoom-in", "Zoom in", "Increase the interface scale.", &ZOOM_IN, View, Some(md("=")), &["larger"], Run::Waiting(9)),
        c("zoom-out", "Zoom out", "Decrease the interface scale.", &ZOOM_OUT, View, Some(md("-")), &["smaller"], Run::Waiting(9)),
        c(
            "open-marketplace-panel",
            "Open plugins",
            "Turn plugins on or off in Settings.",
            &PUZZLE,
            Fleet,
            None,
            &["marketplace", "extensions"],
            now(|s, _| s.open_settings("plugins", None)),
        ),
        c(
            "report-problem",
            "Report a problem",
            "Tell the Fleet maintainers what went wrong.",
            &BUG,
            Fleet,
            None,
            &["bug", "issue", "feedback", "help", "report"],
            now(|s, _| {
                s.report_open(crate::fleet::report::Source::Palette, None, "");
            }),
        ),
    ]);
    // Only with the Mods switch on: it flips between stopping the user's mods and starting them again (`useStartWithoutMods`).
    if s.home_mods_on() {
        let (label, description) = if s.mods_safe_mode() {
            ("Turn mods back on", "Start your mods again.")
        } else {
            ("Start without mods", "Stop your mods until Fleet restarts or you turn them back on.")
        };
        list.push(c(
            "start-without-mods",
            label,
            description,
            &POWER_OFF,
            Fleet,
            None,
            &["mods", "safe mode", "disable", "stop", "extensions"],
            now(|s, _| s.toggle_start_without_mods()),
        ));
    }
    // Fleet's menu bar has these; Rune draws one on macOS only, so the palette carries them (24 H, decision 9).
    for (item, id, label, description) in menu::palette() {
        let icon = match item {
            Item::Quit => &POWER_OFF,
            Item::CheckForUpdates => &REFRESH_CW,
            _ => &FOLDER_OPEN,
        };
        list.push(c(id, label, description, icon, Fleet, None, &[], now(move |s, tx| s.menu_run(item, tx))));
    }
    list
}

fn session_level(s: &State) -> Vec<Command> {
    let mut out = Vec::new();
    for (p, project) in s.projects.iter().enumerate() {
        for (i, session) in project.sessions.iter().enumerate() {
            let mut cmd = c(
                &format!("nav-session-{p}-{i}"),
                &session.title,
                &format!(
                    "{} · {}",
                    project.name,
                    match session.status {
                        crate::mock::Status::Working => "working",
                        crate::mock::Status::NeedsInput => "needs input",
                        crate::mock::Status::Idle => "idle",
                    }
                ),
                &MESSAGE_SQUARE,
                Category::Session,
                None,
                &[],
                now(move |s, tx| {
                    s.rail = Rail::Sessions;
                    s.open_session(p, i, tx);
                }),
            );
            cmd.disabled = s.open_at() == (p, i);
            out.push(cmd);
        }
    }
    out
}

/// What the palette's search keeps and in what order: contiguous matches of the label first,
/// then of the keywords and description, then letters in order. An empty query keeps all, in
/// the registry's order. Fleet's cmdk ranks with its own scorer; this is ours.
pub fn score(cmd: &Command, query: &str) -> Option<u32> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Some(0);
    }
    let label = cmd.label.to_lowercase();
    if label.starts_with(&q) {
        return Some(1000);
    }
    if label.contains(&q) {
        return Some(800);
    }
    let rest = format!("{} {}", cmd.keywords.join(" "), cmd.description).to_lowercase();
    if rest.contains(&q) {
        return Some(500);
    }
    let mut hay = label.chars();
    q.chars().all(|ch| hay.any(|h| h == ch)).then_some(100)
}

/// The command `id` as the list stands now (the status bar's hints read their label and reason from it).
pub fn find(s: &State, id: &str) -> Option<Command> {
    all(s).into_iter().find(|c| c.id == id)
}

/// What a hint's button does: runs the command unless it is missing or disabled (`runCommand`); a command still waiting
/// on its component says so with a toast, the way the palette does.
pub fn run(s: &mut State, tx: &mut Tx, id: &str) {
    let Some(cmd) = find(s, id) else { return };
    if cmd.disabled {
        return;
    }
    match cmd.run {
        Run::Now(f) => f(s, tx),
        Run::Waiting(n) => s.toast(format!("{} isn't in the native app yet (component {n} of the port).", cmd.label), None, 4.0, tx),
        Run::Level(_) => {}
    }
}
