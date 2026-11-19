//! The Weave config dialog's draft (`WeaveConfigDialog.vue`, `lib/weave-flavors.ts`): whose config Weave reads in Fleet sessions,
//! the files being edited, Test, Save and Copy. The config is Fleet's, not a harness's, so every tab's Weave group opens the
//! same dialog; the draft lives on the page's [`Weave`] and stays while the dialog is closed or a tab is switched.
use super::harness_profiles::error_words;
use super::harness_weave::{Detection, Flavor, Weave, WeaveView, loaded_flavors, outdated_flavors, usable_flavors};
use crate::State;
use crate::fleet::http;
use rune::{Key, TextEdit};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub const CONFIG: &str = "config.weave";
pub const LEGACY_CONFIG: &str = "weave-opencode.jsonc";
pub const PROMPTS: &str = "prompts/";

impl Flavor {
    /// The file Fleet keeps for this Weave.
    pub fn file(self) -> &'static str {
        match self {
            Flavor::Weave => CONFIG,
            Flavor::Legacy => LEGACY_CONFIG,
        }
    }
    /// The variable Fleet hands the folder to when it starts OpenCode.
    pub fn variable(self) -> &'static str {
        match self {
            Flavor::Weave => "WEAVE_GLOBAL_CONFIG_DIR",
            Flavor::Legacy => "WEAVE_OPENCODE_CONFIG_DIR",
        }
    }
    /// Where a repo's own config for it lives, which still applies on top.
    pub fn project_path(self) -> &'static str {
        match self {
            Flavor::Weave => "<repo>/.weave/config.weave",
            Flavor::Legacy => "<repo>/.opencode/weave-opencode.jsonc",
        }
    }
    /// What a missing file starts with.
    pub fn starter(self) -> &'static str {
        match self {
            Flavor::Weave => {
                "# Fleet's Weave config. Weave reads it in place of ~/.weave/config.weave in Fleet sessions.\n\
                 # A repo's own .weave/config.weave still applies on top.\n"
            }
            Flavor::Legacy => {
                "{\n  // Fleet's Weave Legacy config. Weave Legacy reads it in place of\n\
                 \x20 // ~/.config/opencode/weave-opencode.jsonc in Fleet sessions.\n\
                 \x20 // A repo's own .opencode/weave-opencode.jsonc still applies on top.\n}\n"
            }
        }
    }
    fn wire(self) -> &'static str {
        match self {
            Flavor::Weave => "weave",
            Flavor::Legacy => "legacy",
        }
    }
}

/// What the dialog is busy with: one thing at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Busy {
    Test,
    Save,
    Copy,
}

/// One harness's answer to Test or Save (`WeaveHarnessCheck`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Check {
    pub harness_name: String,
    pub flavor: Flavor,
    pub ok: bool,
    pub agents: Vec<String>,
    pub error: Option<String>,
    pub details: Vec<String>,
}

impl Check {
    fn parse(v: &Value) -> Option<Check> {
        let c = v.get("check")?;
        let strings = |k: &str| -> Vec<String> {
            c.get(k)
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(|t| t.as_str().map(str::to_string)).collect())
                .unwrap_or_default()
        };
        Some(Check {
            harness_name: v.get("harnessName").and_then(Value::as_str).unwrap_or_default().to_string(),
            flavor: Flavor::BOTH.into_iter().find(|f| Some(f.wire()) == v.get("flavor").and_then(Value::as_str)).unwrap_or(Flavor::Weave),
            ok: c.get("ok").and_then(Value::as_bool) == Some(true),
            agents: strings("agents"),
            error: c.get("error").and_then(Value::as_str).map(str::to_string),
            details: strings("details"),
        })
    }
    fn parse_all(v: &Value) -> Vec<Check> {
        v.as_array().map(|a| a.iter().filter_map(Check::parse).collect()).unwrap_or_default()
    }
}

