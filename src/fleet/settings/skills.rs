//! Settings → Skills & tools, as plain data (`composables/use-skills.ts`, `SkillsToolsPage.vue`, `InstalledSkillsTab.vue`): the
//! installed skills and how the list answer is read, the calls under `/api/skills`, the page's own state, and which tab opens.
//! No Rune and no I/O here but [`send`], which a thread calls; what touches the app is `skills_actions.rs`.
use super::install_target::{InstallTarget, to_install_target};
use super::skills_catalog::CatalogEntry;
use crate::conv::export_history::encode_component;
use crate::fleet::http;
use rune::TextEdit;
use serde_json::Value;

/// The three tabs of the page. Mods exists only while the Mods switch is on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Skills,
    Tools,
    Mods,
}

/// The tabs of Skills and of Tools (`Installed`, `Catalog`, `Custom`); each starts on Installed whenever the page opens.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Inner {
    #[default]
    Installed,
    Catalog,
    Custom,
}

impl Inner {
    pub const ALL: [Inner; 3] = [Inner::Installed, Inner::Catalog, Inner::Custom];
    pub fn id(self) -> &'static str {
        match self {
            Inner::Installed => "installed",
            Inner::Catalog => "catalog",
            Inner::Custom => "custom",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Inner::Installed => "Installed",
            Inner::Catalog => "Catalog",
            Inner::Custom => "Custom",
        }
    }
}

/// The server's preference for the Mods switch (`MODS_PREFERENCE_KEY`): "true" or "false" once the user has chosen.
pub const MODS_PREFERENCE_KEY: &str = "Mods";

/// `SkillSource`: the API sends 0, 1 or 2 (`mapSkillSource`); a number it doesn't know reads as local.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Bundled,
    GitHub,
    Local,
}

impl Source {
    pub fn from_number(n: i64) -> Source {
        match n {
            0 => Source::Bundled,
            1 => Source::GitHub,
            _ => Source::Local,
        }
    }
    /// `mapSkillSourceToNumber`.
    pub fn number(self) -> i64 {
        match self {
            Source::Bundled => 0,
            Source::GitHub => 1,
            Source::Local => 2,
        }
    }
}

/// One install of a skill (`InstalledSkill`): the same skill can be global and in any number of repositories.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstalledSkill {
    pub name: String,
    pub target: InstallTarget,
    pub source: Source,
    pub repo_url: Option<String>,
    pub local_path: Option<String>,
    /// The skill folders Fleet wrote; empty when the skill isn't anywhere a harness reads.
    pub installed_paths: Vec<String>,
    pub update_available: bool,
    pub update_check_error: Option<String>,
    /// For GitHub installs the repository's address: the list sends no description (Fleet's quirk, copied).
    pub description: Option<String>,
    pub path: Option<String>,
}

impl InstalledSkill {
    /// The card's key (`name|targetKey`): busy and confirm states are per install.
    pub fn key(&self) -> String {
        format!("{}|{}", self.name, self.target.key())
    }

    /// The path line: `path || repoUrl || localPath`. For a GitHub install the address shows twice, as the description and here.
    pub fn path_line(&self) -> Option<&str> {
        [&self.path, &self.repo_url, &self.local_path].into_iter().flatten().map(String::as_str).find(|p| !p.is_empty())
    }
}

fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

/// `GET /api/skills`: `{skills: [...]}`. A body with nothing in it is "No data returned"; a skill with no name is dropped.
pub fn parse_skills(body: &Value) -> Result<Vec<InstalledSkill>, String> {
    if body.is_null() {
        return Err("No data returned".into());
    }
    let list = body.get("skills").and_then(Value::as_array).cloned().unwrap_or_default();
    Ok(list
        .iter()
        .filter_map(|s| {
            let name = text(s, "name").filter(|n| !n.is_empty())?;
            let source = Source::from_number(s.get("source").and_then(Value::as_i64).unwrap_or(2));
            let (repo_url, local_path) = (text(s, "repoUrl"), text(s, "localPath"));
            Some(InstalledSkill {
                name,
                target: to_install_target(text(s, "scope").as_deref(), text(s, "projectPath").as_deref()),
                source,
                description: if source == Source::GitHub { repo_url.clone() } else { None },
                path: match source {
                    Source::Local => local_path.clone(),
                    Source::GitHub => repo_url.clone(),
                    Source::Bundled => None,
                },
                repo_url,
                local_path,
                installed_paths: s
                    .get("installedPaths")
                    .and_then(Value::as_array)
                    .map(|p| p.iter().filter_map(|p| p.as_str().map(str::to_string)).collect())
                    .unwrap_or_default(),
                update_available: false,
                update_check_error: None,
            })
        })
        .collect())
}

