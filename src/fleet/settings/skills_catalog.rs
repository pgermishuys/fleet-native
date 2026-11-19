//! Settings → Skills & tools → Skills → Catalog and Custom, as plain data (`composables/use-skill-catalog.ts`,
//! `CatalogTab.vue`, `CustomInstallTab.vue`, and `installSkill` of `use-skills.ts`): the catalog's entries and how the answer
//! is read, and the bodies an install sends, from a catalog card or from a typed address. No Rune and no I/O here.
use super::install_target::InstallTarget;
use super::skills::{Request, Source, installed_key_matches, parse_github_url};
use serde_json::{Value, json};

/// One skill the catalog offers (`CatalogEntry`). `isStale` and `cachedAt` of the answer are read by Fleet and shown nowhere,
/// so they aren't kept.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogEntry {
    pub name: String,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub source: Source,
    pub repo_url: Option<String>,
    pub ref_: Option<String>,
    pub sub_path: Option<String>,
    pub local_path: Option<String>,
    pub target_harnesses: Vec<String>,
    pub author: Option<String>,
    pub version: Option<String>,
    pub tags: Vec<String>,
}

impl CatalogEntry {
    /// The card's title: `displayName || name`.
    pub fn title(&self) -> &str {
        self.display_name.as_deref().filter(|d| !d.is_empty()).unwrap_or(&self.name)
    }

    /// The badge on the card's footer.
    pub fn source_label(&self) -> &'static str {
        match self.source {
            Source::GitHub => "GitHub",
            Source::Local => "Local",
            Source::Bundled => "Bundled",
        }
    }

    /// `POST /api/skills/install`'s body for this card (`handleInstall`): `subPath` falls back to the local path.
    pub fn install_body(&self, target: &InstallTarget) -> Value {
        let mut body = json!({
            "name": self.name,
            "source": self.source.number(),
            "repoUrl": self.repo_url,
            "ref": self.ref_,
            "subPath": self.sub_path.as_ref().or(self.local_path.as_ref()),
            "localPath": self.local_path,
            "targetHarnesses": self.target_harnesses,
        });
        merge(&mut body, target.body());
        body
    }
}

fn merge(body: &mut Value, more: Value) {
    if let (Some(into), Value::Object(more)) = (body.as_object_mut(), more) {
        into.extend(more);
    }
}

fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

fn strings(v: &Value, key: &str) -> Vec<String> {
    v.get(key).and_then(Value::as_array).map(|a| a.iter().filter_map(|s| s.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

/// `GET /api/skills/catalog`: `{entries: [...]}`. A body with nothing in it is "No data returned"; an entry with no name is dropped.
pub fn parse_catalog(body: &Value) -> Result<Vec<CatalogEntry>, String> {
    if body.is_null() {
        return Err("No data returned".into());
    }
    let list = body.get("entries").and_then(Value::as_array).cloned().unwrap_or_default();
    Ok(list
        .iter()
        .filter_map(|e| {
            Some(CatalogEntry {
                name: text(e, "name").filter(|n| !n.is_empty())?,
                display_name: text(e, "displayName"),
                description: text(e, "description"),
                source: Source::from_number(e.get("source").and_then(Value::as_i64).unwrap_or(2)),
                repo_url: text(e, "repoUrl"),
                ref_: text(e, "ref"),
                sub_path: text(e, "subPath"),
                local_path: text(e, "localPath"),
                target_harnesses: strings(e, "targetHarnesses"),
                author: text(e, "author"),
                version: text(e, "version"),
                tags: strings(e, "tags"),
            })
        })
        .collect())
}

pub fn catalog() -> Request {
    Request { method: "GET", path: "/api/skills/catalog".into(), fallback: "Failed to fetch skill catalog" }
}

pub fn install() -> Request {
    Request { method: "POST", path: "/api/skills/install".into(), fallback: "Failed to install skill" }
}

/// `installSkill({url, target})`: what a typed address installs. A string with `github.com` in it is a GitHub address and must
/// parse; anything else is a local path named by its last segment. The error is what the form shows.
pub fn custom_install_body(url: &str, target: &InstallTarget) -> Result<Value, String> {
    let url = url.trim();
    if url.is_empty() {
        return Err("URL is required".into());
    }
    let mut body = if url.contains("github.com") {
        let parsed = parse_github_url(url).ok_or(
            "Invalid GitHub URL format. Expected: https://github.com/owner/repo or https://github.com/owner/repo/tree/branch/path",
        )?;
        json!({
            "name": parsed.name, "source": Source::GitHub.number(), "repoUrl": parsed.repo_url, "ref": parsed.ref_,
            "localPath": null, "targetHarnesses": null, "subPath": parsed.sub_path,
        })
    } else {
        let name = url.split(['/', '\\']).rfind(|p| !p.is_empty()).unwrap_or("unknown-skill");
        json!({
            "name": name, "source": Source::Local.number(), "repoUrl": null, "ref": null,
            "localPath": url, "targetHarnesses": null, "subPath": null,
        })
    };
    merge(&mut body, target.body());
    Ok(body)
}

/// Whether `name` is installed at `target` (`isInstalled`): installed somewhere else doesn't count.
pub fn is_installed(installed: &[super::skills::InstalledSkill], name: &str, target: &InstallTarget) -> bool {
    installed.iter().any(|s| installed_key_matches(s, name, target))
}
