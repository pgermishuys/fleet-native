//! Where the plugin's page is: Fleet's routes under `/github` (`routes/github*.tsx`).
//!
//! `/github` is Your work, `/github/$owner/$repo` a repository, and `/github/$owner/$repo/pulls|issues/$number` one pull request
//! or issue. Pure: the pieces that draw these pages read it, and the address bar and links write it.

/// Whether an item is a pull request or an issue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Pull,
    Issue,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Where {
    /// `/github`: Your work.
    #[default]
    Home,
    /// `/github/$owner/$repo`.
    Repo { owner: String, repo: String },
    /// `/github/$owner/$repo/pulls/$number` or `/issues/$number`.
    Item { owner: String, repo: String, kind: Kind, number: u64 },
}

impl Where {
    /// The address, as Fleet writes it.
    pub fn path(&self) -> String {
        match self {
            Where::Home => "/github".into(),
            Where::Repo { owner, repo } => format!("/github/{owner}/{repo}"),
            Where::Item { owner, repo, kind, number } => {
                let dir = if *kind == Kind::Pull { "pulls" } else { "issues" };
                format!("/github/{owner}/{repo}/{dir}/{number}")
            }
        }
    }

    /// The place an address names, `None` when it isn't one of the plugin's (a number that isn't one, a third folder that is
    /// neither `pulls` nor `issues`, an extra segment).
    pub fn parse(path: &str) -> Option<Where> {
        let rest = path.strip_prefix("/github")?;
        let rest = rest.trim_end_matches('/');
        if rest.is_empty() {
            return Some(Where::Home);
        }
        let parts: Vec<&str> = rest.strip_prefix('/')?.split('/').collect();
        if parts.iter().any(|p| p.is_empty()) {
            return None;
        }
        let (owner, repo) = (parts.first()?.to_string(), parts.get(1)?.to_string());
        match parts.as_slice() {
            [_, _] => Some(Where::Repo { owner, repo }),
            [_, _, dir @ ("pulls" | "issues"), number] => {
                let number = number.parse().ok()?;
                Some(Where::Item { owner, repo, kind: if *dir == "pulls" { Kind::Pull } else { Kind::Issue }, number })
            }
            _ => None,
        }
    }
}