// ─── Requests ───────────────────────────────────────────────────────────────

pub struct Request {
    pub method: &'static str,
    pub path: String,
    /// What the page says when the server gave no reason of its own.
    pub fallback: &'static str,
}

fn skill_path(name: &str, tail: &str, target: &InstallTarget) -> String {
    format!("/api/skills/{}{tail}?{}", encode_component(name), target.query())
}

pub fn list() -> Request {
    Request { method: "GET", path: "/api/skills".into(), fallback: "Failed to fetch skills" }
}
pub fn check_update(name: &str, target: &InstallTarget) -> Request {
    Request { method: "GET", path: skill_path(name, "/update-check", target), fallback: "Failed to check for updates" }
}
pub fn update(name: &str, target: &InstallTarget) -> Request {
    Request { method: "POST", path: skill_path(name, "/update", target), fallback: "Failed to update skill" }
}
pub fn remove(name: &str, target: &InstallTarget) -> Request {
    Request { method: "DELETE", path: skill_path(name, "", target), fallback: "Failed to remove skill" }
}

/// `extractApiError`: the reason a refusal gives (`detail`, `error`, `title` or `message`), a plain-text body as it is, else
/// the page's own words.
pub fn extract_error(body: &str, fallback: &str) -> String {
    let body = body.trim();
    match serde_json::from_str::<Value>(body) {
        Ok(Value::String(s)) if !s.is_empty() => s,
        Ok(v) => ["detail", "error", "title", "message"]
            .iter()
            .find_map(|k| v.get(k).and_then(Value::as_str).filter(|t| !t.is_empty()).map(str::to_string))
            .unwrap_or_else(|| fallback.to_string()),
        Err(_) if !body.is_empty() => body.to_string(),
        Err(_) => fallback.to_string(),
    }
}

/// Sends one call to the machine at `base`: the body (null when it had none), or why not. Blocking: a thread calls it.
pub fn send(base: &str, req: &Request) -> Result<Value, String> {
    send_with(base, req, None)
}

/// [`send`] with a JSON body.
pub fn send_with(base: &str, req: &Request, body: Option<&Value>) -> Result<Value, String> {
    let (status, body) = http::text_with(req.method, base, &req.path, body)?;
    if !(200..300).contains(&status) {
        return Err(extract_error(&body, req.fallback));
    }
    Ok(serde_json::from_str(&body).unwrap_or(Value::Null))
}

/// `{updateAvailable}` of the update check; a body with nothing in it is "No data returned".
pub fn parse_update_check(body: &Value) -> Result<bool, String> {
    if body.is_null() {
        return Err("No data returned".into());
    }
    Ok(body.get("updateAvailable").and_then(Value::as_bool) == Some(true))
}

// ─── The GitHub address ─────────────────────────────────────────────────────

/// What `parseGitHubUrl` reads out of a GitHub address.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitHubUrl {
    pub repo_url: String,
    pub ref_: Option<String>,
    pub sub_path: Option<String>,
    pub name: String,
}

/// `parseGitHubUrl`: `https://github.com/owner/repo`, `…/tree/<branch>` or `…/tree/<branch>/<path>` (`blob` too), with an
/// optional `.git` and trailing slash. Shared by the Custom tabs of Skills and Tools.
pub fn parse_github_url(url: &str) -> Option<GitHubUrl> {
    let rest = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://"))?.strip_prefix("github.com/")?;
    let (owner, rest) = rest.split_once('/')?;
    if owner.is_empty() {
        return None;
    }
    let (repo, tail) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, ""),
    };
    let repo = if repo.len() > 4 { repo.strip_suffix(".git").unwrap_or(repo) } else { repo };
    if repo.is_empty() {
        return None;
    }
    let (ref_, sub_path) = if tail.is_empty() || tail == "/" {
        (None, None)
    } else {
        let after = tail.strip_prefix("/tree/").or_else(|| tail.strip_prefix("/blob/"))?;
        let (branch, path) = after.split_once('/').map_or((after, ""), |(b, p)| (b, p));
        if branch.is_empty() {
            return None;
        }
        let path = path.trim_matches('/');
        (Some(branch.to_string()), (!path.is_empty()).then(|| path.to_string()))
    };
    let name = sub_path.as_deref().and_then(|p| p.split('/').rfind(|s| !s.is_empty())).unwrap_or(repo).to_string();
    Some(GitHubUrl { repo_url: format!("https://github.com/{owner}/{repo}.git"), ref_, sub_path, name })
}

