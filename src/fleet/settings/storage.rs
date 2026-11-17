//! Settings → Folders → Storage as plain data (`StorageSection.vue`, `lib/storage-size-cache.ts`): the checkouts Fleet made, their
//! sizes, the totals and labels, the report of a clean, and the cache that keeps a measured size for a few minutes. No Rune and no
//! I/O here but [`load`], [`measure`] and [`clean`], which a thread calls. What touches the app is `storage_actions.rs`.
use crate::composer::format::format_bytes;
use crate::fleet::http;
use crate::fleet::new_session::feeds::extract_error;
use serde_json::{Value, json};
use std::collections::{HashMap, VecDeque};

pub const LOAD_FALLBACK: &str = "Couldn't load the worktree checkouts.";
pub const CLEAN_FALLBACK: &str = "Couldn't clean the checkouts.";
/// How many folder sizes are asked for at once; each is a walk of the folder.
pub const SIZE_CONCURRENCY: usize = 3;
/// How long a measured size is shown again without measuring it again (`STORAGE_SIZE_TTL_MS`), in seconds on the state's clock.
pub const SIZE_TTL_SECS: f64 = 5.0 * 60.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Remove,
    Keep,
    Recent,
    InUse,
}

/// One checkout (`WorktreeCheckout`), the parts the card shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checkout {
    pub workspace_id: String,
    pub title: String,
    pub repository: String,
    pub verdict: Verdict,
    pub reason: String,
}

/// A measured folder: bytes, and `complete` false when Fleet stopped counting at its cap.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Size {
    pub bytes: f64,
    pub complete: bool,
}

pub type Sizes = HashMap<String, Size>;

/// What a clean answers: what went and what stayed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub removed: Vec<Checkout>,
    pub kept: Vec<Checkout>,
}

/// What the list read answers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listing {
    pub checkouts: Vec<Checkout>,
    pub archived_days: u32,
}

fn checkout(v: &Value) -> Option<Checkout> {
    let text = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
    let verdict = match v.get("verdict").and_then(Value::as_str)? {
        "remove" => Verdict::Remove,
        "keep" => Verdict::Keep,
        "recent" => Verdict::Recent,
        "in-use" => Verdict::InUse,
        _ => return None,
    };
    Some(Checkout {
        workspace_id: text("workspaceId")?,
        title: text("title").unwrap_or_default(),
        repository: text("repository").unwrap_or_default(),
        verdict,
        reason: text("reason").unwrap_or_default(),
    })
}

fn checkouts(v: Option<&Value>) -> Vec<Checkout> {
    v.and_then(Value::as_array).into_iter().flatten().filter_map(checkout).collect()
}

pub fn parse_list(status: u16, body: &str) -> Result<Listing, String> {
    if !(200..300).contains(&status) {
        return Err(extract_error(body, LOAD_FALLBACK));
    }
    let v: Value = serde_json::from_str(body).ok().filter(Value::is_object).ok_or_else(|| LOAD_FALLBACK.to_string())?;
    let days = v.get("archivedDays").and_then(|d| d.as_u64().or_else(|| d.as_str()?.parse().ok())).unwrap_or(7);
    Ok(Listing { checkouts: checkouts(v.get("checkouts")), archived_days: days as u32 })
}

/// `bytes` comes typed number or string (an int64); a size fits a number.
pub fn parse_size(body: &str) -> Option<Size> {
    let v: Value = serde_json::from_str(body).ok()?;
    let bytes = v.get("bytes").and_then(|b| b.as_f64().or_else(|| b.as_str()?.parse().ok()))?;
    Some(Size { bytes, complete: v.get("complete").and_then(Value::as_bool)? })
}

pub fn parse_clean(status: u16, body: &str) -> Result<Report, String> {
    if !(200..300).contains(&status) {
        return Err(extract_error(body, CLEAN_FALLBACK));
    }
    let v: Value = serde_json::from_str(body).ok().filter(Value::is_object).ok_or_else(|| CLEAN_FALLBACK.to_string())?;
    Ok(Report { removed: checkouts(v.get("removed")), kept: checkouts(v.get("kept")) })
}

pub fn load(base: &str) -> Result<Listing, String> {
    match http::text_with("GET", base, "/api/storage/worktrees", None) {
        Ok((status, body)) => parse_list(status, &body),
        Err(_) => Err(LOAD_FALLBACK.to_string()),
    }
}

/// One folder's size; a folder that can't be measured stays "—".
pub fn measure(base: &str, id: &str) -> Option<Size> {
    let (status, body) = http::text_with("GET", base, &format!("/api/storage/worktrees/{id}/size"), None).ok()?;
    if !(200..300).contains(&status) { None } else { parse_size(&body) }
}