/// The dialog: whether it is open, and the draft (source, files, the file being edited) with what Test and Save said.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Config {
    pub open: bool,
    /// Weave reads the config Fleet keeps (else the user's own file).
    pub fleet_source: bool,
    pub files: BTreeMap<String, String>,
    pub active: String,
    /// The text of `active`; copied into `files` by [`Weave::commit_editor`].
    pub editor: TextEdit,
    pub busy: Option<Busy>,
    pub checks: Option<Vec<Check>>,
    pub notice: Option<String>,
    pub adding: bool,
    pub new_prompt: TextEdit,
}

impl Eq for Config {}

/// `promptFileName`: `.md` is added when it is missing.
pub fn prompt_file_name(name: &str) -> String {
    let name = name.trim();
    if name.ends_with(".md") { name.to_string() } else { format!("{name}.md") }
}

/// `^[A-Za-z0-9_-][A-Za-z0-9._-]*$`.
fn prompt_name_ok(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

fn sorted(files: &BTreeMap<String, String>) -> Vec<(&String, &String)> {
    files.iter().collect()
}

/// The last part of a folder's path, for either separator (`folderName`).
pub fn folder_name(directory: &str) -> &str {
    directory.rsplit(['/', '\\']).find(|p| !p.is_empty()).unwrap_or(directory)
}

impl Config {
    /// The files as drafted: the editor's text belongs to the file being edited.
    pub fn current_files(&self) -> BTreeMap<String, String> {
        let mut files = self.files.clone();
        if let Some(f) = files.get_mut(&self.active) {
            f.clone_from(&self.editor.text);
        }
        files
    }

    /// `dirty`: the source or a file differs from what is saved.
    pub fn dirty(&self, view: &WeaveView) -> bool {
        self.fleet_source != view.fleet_source || sorted(&self.current_files()) != sorted(&view.files)
    }

    /// `fileTabs`: the files of the Weaves this machine runs; a file of one it doesn't stays saved, out of the way.
    pub fn file_tabs(&self, harnesses: &[Detection]) -> Vec<String> {
        let flavors = loaded_flavors(harnesses);
        let (weave, legacy) = (flavors.contains(&Flavor::Weave), flavors.contains(&Flavor::Legacy));
        let mut tabs = Vec::new();
        if weave && self.files.contains_key(CONFIG) {
            tabs.push(CONFIG.to_string());
        }
        if weave {
            tabs.extend(self.files.keys().filter(|p| p.starts_with(PROMPTS)).cloned());
        }
        if legacy && self.files.contains_key(LEGACY_CONFIG) {
            tabs.push(LEGACY_CONFIG.to_string());
        }
        tabs
    }

    /// The editor shows `path`.
    pub fn select(&mut self, path: &str) {
        self.commit_editor();
        self.active = path.to_string();
        self.editor = TextEdit::new(self.files.get(path).cloned().unwrap_or_default());
    }

    /// The editor's text goes into its file; true when that changed the file (and so clears the checks).
    pub fn commit_editor(&mut self) -> bool {
        match self.files.get_mut(&self.active) {
            Some(f) if *f != self.editor.text => {
                f.clone_from(&self.editor.text);
                self.checks = None;
                true
            }
            _ => false,
        }
    }

    /// `fillStarters`: (the editor's text is committed before the files change) every Weave Fleet can hand a config to gets its file, so there is somewhere to start.
    pub fn fill_starters(&mut self, usable: &[Flavor]) {
        for f in usable {
            self.files.entry(f.file().to_string()).or_insert_with(|| f.starter().to_string());
        }
        self.reload_editor();
    }

    fn reload_editor(&mut self) {
        let path = self.active.clone();
        self.editor = TextEdit::new(self.files.get(&path).cloned().unwrap_or_default());
    }

    /// The watch on the view: a load fills the draft unless it has changes that aren't saved (`dirty` is read against the new
    /// view, as Fleet reads it).
    pub fn on_view(&mut self, view: &WeaveView, had_previous: bool) {
        if had_previous && self.dirty(view) {
            return;
        }
        self.fleet_source = view.fleet_source;
        self.files = view.files.clone();
        if view.fleet_source {
            self.fill_starters(&usable_flavors(&view.harnesses));
        }
        self.reload_editor();
    }

    /// `chooseSource`.
    pub fn choose_source(&mut self, fleet: bool, usable: &[Flavor]) {
        if fleet && usable.is_empty() {
            return;
        }
        self.commit_editor();
        self.fleet_source = fleet;
        self.checks = None;
        self.notice = None;
        if fleet {
            self.fill_starters(usable);
        }
    }

    /// `promptNameProblem` for the name typed in the "+ prompt" box.
    pub fn prompt_problem(&self) -> Option<&'static str> {
        let name = self.new_prompt.text.trim();
        if name.is_empty() {
            None
        } else if !prompt_name_ok(name) {
            Some("Use letters, digits, '-', '_' and '.'.")
        } else if self.files.contains_key(&format!("{PROMPTS}{}", prompt_file_name(name))) {
            Some("There's already a prompt with that name.")
        } else {
            None
        }
    }

    /// `addPrompt`: an empty file next to the config, which opens.
    pub fn add_prompt(&mut self) {
        let name = self.new_prompt.text.trim().to_string();
        if name.is_empty() || self.prompt_problem().is_some() {
            return;
        }
        self.commit_editor();
        let path = format!("{PROMPTS}{}", prompt_file_name(&name));
        self.files.insert(path.clone(), String::new());
        self.select(&path);
        self.adding = false;
        self.new_prompt = TextEdit::default();
    }

    /// `removePrompt`.
    pub fn remove_prompt(&mut self, path: &str) {
        self.commit_editor();
        self.files.remove(path);
        if self.active == path {
            self.reload_editor();
        }
    }
}

