//! The Sessions table's order (`tabs/SessionsTab.vue`): which column, which way, and how two rows compare.
use super::SessionRow;
use crate::fleet::work::parse_iso_ms;
use std::cmp::Ordering;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortBy {
    Title,
    Project,
    Tokens,
    Cost,
    EstimatedCost,
    DurationSeconds,
    Models,
    CreatedAt,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortDir {
    Asc,
    Desc,
}

/// The columns, left to right.
pub const COLUMNS: [SortBy; 8] = [
    SortBy::Title,
    SortBy::Project,
    SortBy::Tokens,
    SortBy::Cost,
    SortBy::EstimatedCost,
    SortBy::DurationSeconds,
    SortBy::Models,
    SortBy::CreatedAt,
];

impl SortBy {
    /// The name the read sends as `sortBy`.
    pub fn key(self) -> &'static str {
        match self {
            SortBy::Title => "title",
            SortBy::Project => "project",
            SortBy::Tokens => "tokens",
            SortBy::Cost => "cost",
            SortBy::EstimatedCost => "estimatedCost",
            SortBy::DurationSeconds => "durationSeconds",
            SortBy::Models => "models",
            SortBy::CreatedAt => "createdAt",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SortBy::Title => "Title",
            SortBy::Project => "Project",
            SortBy::Tokens => "Tokens",
            SortBy::Cost => "Cost",
            SortBy::EstimatedCost => "Estimated cost",
            SortBy::DurationSeconds => "Duration",
            SortBy::Models => "Models",
            SortBy::CreatedAt => "Created",
        }
    }

    /// The way pressing another column first sorts it.
    pub fn default_dir(self) -> SortDir {
        match self {
            SortBy::Title | SortBy::Project | SortBy::Models => SortDir::Asc,
            _ => SortDir::Desc,
        }
    }

    /// Numbers sit against the right edge.
    pub fn align_right(self) -> bool {
        matches!(self, SortBy::Tokens | SortBy::Cost | SortBy::EstimatedCost | SortBy::DurationSeconds)
    }
}

impl SortDir {
    pub fn key(self) -> &'static str {
        match self {
            SortDir::Asc => "asc",
            SortDir::Desc => "desc",
        }
    }

    pub fn flipped(self) -> SortDir {
        match self {
            SortDir::Asc => SortDir::Desc,
            SortDir::Desc => SortDir::Asc,
        }
    }
}

/// The page opens sorted by Created, newest first.
pub const DEFAULT_SORT: (SortBy, SortDir) = (SortBy::CreatedAt, SortDir::Desc);

/// `handleSort`: pressing the sorted column flips its way; another column takes its own default.
pub fn pressed(current: (SortBy, SortDir), column: SortBy) -> (SortBy, SortDir) {
    if current.0 == column { (column, current.1.flipped()) } else { (column, column.default_dir()) }
}

/// `formatSessionTitle`.
pub fn title_label(session: &SessionRow) -> String {
    match session.title.as_deref().map(str::trim) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => "Untitled session".to_string(),
    }
}

/// `resolveProjectLabel`: the name, else the id, else "Unassigned".
pub fn project_label(session: &SessionRow) -> String {
    let named = |s: &Option<String>| s.as_deref().map(str::trim).filter(|t| !t.is_empty()).map(str::to_string);
    named(&session.project_name).or_else(|| named(&session.project_id)).unwrap_or_else(|| "Unassigned".to_string())
}

/// `resolveModelsLabel`: the models joined, else "Unknown model".
pub fn models_label(models: &[String]) -> String {
    if models.is_empty() { "Unknown model".to_string() } else { models.join(", ") }
}

/// A letter without its accent or case, so "Édith" and "edith" compare equal (`sensitivity: "base"`). Covers Latin-1
/// and Latin Extended-A; ICU's full table is not reproduced.
fn base(c: char) -> char {
    let lower = c.to_lowercase().next().unwrap_or(c);
    match lower {
        'à'..='å' | 'ā' | 'ă' | 'ą' => 'a',
        'ç' | 'ć' | 'č' => 'c',
        'è'..='ë' | 'ē' | 'ę' | 'ě' => 'e',
        'ì'..='ï' | 'ī' => 'i',
        'ñ' | 'ń' | 'ň' => 'n',
        'ò'..='ö' | 'ø' | 'ō' | 'ő' => 'o',
        'ù'..='ü' | 'ū' | 'ů' | 'ű' => 'u',
        'ý' | 'ÿ' => 'y',
        'š' | 'ś' => 's',
        'ž' | 'ź' | 'ż' => 'z',
        other => other,
    }
}

/// Where a character sorts before its letter does: spaces and punctuation, then digits, then letters (as ICU lists them).
fn class(c: char) -> u8 {
    if c.is_alphabetic() {
        2
    } else if c.is_numeric() {
        1
    } else {
        0
    }
}

/// `localeCompare(b, undefined, { sensitivity: "base" })`: case and accents don't matter. A stand-in for ICU, right for
/// the titles and names a session has; two strings that differ only in case or accent are equal.
pub fn locale_compare(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a.chars().map(base), b.chars().map(base));
    loop {
        match (a.next(), b.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x == y => {}
            (Some(x), Some(y)) => return class(x).cmp(&class(y)).then(x.cmp(&y)),
        }
    }
}

fn directed(order: Ordering, dir: SortDir) -> Ordering {
    if dir == SortDir::Asc { order } else { order.reverse() }
}

/// `compareNumber`. A comparison with a number that isn't one is a tie (JS sorts a `NaN` result as zero).
fn compare_number(a: f64, b: f64, dir: SortDir) -> Ordering {
    directed(a.partial_cmp(&b).unwrap_or(Ordering::Equal), dir)
}

/// `compareNullableNumber`: a missing one goes last, whichever way the column runs.
fn compare_nullable(a: Option<f64>, b: Option<f64>, dir: SortDir) -> Ordering {
    match (a, b) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(a), Some(b)) => compare_number(a, b, dir),
    }
}

/// `compareSessions`: how two rows order under a column and a way.
pub fn compare_sessions(a: &SessionRow, b: &SessionRow, by: SortBy, dir: SortDir) -> Ordering {
    match by {
        SortBy::Title => directed(locale_compare(&title_label(a), &title_label(b)), dir),
        SortBy::Project => directed(locale_compare(&project_label(a), &project_label(b)), dir),
        SortBy::Tokens => compare_number(a.tokens, b.tokens, dir),
        SortBy::Cost => compare_number(a.cost, b.cost, dir),
        SortBy::EstimatedCost => compare_number(a.estimated_cost, b.estimated_cost, dir),
        SortBy::DurationSeconds => compare_nullable(a.duration_seconds, b.duration_seconds, dir),
        SortBy::Models => directed(locale_compare(&models_label(&a.models), &models_label(&b.models)), dir),
        SortBy::CreatedAt => match (parse_iso_ms(&a.created_at), parse_iso_ms(&b.created_at)) {
            (Some(x), Some(y)) => directed(x.cmp(&y), dir),
            _ => Ordering::Equal,
        },
    }
}

/// The rows in order (a stable sort, as `Array.prototype.sort` is).
pub fn sorted(sessions: &[SessionRow], by: SortBy, dir: SortDir) -> Vec<SessionRow> {
    let mut rows = sessions.to_vec();
    rows.sort_by(|a, b| compare_sessions(a, b, by, dir));
    rows
}
