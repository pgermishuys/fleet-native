//! Invented skills for Settings → Skills & tools when the app has no Fleet to ask (and for its scenes and tests). Nothing here
//! is from a real machine.
use crate::fleet::settings::install_target::InstallTarget;
use crate::fleet::settings::skills::{InstalledSkill, Source};
use crate::fleet::settings::skills_catalog::CatalogEntry;

pub const REPOSITORY: &str = "/work/lantern";

fn skill(name: &str, target: InstallTarget, source: Source, from: Option<&str>, paths: &[&str]) -> InstalledSkill {
    let github = source == Source::GitHub;
    InstalledSkill {
        name: name.into(),
        target,
        source,
        repo_url: github.then(|| from.map(str::to_string)).flatten(),
        local_path: (source == Source::Local).then(|| from.map(str::to_string)).flatten(),
        installed_paths: paths.iter().map(|p| p.to_string()).collect(),
        update_available: false,
        update_check_error: None,
        description: if github { from.map(str::to_string) } else { None },
        path: from.map(str::to_string).filter(|_| source != Source::Bundled),
    }
}

/// A global skill from a local folder, the same skill in one repository, a GitHub install, a bundled skill and one a harness
/// can't see.
pub fn installed() -> Vec<InstalledSkill> {
    let project = || InstallTarget::Project(REPOSITORY.into());
    vec![
        skill(
            "release-notes",
            InstallTarget::Global,
            Source::Local,
            Some("/work/skills/release-notes"),
            &["~/.config/opencode/skills/release-notes"],
        ),
        skill(
            "release-notes",
            project(),
            Source::Local,
            Some("/work/skills/release-notes"),
            &["/work/lantern/.opencode/skills/release-notes"],
        ),
        skill(
            "tidy-imports",
            InstallTarget::Global,
            Source::GitHub,
            Some("https://github.com/acme-labs/tidy-imports.git"),
            &["~/.config/opencode/skills/tidy-imports"],
        ),
        skill("commit-style", InstallTarget::Global, Source::Bundled, None, &["~/.config/opencode/skills/commit-style"]),
        skill("harbor-deploy", project(), Source::Local, Some("/work/skills/harbor-deploy"), &[]),
    ]
}

/// The repositories the picker lists.
pub fn repositories() -> Vec<(String, String)> {
    vec![
        (REPOSITORY.into(), "lantern".into()),
        ("/work/harbor-api".into(), "harbor-api".into()),
        ("/work/tidepool".into(), "tidepool".into()),
    ]
}

/// What "Check for updates" finds with no Fleet: the GitHub skill has a newer version.
pub fn has_update(name: &str) -> bool {
    name == "tidy-imports"
}

fn entry(name: &str, display: Option<&str>, about: &str, source: Source, from: Option<&str>) -> CatalogEntry {
    CatalogEntry {
        name: name.into(),
        display_name: display.map(str::to_string),
        description: Some(about.into()),
        source,
        repo_url: (source == Source::GitHub).then(|| from.map(str::to_string)).flatten(),
        ref_: (source == Source::GitHub).then(|| "main".to_string()),
        sub_path: None,
        local_path: (source == Source::Local).then(|| from.map(str::to_string)).flatten(),
        target_harnesses: vec!["opencode".into()],
        author: None,
        version: None,
        tags: Vec::new(),
    }
}

/// What the catalog offers with no Fleet: one that is already installed, a long description, tags, and each source.
pub fn catalog() -> Vec<CatalogEntry> {
    let mut notes = entry(
        "release-notes",
        Some("Release notes"),
        "Drafts release notes from the merged pull requests since the last tag, grouped by area, in the voice of the project's \
         earlier notes. Reads the changelog first so a repeated line is never written twice, and leaves anything it can't place \
         for you to sort by hand. Works from the git history alone, so it needs no network and no tokens at all.",
        Source::Local,
        Some("/work/skills/release-notes"),
    );
    notes.author = Some("Lantern team".into());
    notes.version = Some("1.2.0".into());
    notes.tags = vec!["docs".into(), "git".into()];
    let mut tidy = entry(
        "tidy-imports",
        None,
        "Sorts and groups imports the way the repository already does.",
        Source::GitHub,
        Some("https://github.com/acme-labs/tidy-imports.git"),
    );
    tidy.author = Some("acme-labs".into());
    tidy.version = Some("0.4.1".into());
    tidy.tags = vec!["style".into()];
    let mut style =
        entry("commit-style", Some("Commit style"), "Writes commit messages in the project's own format.", Source::Bundled, None);
    style.version = Some("2.0.0".into());
    let probe = entry(
        "api-probe",
        Some("API probe"),
        "Calls an endpoint and reports what came back.",
        Source::GitHub,
        Some("https://github.com/harbor-dev/api-probe.git"),
    );
    vec![notes, probe, tidy, style]
}

/// The install a catalog card makes with no Fleet.
pub fn from_entry(entry: &CatalogEntry, target: InstallTarget) -> InstalledSkill {
    let folder = match &target {
        InstallTarget::Global => format!("~/.config/opencode/skills/{}", entry.name),
        InstallTarget::Project(p) => format!("{p}/.opencode/skills/{}", entry.name),
    };
    InstalledSkill {
        name: entry.name.clone(),
        target,
        source: entry.source,
        repo_url: entry.repo_url.clone(),
        local_path: entry.local_path.clone(),
        installed_paths: vec![folder],
        update_available: false,
        update_check_error: None,
        description: entry.repo_url.clone(),
        path: entry.repo_url.clone().or_else(|| entry.local_path.clone()),
    }
}

/// The install a typed address makes with no Fleet.
pub fn from_address(name: &str, url: &str, target: InstallTarget) -> InstalledSkill {
    let github = url.contains("github.com");
    let mut e = entry(name, None, "", if github { Source::GitHub } else { Source::Local }, Some(url));
    e.description = None;
    from_entry(&e, target)
}