impl Weave {
    fn detections(&self) -> &[Detection] {
        self.view.as_ref().map_or(&[], |v| v.harnesses.as_slice())
    }
    pub fn usable(&self) -> Vec<Flavor> {
        usable_flavors(self.detections())
    }
    pub fn outdated(&self) -> Vec<Flavor> {
        outdated_flavors(self.detections())
    }
    pub fn dirty(&self) -> bool {
        self.view.as_ref().is_some_and(|v| self.config.dirty(v))
    }

    /// `show`: the answer replaces what was there, a save that still waits is followed, and the draft is refilled.
    pub fn show(&mut self, view: WeaveView) {
        let had = self.view.is_some();
        self.next_poll = view.apply_pending().then(|| crate::fleet::link::now_ms() + super::harness_weave::APPLY_POLL_MS);
        self.config.on_view(&view, had);
        self.view = Some(view);
    }

    /// Every frame while the page shows: what is typed goes into the file, and the tab follows the files that can be edited.
    pub fn keep_config(&mut self) -> bool {
        let mut changed = self.config.commit_editor();
        let tabs = self.config.file_tabs(self.detections());
        if self.view.is_some() && !tabs.contains(&self.config.active) {
            let first = tabs.first().cloned().unwrap_or_else(|| CONFIG.to_string());
            self.config.select(&first);
            changed = true;
        }
        changed
    }
}

/// One flavour's own files: which Weave, where they are read from, the files.
type Own = (Flavor, String, BTreeMap<String, String>);

/// The server's `{saved, checks, config}`.
struct Saved {
    saved: bool,
    checks: Vec<Check>,
    config: Option<WeaveView>,
}

impl State {
    /// Edit…: opens the dialog on the draft as it was left.
    pub fn open_weave_config(&mut self) {
        let w = &mut self.settings.harnesses.weave;
        w.config.open = true;
        w.config.notice = None;
    }

    pub fn close_weave_config(&mut self) {
        self.settings.harnesses.weave.config.open = false;
    }

