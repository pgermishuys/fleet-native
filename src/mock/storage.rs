//! Invented worktree checkouts for Settings → Folders → Storage when there is no Fleet: the same five Fleet's own test uses
//! (a repository, a title, a verdict and a reason), with sizes a clean can add up.
use crate::fleet::settings::storage::{Checkout, Listing, Size, Sizes, Verdict};

fn checkout(id: &str, repository: &str, title: &str, verdict: Verdict, reason: &str) -> Checkout {
    Checkout { workspace_id: id.into(), title: title.into(), repository: repository.into(), verdict, reason: reason.into() }
}

pub fn listing() -> Listing {
    Listing {
        archived_days: 7,
        checkouts: vec![
            checkout("w1", "harbor-api", "Export to CSV", Verdict::Remove, "Archived 9 days ago."),
            checkout("w2", "pocket-notes", "Fix note sync", Verdict::Remove, "Archived 12 days ago."),
            checkout("w3", "lumen-site", "Checkout flow", Verdict::Keep, "Has an ignored file you may want: .env.local"),
            checkout("w4", "lumen-site", "Hero refresh", Verdict::Recent, "Archived yesterday."),
            checkout("w5", "harbor-api", "Add webhooks", Verdict::InUse, "The session isn't archived."),
        ],
    }
}

/// What measuring each folder finds: 2 GB, but 1 GB for the second.
pub fn sizes() -> Sizes {
    const GB: f64 = 1024.0 * 1024.0 * 1024.0;
    listing()
        .checkouts
        .into_iter()
        .map(|c| {
            let bytes = if c.workspace_id == "w2" { GB } else { 2.0 * GB };
            (c.workspace_id, Size { bytes, complete: true })
        })
        .collect()
}