/// Clean the checkouts of these workspaces, on the machine shown.
pub fn clean(base: &str, ids: &[String]) -> Result<Report, String> {
    match http::text_with("POST", base, "/api/storage/worktrees/clean", Some(&json!({ "workspaceIds": ids }))) {
        Ok((status, body)) => parse_clean(status, &body),
        Err(_) => Err(CLEAN_FALLBACK.to_string()),
    }
}

/// `sumOf`: the bytes and the label for a list. "—" when none of a non-empty list is measured; "≥ " when some are missing or one
/// stopped at its cap.
pub fn sum_of(list: &[&Checkout], sizes: &Sizes) -> (f64, String) {
    let (mut bytes, mut known, mut complete) = (0.0, 0, true);
    for c in list {
        let Some(size) = sizes.get(&c.workspace_id) else { continue };
        known += 1;
        bytes += size.bytes;
        complete &= size.complete;
    }
    let label = if !list.is_empty() && known == 0 {
        "—".to_string()
    } else {
        format!("{}{}", if complete && known == list.len() { "" } else { "≥ " }, format_bytes(Some(bytes)))
    };
    (bytes, label)
}

/// `sizeLabel`.
pub fn size_label(c: &Checkout, sizes: &Sizes) -> String {
    match sizes.get(&c.workspace_id) {
        None => "—".into(),
        Some(s) => format!("{}{}", if s.complete { "" } else { "≥ " }, format_bytes(Some(s.bytes))),
    }
}

/// `noun(n)`: "1 checkout" or "N checkouts".
pub fn noun(n: usize) -> String {
    format!("{n} {}", if n == 1 { "checkout" } else { "checkouts" })
}

pub fn is_are(n: usize) -> &'static str {
    if n == 1 { "is" } else { "are" }
}

pub fn mock_listing() -> Listing {
    crate::mock::storage::listing()
}

/// What the card keeps while Folders is open.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StoragePage {
    /// The machine key it was read from; leaving the page or moving the switch reads again.
    pub loaded_for: Option<String>,
    /// A newer read (or a clean) makes older answers stale (`generation`).
    pub run: u64,
    pub loading: bool,
    pub error: Option<String>,
    pub checkouts: Vec<Checkout>,
    pub archived_days: u32,
    pub sizes: Sizes,
    pub previewing: bool,
    pub cleaning: bool,
    pub report: Option<Report>,
    /// The sizes at the time of the run, so the report still has them after the reload.
    pub report_sizes: Sizes,
    /// Workspaces waiting to be measured, removable ones first, and how many are being measured now.
    pub queue: VecDeque<String>,
    pub in_flight: usize,
}

impl StoragePage {
    pub fn by(&self, verdicts: &[Verdict]) -> Vec<&Checkout> {
        self.checkouts.iter().filter(|c| verdicts.contains(&c.verdict)).collect()
    }

    pub fn removable(&self) -> Vec<&Checkout> {
        self.by(&[Verdict::Remove])
    }

    pub fn kept(&self) -> Vec<&Checkout> {
        self.by(&[Verdict::Keep])
    }

    pub fn recent(&self) -> Vec<&Checkout> {
        self.by(&[Verdict::Recent])
    }

    /// The three boxes: (key, label, list). "in use" counts `in-use` and `recent`.
    pub fn splits(&self) -> [(&'static str, &'static str, Vec<&Checkout>); 3] {
        [
            ("in-use", "in use", self.by(&[Verdict::InUse, Verdict::Recent])),
            ("keep", "need a look", self.kept()),
            ("remove", "could be freed", self.removable()),
        ]
    }

    /// A read's answer: the counts show now; the queue holds what isn't remembered, removable folders first.
    pub fn adopt(&mut self, listing: Listing, recent: Sizes) {
        self.archived_days = listing.archived_days;
        self.sizes.extend(recent);
        let mut order: Vec<&Checkout> = listing.checkouts.iter().filter(|c| !self.sizes.contains_key(&c.workspace_id)).collect();
        order.sort_by_key(|c| c.verdict != Verdict::Remove);
        self.queue = order.into_iter().map(|c| c.workspace_id.clone()).collect();
        self.checkouts = listing.checkouts;
    }
}

/// `lib/storage-size-cache.ts`: per machine, then per workspace, the size and when it was measured (on the state's clock).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SizeCache {
    measured: HashMap<String, HashMap<String, (Size, f64)>>,
}

impl SizeCache {
    /// The sizes measured on this machine within the last few minutes.
    pub fn recent(&self, machine: &str, now: f64) -> Sizes {
        let Some(m) = self.measured.get(machine) else { return Sizes::new() };
        m.iter().filter(|(_, (_, at))| now - at < SIZE_TTL_SECS).map(|(id, (size, _))| (id.clone(), *size)).collect()
    }

    pub fn remember(&mut self, machine: &str, id: &str, size: Size, now: f64) {
        self.measured.entry(machine.to_string()).or_default().insert(id.to_string(), (size, now));
    }
}

impl Eq for StoragePage {}
impl Eq for SizeCache {}