    pub fn weave_choose_source(&mut self, fleet: bool) {
        let w = &mut self.settings.harnesses.weave;
        let usable = w.usable();
        w.config.choose_source(fleet, &usable);
    }

    /// The "+ prompt" tab turns into a form; Esc in its box turns it back.
    pub fn weave_adding_prompt(&mut self, on: bool) {
        self.settings.harnesses.weave.config.adding = on;
    }

    /// Keys of the prompt's name box: Esc cancels, Enter adds.
    pub fn weave_prompt_key(&mut self, key: &Key) -> bool {
        match key {
            Key::Escape => {
                self.weave_adding_prompt(false);
                true
            }
            _ => false,
        }
    }

    fn weave_machine_call<T: Send + 'static>(
        &mut self,
        call: impl FnOnce(&str) -> T + Send + 'static,
        done: impl FnOnce(&mut State, T) + Send + 'static,
    ) {
        let machine = self.harness_machine();
        let here = machine.clone();
        let sent = self.feed_call(&machine, call, move |s, _, answer| {
            if s.harness_machine() == here {
                done(s, answer);
            }
        });
        if !sent {
            self.settings.harnesses.weave.config.busy = None;
        }
    }

    /// `copyOwn`: reads the user's own files of every Weave Fleet can hand a config to, replaces each one's files as a set and
    /// picks Fleet as the source. Nothing is saved.
    pub fn weave_copy_own(&mut self) {
        let w = &mut self.settings.harnesses.weave;
        if w.config.busy.is_some() {
            return;
        }
        w.config.busy = Some(Busy::Copy);
        w.config.notice = None;
        let usable = w.usable();
        self.weave_machine_call(
            move |base| -> Result<Vec<Own>, String> {
                let mut out = Vec::new();
                for f in usable {
                    let (status, body) = http::weave_own(base, f.wire())?;
                    if !(200..300).contains(&status) {
                        return Err(error_words(status, &body));
                    }
                    let v: Value = serde_json::from_str(&body).unwrap_or_default();
                    let files = v
                        .get("files")
                        .and_then(Value::as_object)
                        .map(|o| o.iter().filter_map(|(k, t)| Some((k.clone(), t.as_str()?.to_string()))).collect())
                        .unwrap_or_default();
                    out.push((f, v.get("path").and_then(Value::as_str).unwrap_or_default().to_string(), files));
                }
                Ok(out)
            },
            |s, answer| {
                let w = &mut s.settings.harnesses.weave;
                w.config.busy = None;
                match answer {
                    Err(why) => w.config.notice = Some(why),
                    Ok(read) => {
                        let usable = w.usable();
                        let c = &mut w.config;
                        c.commit_editor();
                        let mut paths = Vec::new();
                        for (flavor, path, own) in read {
                            if own.is_empty() {
                                continue;
                            }
                            // The flavour's files are replaced as a set: its config and, for Weave, its prompts.
                            c.files.retain(|p, _| {
                                if flavor == Flavor::Weave { p != CONFIG && !p.starts_with(PROMPTS) } else { p != LEGACY_CONFIG }
                            });
                            c.files.extend(own);
                            paths.push(path);
                        }
                        c.fleet_source = true;
                        c.fill_starters(&usable);
                        c.checks = None;
                        c.notice = Some(if paths.is_empty() {
                            "Your own Weave files are empty, so there was nothing to copy.".to_string()
                        } else {
                            format!("Copied from {}. Not saved yet.", paths.join(" and "))
                        });
                    }
                }
            },
        );
    }

    /// Test: every usable Weave that has a file is tried with the harnesses; nothing is saved.
    pub fn weave_test(&mut self) {
        let w = &mut self.settings.harnesses.weave;
        if w.config.busy.is_some() {
            return;
        }
        w.config.commit_editor();
        w.config.busy = Some(Busy::Test);
        w.config.notice = None;
        let files = w.config.files.clone();
        let tries: Vec<Flavor> = w.usable().into_iter().filter(|f| files.contains_key(f.file())).collect();
        self.weave_machine_call(
            move |base| -> Result<Vec<Check>, String> {
                let mut all = Vec::new();
                for f in tries {
                    let (status, body) = http::weave_check(base, f.wire(), &json!(files))?;
                    if !(200..300).contains(&status) {
                        return Err(error_words(status, &body));
                    }
                    all.extend(Check::parse_all(&serde_json::from_str(&body).unwrap_or_default()));
                }
                Ok(all)
            },
            |s, answer| {
                let c = &mut s.settings.harnesses.weave.config;
                c.busy = None;
                match answer {
                    Ok(checks) => c.checks = Some(checks),
                    Err(why) => c.notice = Some(why),
                }
            },
        );
    }

    /// Save: with Fleet as the source the harnesses try the files first, and a failed check saves nothing and comes back as
    /// `checks`.
    pub fn weave_save(&mut self) {
        let w = &mut self.settings.harnesses.weave;
        if w.config.busy.is_some()
            || w.view.is_none()
            || (!w.dirty() && w.config.fleet_source == w.view.as_ref().is_some_and(|v| v.fleet_source))
        {
            return;
        }
        w.config.commit_editor();
        w.config.busy = Some(Busy::Save);
        w.config.notice = None;
        let (fleet, files) = (w.config.fleet_source, w.config.files.clone());
        self.weave_machine_call(
            move |base| -> Result<Saved, String> {
                let (status, body) = http::weave_save(base, if fleet { "fleet" } else { "own" }, &json!(files))?;
                if !(200..300).contains(&status) {
                    return Err(error_words(status, &body));
                }
                let v: Value = serde_json::from_str(&body).unwrap_or_default();
                Ok(Saved {
                    saved: v.get("saved").and_then(Value::as_bool) == Some(true),
                    checks: Check::parse_all(v.get("checks").unwrap_or(&Value::Null)),
                    config: v.get("config").and_then(WeaveView::parse),
                })
            },
            move |s, answer| {
                let w = &mut s.settings.harnesses.weave;
                w.config.busy = None;
                match answer {
                    Err(why) => w.config.notice = Some(why),
                    Ok(done) => {
                        w.config.checks = (!done.checks.is_empty()).then_some(done.checks);
                        if done.saved {
                            if let Some(config) = done.config {
                                w.show(config);
                            }
                            w.config.notice = Some(
                                if fleet {
                                    "Saved. New sessions use it, and running ones pick it up as each folder goes idle."
                                } else {
                                    "Saved. New sessions read your own Weave files."
                                }
                                .to_string(),
                            );
                        }
                    }
                }
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_names_follow_the_rule() {
        assert!(prompt_name_ok("team"));
        assert!(prompt_name_ok("_a.b-c"));
        assert!(!prompt_name_ok(".hidden"));
        assert!(!prompt_name_ok("a b"));
        assert!(!prompt_name_ok("a/b"));
        assert_eq!(prompt_file_name(" team "), "team.md");
        assert_eq!(prompt_file_name("team.md"), "team.md");
    }

    #[test]
    fn folders_are_named_by_their_last_part() {
        assert_eq!(folder_name("/work/alpha"), "alpha");
        assert_eq!(folder_name("C:\\work\\beta\\"), "beta");
        assert_eq!(folder_name("/"), "/");
    }

    #[test]
    fn starters_are_fleet_s_exactly() {
        assert_eq!(Flavor::Weave.starter().lines().count(), 2);
        assert!(Flavor::Weave.starter().starts_with("# Fleet's Weave config. Weave reads it in place of ~/.weave/config.weave"));
        assert!(Flavor::Legacy.starter().starts_with("{\n  // Fleet's Weave Legacy config."));
        assert!(Flavor::Legacy.starter().ends_with("on top.\n}\n"));
    }
}