// ─── The page ───────────────────────────────────────────────────────────────

/// What Skills & tools keeps while it is open: the machine it asked, the list, the install target the Catalog and Custom tabs
/// share, and the busy and confirm states (per install, as `InstalledSkillsTab.vue` keeps them).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SkillsPage {
    pub loaded_for: Option<String>,
    pub tab: Tab,
    /// The anchor the tab was last chosen from, so a later one (the old `tools` and `mods` ids) re-chooses it.
    pub anchor_seen: Option<String>,
    pub inner: Inner,
    /// What the shown machine answered to `GET /api/features/mods` (`on`); none until read, or when it didn't say.
    pub mods_server: Option<bool>,
    pub mods_seen_on: bool,
    pub skills: Vec<InstalledSkill>,
    pub loading: bool,
    pub error: Option<String>,
    /// Numbers the list's reads, so an answer to one the page has left or one a newer read overtook is dropped.
    pub seq: u64,
    /// Numbers the list's own reads (a remove's reload overtakes the one before it).
    pub reads: u64,
    pub target: InstallTarget,
    /// An install from Catalog or Custom is under way: the picker is disabled while it is.
    pub installing: bool,
    /// The repositories the picker lists, by path and name; `repos_loading` while they are read.
    pub repositories: Vec<(String, String)>,
    pub repos_loading: bool,
    pub checking: Option<String>,
    pub updating: Option<String>,
    pub removing: Option<String>,
    pub confirm_remove: Option<String>,
    /// Catalog (`useSkillCatalog`): read each time the tab opens.
    pub catalog: Vec<CatalogEntry>,
    pub catalog_loading: bool,
    pub catalog_error: Option<String>,
    /// Numbers the catalog's reads, so an answer to one the tab has left is dropped.
    pub catalog_reads: u64,
    /// The catalog card being installed (the others keep working; the picker is disabled).
    pub installing_name: Option<String>,
    /// The failure of a catalog install, under the grid.
    pub install_error: Option<String>,
    /// Custom: the address field and the form's own error; both are forgotten when the tab is left.
    pub custom: TextEdit,
    pub custom_error: Option<String>,
}

impl Eq for SkillsPage {}

impl SkillsPage {
    /// The Mods switch (`isSwitchedOn`): the user's choice (the `Mods` preference, which is the app's own) wins as soon as it
    /// is made, else the server's answer, else off.
    pub fn mods_on(&self, preference: Option<&str>) -> bool {
        match preference {
            Some("") | None => self.mods_server.unwrap_or(false),
            Some(chosen) => chosen == "true",
        }
    }

    /// `tabFor`: `tools` opens Tools, `mods` opens Mods only while the switch is on, anything else Skills.
    pub fn tab_for(&self, anchor: Option<&str>, preference: Option<&str>) -> Tab {
        match anchor {
            Some("tools") => Tab::Tools,
            Some("mods") if self.mods_on(preference) => Tab::Mods,
            _ => Tab::Skills,
        }
    }

    /// The two watches of `SkillsToolsPage.vue`: landed on `mods` before the switch was known, it opens once it is known to
    /// be on; with the switch off while Mods is open, the page falls back to Skills.
    pub fn settle_mods(&mut self, anchor: Option<&str>, preference: Option<&str>) {
        let on = self.mods_on(preference);
        // Both watches fire when the switch changes, not every frame: a user who left Mods stays out of it.
        if on == self.mods_seen_on {
            return;
        }
        self.mods_seen_on = on;
        if on && anchor == Some("mods") {
            self.tab = Tab::Mods;
        }
        if !on && self.tab == Tab::Mods {
            self.tab = Tab::Skills;
        }
    }

    /// Patches the install a check, update or remove named (`patchSkill`): same name and same target.
    pub fn patch(&mut self, name: &str, target: &InstallTarget, change: impl Fn(&mut InstalledSkill)) {
        for s in self.skills.iter_mut().filter(|s| s.name == name && s.target.is_same(target)) {
            change(s);
        }
    }
}

/// Whether `skill` is `name` at `target` (the catalog's "Installed" check).
pub fn installed_key_matches(skill: &InstalledSkill, name: &str, target: &InstallTarget) -> bool {
    skill.name == name && skill.target.is_same(target)
}
