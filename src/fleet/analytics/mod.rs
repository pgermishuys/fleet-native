//! Analytics (component 20, `docs/parity/20-analytics.md`). Plain Rust, no Rune.
//!
//! This file holds what the four reads answer (`api/client.ts`: `AnalyticsSummary`, `DailyAnalytics`, `ModelAnalytics`,
//! `SessionAnalytics`), the query each read sends (`use-analytics-{summary,daily,models,sessions}.ts`,
//! `use-analytics-menu.ts`'s `exportHref`) and the words a section says when nothing matched (`AnalyticsContent.vue`).
//! The sections, the filters and the sessions table's order are in the files beside it.
//!
//! A shape is read the way the client uses it: a field that is missing or `null` is zero, empty or none (Fleet casts
//! the body and shows `undefined` as nothing), and a field this app doesn't know is ignored.
use serde_json::Value;

pub mod calendar;
pub mod export;
pub mod fields;
pub mod filters;
pub mod nav;
pub mod sections;
pub mod sessions_sort;
pub mod store;

use filters::Filters;
use sessions_sort::{SortBy, SortDir};

/// `AnalyticsTopItem`: a project or a model in the summary's top five.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TopItem {
    pub name: String,
    pub tokens: f64,
    pub cost: f64,
}

/// `AnalyticsSummary`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Summary {
    pub total_tokens: f64,
    pub total_cost: f64,
    pub total_estimated_cost: f64,
    pub session_count: f64,
    pub message_count: f64,
    pub top_models: Vec<TopItem>,
    pub top_projects: Vec<TopItem>,
}

/// `DailyAnalytics`: one day that had events.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Daily {
    /// "2026-11-20", the UTC day.
    pub date: String,
    pub tokens: f64,
    pub cost: f64,
    pub estimated_cost: f64,
    pub sessions: f64,
    pub messages: f64,
}

/// `ModelAnalytics`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModelRow {
    pub model_id: String,
    pub provider_id: String,
    pub tokens: f64,
    pub cost: f64,
    pub estimated_cost: f64,
    pub message_count: f64,
    pub avg_cost_per_message: f64,
}

/// `SessionAnalytics`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SessionRow {
    pub session_id: String,
    pub title: Option<String>,
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    pub tokens: f64,
    pub cost: f64,
    pub estimated_cost: f64,
    pub models: Vec<String>,
    /// `null` for a session with no end to measure to.
    pub duration_seconds: Option<f64>,
    /// ISO text, as the server wrote it.
    pub created_at: String,
}

fn num(v: &Value, key: &str) -> f64 {
    v.get(key).and_then(Value::as_f64).unwrap_or(0.0)
}

fn text(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or_default().to_string()
}

