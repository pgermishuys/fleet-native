//! Reading a board source's config and telling a duplicate (`BoardSourceConfig.vue` 60-199): the repository (`repository`,
//! or `owner` and `repo`), the label filter and the assignee, the list's order, the signature two sources share when they
//! sync the same thing, and the config a new source is sent with.
use super::Source;
use serde_json::{Value, json};

/// `ParsedSourceConfig`: each part, none when missing or blank. Config that isn't JSON has none of them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SourceConfig {
    pub repository: Option<String>,
    /// The labels joined with ", ".
    pub labels: Option<String>,
    pub assignee: Option<String>,
}

/// `parseSourceConfig`.
pub fn parse_config(config: &str) -> SourceConfig {
    let Ok(payload) = serde_json::from_str::<Value>(config) else { return SourceConfig::default() };
    let field = |k: &str| payload.get(k).and_then(Value::as_str);
    let repository = match (field("repository"), field("owner"), field("repo")) {
        (Some(r), _, _) => r.trim().to_string(),
        (None, Some(owner), Some(repo)) => format!("{}/{}", owner.trim(), repo.trim()),
        _ => String::new(),
    };
    SourceConfig {
        repository: (!repository.is_empty()).then_some(repository),
        labels: normalize_labels(payload.get("labels").unwrap_or(&Value::Null)),
        assignee: field("assignee").map(str::trim).filter(|s| !s.is_empty()).map(str::to_string),
    }
}

/// `normalizeLabels`: a list of strings or a comma-separated string, each trimmed, blanks dropped, joined with ", "; none
/// when nothing is left or the value is neither.
pub fn normalize_labels(value: &Value) -> Option<String> {
    let labels: Vec<&str> = match value {
        Value::Array(items) => items.iter().filter_map(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).collect(),
        Value::String(text) => text.split(',').map(str::trim).filter(|s| !s.is_empty()).collect(),
        _ => return None,
    };
    (!labels.is_empty()).then(|| labels.join(", "))
}

/// `createSourceSignature`: repository, labels and assignee, lower-cased, so the same source in other letters is a duplicate.
pub fn signature(repository: &str, labels: Option<&str>, assignee: Option<&str>) -> String {
    format!("{}::{}::{}", repository.trim().to_lowercase(), labels.unwrap_or("").to_lowercase(), assignee.unwrap_or("").to_lowercase())
}

/// `SourceViewModel`: one row of the sources list, before the last sync is put in words (a view's work).
#[derive(Clone, Debug, PartialEq)]
pub struct SourceRow {
    pub id: String,
    pub provider_type: String,
    /// The repository, else "Unknown repository".
    pub repository: String,
    pub labels: Option<String>,
    pub assignee: Option<String>,
    pub last_sync_at: Option<String>,
}

/// `sourceViewModels`: every source read, sorted by the repository's label (`localeCompare`; here a case-folded compare).
pub fn rows(sources: &[Source]) -> Vec<SourceRow> {
    let mut rows: Vec<SourceRow> = sources
        .iter()
        .map(|s| {
            let config = parse_config(&s.config);
            SourceRow {
                id: s.id.clone(),
                provider_type: s.provider_type.clone(),
                repository: config.repository.unwrap_or_else(|| "Unknown repository".to_string()),
                labels: config.labels,
                assignee: config.assignee,
                last_sync_at: s.last_sync_at.clone(),
            }
        })
        .collect();
    rows.sort_by(|a, b| a.repository.to_lowercase().cmp(&b.repository.to_lowercase()).then_with(|| a.repository.cmp(&b.repository)));
    rows
}

/// `isDuplicateSource`: whether the form's repository, label filter and "Assigned to me" are already a listed source. A
/// blank repository is never one.
pub fn is_duplicate(sources: &[Source], repository: &str, label_filter: &str, assigned_to_me: bool) -> bool {
    if repository.trim().is_empty() {
        return false;
    }
    let wanted = signature(repository, normalize_labels(&json!(label_filter)).as_deref(), assigned_to_me.then_some(ASSIGNED_TO_ME));
    rows(sources).iter().any(|r| signature(&r.repository, r.labels.as_deref(), r.assignee.as_deref()) == wanted)
}

/// What "Assigned to me" sends.
pub const ASSIGNED_TO_ME: &str = "@me";

/// `handleAddSource`'s config, as the text the server stores: the trimmed repository, the labels only when some are left
/// ("a, b"), the assignee only when "Assigned to me" is on.
pub fn add_config(repository: &str, label_filter: &str, assigned_to_me: bool) -> String {
    // Written by hand to keep Fleet's key order (`JSON.stringify`): a `Map` may sort its keys.
    let text = |s: &str| Value::String(s.to_string()).to_string();
    let mut parts = vec![format!("\"repository\":{}", text(repository.trim()))];
    if let Some(labels) = normalize_labels(&json!(label_filter)) {
        parts.push(format!("\"labels\":{}", text(&labels)));
    }
    if assigned_to_me {
        parts.push(format!("\"assignee\":{}", text(ASSIGNED_TO_ME)));
    }
    format!("{{{}}}", parts.join(","))
}
