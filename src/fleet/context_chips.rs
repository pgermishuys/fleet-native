//! What the header's context chips show (`SessionContextChips.vue`, rows 10.x) and the words of the pull request pill's trigger
//! (`PullRequestPill.vue`, row 11.1). Plain data: the view only draws it.
//!
//! The chips read generic smart links (`resource_type`, `provider_id`) the way Fleet's does; nothing here imports GitHub
//! code. A plugin that fills the places gets the same chips.
use super::pr_state::{self, Checks, PrState};
use super::rows::Origin;
use super::smart_links::{
    Enrichment, Relationship, SmartLink, SmartLinks, is_pull_request, link_number, link_pr_facts, link_title, needs_attention,
};

/// `MAX_CHIPS`: how many of the other links get a chip of their own; the rest are "+N".
pub const MAX_CHIPS: usize = 2;

/// A GitHub origin the server hasn't turned into a link yet, shown from the session's own record (row 10.4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingOrigin {
    pub number: String,
    pub title: String,
    pub is_pull_request: bool,
}

/// One link as a small chip (rows 10.5, 10.6).
#[derive(Clone, Debug, PartialEq)]
pub struct LinkChip {
    pub link: SmartLink,
    pub number: String,
    /// The tooltip, and the start of the aria label (`describe`).
    pub description: String,
    pub attention: bool,
}

/// The pull request pill's trigger (row 11.1).
#[derive(Clone, Debug, PartialEq)]
pub struct Pill {
    pub link: SmartLink,
    pub number: String,
    /// `data-pr`: the state's colour; neutral (draft) until GitHub has answered.
    pub state: PrState,
    /// Whether GitHub has answered: only then does the pill show the words.
    pub resolved: bool,
    pub words: String,
    /// The check icon of an open pull request with checks.
    pub checks: Option<Checks>,
    pub description: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ContextChips {
    /// The automation that started the session, by its title.
    pub automation: Option<String>,
    pub pending: Option<PendingOrigin>,
    pub origin: Option<LinkChip>,
    pub pill: Option<Pill>,
    pub links: Vec<LinkChip>,
    pub overflow: usize,
    /// The link the "+N" chip asks the Context tab to show (the first one with no chip).
    pub overflow_target: String,
}

/// `describe`: "Pull request #187: Title. Session started from this. Open".
pub fn describe(link: &SmartLink) -> String {
    let kind = if is_pull_request(link) { "Pull request" } else { "Issue" };
    let number = link_number(link).map(|n| n.to_string()).unwrap_or_default();
    let mut parts = vec![format!("{kind} #{number}: {}", link_title(link))];
    if link.relationship == Relationship::Origin {
        parts.push("Session started from this".into());
    }
    if link.enrichment == Enrichment::Resolved && !link.status_label.is_empty() {
        parts.push(link.status_label.clone());
    }
    parts.join(". ")
}

fn number_text(link: &SmartLink) -> String {
    link_number(link).map(|n| n.to_string()).unwrap_or_default()
}

fn chip(link: &SmartLink) -> LinkChip {
    LinkChip { link: link.clone(), number: number_text(link), description: describe(link), attention: needs_attention(link) }
}

/// The part of `/pull/N` or `/issues/N` in a URL.
fn url_number(url: &str) -> Option<String> {
    ["/pull/", "/issues/"].iter().find_map(|marker| {
        let at = url.find(marker)? + marker.len();
        let digits: String = url[at..].chars().take_while(char::is_ascii_digit).collect();
        (!digits.is_empty()).then_some(digits)
    })
}

impl Pill {
    pub fn of(link: &SmartLink) -> Pill {
        let facts = link_pr_facts(link);
        let resolved = link.enrichment == Enrichment::Resolved;
        let state = if resolved { pr_state::pr_state(&facts) } else { PrState::Draft };
        let words = pr_state::pr_words(&facts);
        let open = facts.state == pr_state::Lifecycle::Open;
        let number = number_text(link);
        Pill {
            number: number.clone(),
            state,
            resolved,
            checks: (resolved && open && facts.checks != Checks::None).then_some(facts.checks),
            description: format!("Pull request #{number}: {}. {words}", link_title(link)),
            words,
            link: link.clone(),
        }
    }
}

impl ContextChips {
    /// The chips of one session, or `None` when it has no automation origin, pending origin or header link (the row isn't drawn).
    pub fn of(links: &SmartLinks, session: &str, origin: Option<&Origin>) -> Option<ContextChips> {
        let header = links.header_links(session);
        let pull_request = links.session_pull_request(session);
        let pill_id = pull_request.map(|l| l.id.clone());
        let origin_link = header.iter().find(|l| l.relationship == Relationship::Origin && Some(&l.id) != pill_id.as_ref()).copied();
        let origin_id = origin_link.map(|l| l.id.clone());
        let others: Vec<&SmartLink> =
            header.iter().filter(|l| Some(&l.id) != pill_id.as_ref() && Some(&l.id) != origin_id.as_ref()).copied().collect();

        let automation = origin
            .filter(|o| o.source_type == "automation")
            .map(|o| o.title.clone().filter(|t| !t.is_empty()).unwrap_or_else(|| "Automation".into()));
        let pending = origin.and_then(|o| {
            let url = o.resource_url.as_deref().filter(|u| !u.is_empty())?;
            if o.provider_id != "builtin.github" || header.iter().any(|l| l.relationship == Relationship::Origin || l.url == url) {
                return None;
            }
            Some(PendingOrigin {
                number: url_number(url)?,
                title: o.title.clone().unwrap_or_else(|| url.to_string()),
                is_pull_request: o.source_type == "github-pull-request",
            })
        });
        if automation.is_none() && pending.is_none() && header.is_empty() {
            return None;
        }
        let shown = others.len().min(MAX_CHIPS);
        Some(ContextChips {
            automation,
            pending,
            origin: origin_link.map(chip),
            pill: pull_request.map(Pill::of),
            links: others[..shown].iter().map(|l| chip(l)).collect(),
            overflow: others.len() - shown,
            overflow_target: others.get(MAX_CHIPS).map(|l| l.id.clone()).unwrap_or_default(),
        })
    }
}