fn opt_text(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

fn top_items(v: &Value, key: &str) -> Vec<TopItem> {
    let Some(list) = v.get(key).and_then(Value::as_array) else { return Vec::new() };
    list.iter()
        .filter(|item| item.is_object())
        .map(|item| TopItem { name: text(item, "name"), tokens: num(item, "tokens"), cost: num(item, "cost") })
        .collect()
}

/// The summary's body. Anything but an object is no answer.
pub fn parse_summary(body: &Value) -> Option<Summary> {
    body.is_object().then(|| Summary {
        total_tokens: num(body, "totalTokens"),
        total_cost: num(body, "totalCost"),
        total_estimated_cost: num(body, "totalEstimatedCost"),
        session_count: num(body, "sessionCount"),
        message_count: num(body, "messageCount"),
        top_models: top_items(body, "topModels"),
        top_projects: top_items(body, "topProjects"),
    })
}

/// The rows of a list answer; anything but an array is no answer, and an entry that isn't an object is skipped.
fn rows<T>(body: &Value, read: impl Fn(&Value) -> Option<T>) -> Option<Vec<T>> {
    Some(body.as_array()?.iter().filter(|row| row.is_object()).filter_map(read).collect())
}

/// The daily body, in the order the server sent it. A row with no date is skipped: it has no day to stand for.
pub fn parse_days(body: &Value) -> Option<Vec<Daily>> {
    rows(body, |d| {
        Some(Daily {
            date: d.get("date")?.as_str()?.to_string(),
            tokens: num(d, "tokens"),
            cost: num(d, "cost"),
            estimated_cost: num(d, "estimatedCost"),
            sessions: num(d, "sessions"),
            messages: num(d, "messages"),
        })
    })
}

pub fn parse_models(body: &Value) -> Option<Vec<ModelRow>> {
    rows(body, |m| {
        Some(ModelRow {
            model_id: text(m, "modelId"),
            provider_id: text(m, "providerId"),
            tokens: num(m, "tokens"),
            cost: num(m, "cost"),
            estimated_cost: num(m, "estimatedCost"),
            message_count: num(m, "messageCount"),
            avg_cost_per_message: num(m, "avgCostPerMessage"),
        })
    })
}

pub fn parse_sessions(body: &Value) -> Option<Vec<SessionRow>> {
    rows(body, |s| {
        Some(SessionRow {
            session_id: text(s, "sessionId"),
            title: opt_text(s, "title"),
            project_id: opt_text(s, "projectId"),
            project_name: opt_text(s, "projectName"),
            tokens: num(s, "tokens"),
            cost: num(s, "cost"),
            estimated_cost: num(s, "estimatedCost"),
            models: s
                .get("models")
                .and_then(Value::as_array)
                .map_or_else(Vec::new, |l| l.iter().filter_map(|m| m.as_str().map(str::to_string)).collect()),
            duration_seconds: s.get("durationSeconds").and_then(Value::as_f64),
            created_at: text(s, "createdAt"),
        })
    })
}

// ─── the requests ───────────────────────────────────────────────────────────

/// A query value as `URLSearchParams` writes it: spaces are `+`, and anything but a letter, digit or `*-._` is `%XX`.
fn encode(value: &str) -> String {
    let mut out = String::new();
    for b in value.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'*' | b'-' | b'.' | b'_' => out.push(b as char),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// `path?a=1&b=2` of the pairs whose value isn't empty (`if (from) query.set(...)`).
fn with_query(path: &str, pairs: &[(&str, &str)]) -> String {
    let query: Vec<String> = pairs.iter().filter(|(_, v)| !v.is_empty()).map(|(k, v)| format!("{k}={}", encode(v))).collect();
    if query.is_empty() { path.to_string() } else { format!("{path}?{}", query.join("&")) }
}

/// The project filter as the reads send it: trimmed, and nothing when blank (`filters.projectId.trim() || undefined`).
fn project(filters: &Filters) -> &str {
    filters.project_id.trim()
}

/// `GET /api/analytics/summary`: the page's, and with `range_only` the menu's (no project: it lists the range's projects).
pub fn summary_path(filters: &Filters, range_only: bool) -> String {
    let project = if range_only { "" } else { project(filters) };
    with_query("/api/analytics/summary", &[("from", &filters.from), ("to", &filters.to), ("projectId", project)])
}

pub fn daily_path(filters: &Filters) -> String {
    with_query("/api/analytics/daily", &[("from", &filters.from), ("to", &filters.to), ("projectId", project(filters))])
}

/// The models read puts the project between the dates; the server ignores it (row 11.5).
pub fn models_path(filters: &Filters) -> String {
    with_query("/api/analytics/models", &[("from", &filters.from), ("projectId", project(filters)), ("to", &filters.to)])
}

/// The page asks for the sort it shows; the server ignores it and sends the newest 50 (rows 5.1, 5.10).
pub fn sessions_path(filters: &Filters, by: SortBy, dir: SortDir) -> String {
    with_query(
        "/api/analytics/sessions",
        &[("from", &filters.from), ("to", &filters.to), ("projectId", project(filters)), ("sortBy", by.key()), ("sortDir", dir.key())],
    )
}

/// `exportHref`: the CSV of the same range and project.
pub fn export_path(filters: &Filters) -> String {
    with_query("/api/analytics/export", &[("format", "csv"), ("from", &filters.from), ("to", &filters.to), ("projectId", project(filters))])
}

// ─── the words ──────────────────────────────────────────────────────────────

/// What a section says when its read answered with nothing; `with_project` is whether a project is picked.
pub fn daily_empty(with_project: bool) -> &'static str {
    if with_project {
        "No daily analytics matched the selected date range and project."
    } else {
        "No daily analytics matched the selected date range."
    }
}

pub fn models_empty(with_project: bool) -> &'static str {
    if with_project {
        "No model analytics matched the selected date range and project."
    } else {
        "No model analytics matched the selected date range."
    }
}

pub fn projects_empty(with_project: bool) -> &'static str {
    if with_project {
        "No projects matched the selected date range and project."
    } else {
        "No project analytics matched the selected date range."
    }
}

/// Sessions' own words, which don't change with the filters (`SessionsTab.vue`).
pub const SESSIONS_EMPTY: &str = "No sessions matched the current analytics filters";

/// A project in the filter's menu: its id is its name (row 3.7).
pub type ProjectOption = TopItem;

/// `withSelectedProject`: the range's top projects, and the picked project after them when it isn't among them
/// (zero tokens, zero cost), so the select can still show it. A blank pick adds nothing.
pub fn with_selected_project(projects: &[TopItem], selected: &str) -> Vec<ProjectOption> {
    let mut options: Vec<ProjectOption> = Vec::new();
    for project in projects {
        // A later project with the same id replaces the earlier one in its place, as a `Map` does.
        match options.iter_mut().find(|o| o.name == project.name) {
            Some(existing) => *existing = project.clone(),
            None => options.push(project.clone()),
        }
    }
    if !selected.is_empty() && !options.iter().any(|o| o.name == selected) {
        options.push(TopItem { name: selected.to_string(), tokens: 0.0, cost: 0.0 });
    }
    options
}
