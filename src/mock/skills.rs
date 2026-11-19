//! Invented skills for Settings → Skills & tools when the app has no Fleet to ask (and for its scenes and tests). Nothing here
//! is from a real machine.
use crate::fleet::settings::install_target::InstallTarget;
use crate::fleet::settings::skills::{InstalledSkill, Source};

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
