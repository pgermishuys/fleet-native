//! `lib/mods/check-summary.ts` (component 22, piece I): what a draft touches, in plain words, from the static check's report, and
//! the first error and the warning count. The report is the server's JSON (`ModCheckReport`), read as it comes.
use serde_json::Value;

/// One line of the review dialog's "what this mod touches" list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckRow {
    pub key: String,
    pub text: String,
    /// Something Fleet can't put in words, shown in code formatting after the text.
    pub code: Option<String>,
    /// The Stage 1 constants ("None"): shown quietly.
    pub muted: bool,
}

fn site_words(site: &str) -> Option<&'static str> {
    match site {
        "ComposerBand" => Some("A band above the composer"),
        "StatusChip" => Some("A chip in the status bar"),
        "Pane" => Some("A pane it opens"),
        _ => None,
    }
}

fn listens(event: &str) -> Option<&'static str> {
    match event {
        "ui.press" | "ui.input" | "ui.select" => Some("Buttons and fields it draws"),
        "session.start" => Some("When a session opens"),
        "turn.complete" => Some("When a turn ends"),
        _ => None,
    }
}

fn regex_of(v: &Value) -> Option<&str> {
    v.as_object()?.get("$regex")?.as_str()
}

/// The values a matcher field allows, as plain words; none when the field isn't a shape we know.
fn words(v: &Value) -> Option<Vec<String>> {
    if let Some(s) = v.as_str() {
        return Some(vec![s.to_string()]);
    }
    if let Some(re) = regex_of(v) {
        let flags = v.get("flags").and_then(Value::as_str).unwrap_or("");
        return Some(vec![format!("matching /{re}/{flags}")]);
    }
    let items = v.as_array().filter(|a| !a.is_empty())?;
    let mut all = Vec::new();
    for item in items {
        if !(item.is_string() || regex_of(item).is_some()) {
            return None;
        }
        all.extend(words(item)?);
    }
    Some(all)
}

fn join_lower(parts: &[String], glue: &str) -> String {
    let lowered: Vec<String> = parts
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let mut chars = p.chars();
            match (i, chars.next()) {
                (0, _) | (_, None) => p.clone(),
                (_, Some(c)) => c.to_lowercase().chain(chars).collect(),
            }
        })
        .collect();
    lowered.join(glue)
}

fn list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [init @ .., last] => format!("{} and {last}", init.join(", ")),
    }
}

type Described = (String, Option<String>);

fn raw(matcher: &Value) -> Described {
    ("A place Fleet can't put in words:".into(), Some(matcher.to_string()))
}

fn tool_rows(components: &[&str], props: Option<&Value>, matcher: &Value) -> Described {
    let both = components.contains(&"ToolUse") && components.contains(&"ToolResult");
    let wher = if both {
        " (on the line, and the opened body)"
    } else if components.first() == Some(&"ToolUse") {
        " (on the line)"
    } else {
        " (in the opened body)"
    };
    let Some(props) = props else { return (format!("Rows for every tool call{wher}"), None) };
    let tools = props.as_object().filter(|o| o.len() == 1).and_then(|o| o.get("tool")).and_then(words);
    match tools {
        Some(tools) => (format!("Rows for {} calls{wher}", list(&tools)), None),
        None => raw(matcher),
    }
}

fn describe_render(matcher: Option<&Value>) -> Vec<Described> {
    let Some(matcher) = matcher.filter(|m| !m.is_null()) else { return vec![("Every place a mod can draw".into(), None)] };
    let Some(object) = matcher.as_object() else { return vec![raw(matcher)] };
    let components = object.get("component").and_then(words);
    let Some(components) = components.filter(|_| object.keys().all(|k| k == "component" || k == "props")) else {
        return vec![raw(matcher)];
    };
    let is_tool = |n: &&str| *n == "ToolUse" || *n == "ToolResult";
    let tools: Vec<&str> = components.iter().map(String::as_str).filter(is_tool).collect();
    let sites: Vec<&str> = components.iter().map(String::as_str).filter(|n| !is_tool(n)).collect();
    // Props narrow the tool rows only; for any other site they're something Fleet can't put in words.
    if object.contains_key("props") && !sites.is_empty() {
        return vec![raw(matcher)];
    }
    let mut described = Vec::new();
    if !tools.is_empty() {
        described.push(tool_rows(&tools, object.get("props"), matcher));
    }
    for site in sites {
        described.push(match site_words(site) {
            Some(w) => (w.to_string(), None),
            None => raw(&serde_json::json!({ "component": site })),
        });
    }
    described
}

#[derive(PartialEq)]
enum Reach {
    None,
    Some,
    Every,
}

/// Which tool rows a render hook can be asked to draw, and so read. Anything Fleet can't read as plain site names counts as every
/// call, so the review never says less than it can.
fn tool_reach(hook: &Value) -> Reach {
    if hook.get("event").and_then(Value::as_str) != Some("ui.render") {
        return Reach::None;
    }
    let Some(matcher) = hook.get("matcher").and_then(Value::as_object) else { return Reach::Every };
    let named: Option<Vec<&str>> = match matcher.get("component") {
        Some(Value::String(s)) => Some(vec![s.as_str()]),
        Some(Value::Array(a)) => a.iter().map(Value::as_str).collect(),
        _ => None,
    };
    if matcher.get("component").is_some() && named.is_none() {
        return Reach::Every;
    }
    if named.is_some_and(|n| !n.iter().any(|n| *n == "ToolUse" || *n == "ToolResult")) {
        return Reach::None;
    }
    match matcher.get("props").and_then(Value::as_object).and_then(|p| p.get("tool")).and_then(words) {
        Some(_) => Reach::Some,
        None => Reach::Every,
    }
}

fn strings(report: &Value, key: &str) -> Vec<String> {
    report.get(key).and_then(Value::as_array).map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

fn row(key: &str, text: impl Into<String>) -> CheckRow {
    CheckRow { key: key.to_string(), text: text.into(), code: None, muted: false }
}

/// `summarizeCheck`: what a mod touches, in plain words. Rows that don't apply are left out.
pub fn summarize_check(report: &Value) -> Vec<CheckRow> {
    let mut rows = Vec::new();
    let calls = strings(report, "calls");
    let has = |c: &str| calls.iter().any(|x| x == c);
    let hooks: Vec<&Value> = report.get("hooks").and_then(Value::as_array).map(|a| a.iter().collect()).unwrap_or_default();
    let titles: Vec<String> = report
        .get("panes")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|p| p.get("title").and_then(Value::as_str).map(str::to_string)).collect())
        .unwrap_or_default();

    // A declared pane is said under "Adds"; "A pane it opens" would say the same thing twice.
    let pane_words = site_words("Pane").unwrap_or_default();
    let drawn = hooks
        .iter()
        .filter(|h| h.get("event").and_then(Value::as_str) == Some("ui.render"))
        .flat_map(|h| describe_render(h.get("matcher")))
        .filter(|(text, _)| titles.is_empty() || text != pane_words);
    for (i, (text, code)) in drawn.enumerate() {
        rows.push(CheckRow { code, ..row(&if i == 0 { "draws".to_string() } else { format!("draws-{i}") }, text) });
    }
    if !titles.is_empty() {
        rows.push(row("adds", format!("A pane in the + menu: {}", list(&titles))));
    }

    let mut heard: Vec<String> = Vec::new();
    for w in hooks.iter().filter_map(|h| h.get("event").and_then(Value::as_str)).filter_map(listens) {
        if !heard.iter().any(|x| x == w) {
            heard.push(w.to_string());
        }
    }
    if !heard.is_empty() {
        rows.push(row("listens", join_lower(&heard, ", ")));
    }

    let mut reads: Vec<String> = Vec::new();
    let reach: Vec<Reach> = hooks.iter().map(|h| tool_reach(h)).collect();
    if reach.contains(&Reach::Every) {
        reads.push("Tool input and output of every tool call".into());
    } else if reach.contains(&Reach::Some) {
        reads.push("Tool input and output of those calls".into());
    }
    if calls.iter().any(|c| c.starts_with("session.")) {
        reads.push("Session details (title, folder, harness)".into());
    }
    if !reads.is_empty() {
        rows.push(row("reads", join_lower(&reads, "; ")));
    }

    let state = strings(report, "state");
    if !state.is_empty() {
        rows.push(row("state", format!("Keeps per-session state: {}", state.join(", "))));
    }
    if has("store.set") || has("store.delete") {
        rows.push(row("store", "Saves data for you on this machine"));
    } else if has("store.get") || has("store.keys") {
        rows.push(row("store", "Reads its saved data"));
    }
    let pages = strings(report, "pages");
    if !pages.is_empty() {
        rows.push(row("pages", format!("Shows its own pages: {}", pages.join(", "))));
    }

    let mut also: Vec<String> = Vec::new();
    if has("ui.toast") {
        also.push("Shows notices".into());
    }
    if has("ui.open") {
        also.push("Opens panes".into());
    }
    if has("clock.every") || has("clock.after") {
        also.push("Runs timers".into());
    }
    if !also.is_empty() {
        rows.push(row("also", join_lower(&also, ", ")));
    }

    // Stage 1 mods can't do these; the rows say so instead of leaving the question open.
    for (key, text) in [("changes", "Nothing the agent sees"), ("network", "None"), ("files", "None")] {
        rows.push(CheckRow { muted: true, ..row(key, text) });
    }
    rows
}

/// The label of a row's key; several draw rows share the first one's (`draws-1`…).
pub fn label_of(key: &str) -> &str {
    let base = key.rsplit_once('-').filter(|(_, n)| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())).map_or(key, |(b, _)| b);
    match base {
        "draws" => "Draws",
        "adds" => "Adds",
        "listens" => "Reacts to",
        "reads" => "Reads",
        "state" => "Remembers",
        "store" => "Saves",
        "pages" => "Pages",
        "also" => "Also",
        "changes" => "Changes",
        "network" => "Network",
        "files" => "Files",
        _ => key,
    }
}

/// The first error of a report, with where it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FirstError {
    pub message: String,
    pub line: Option<i64>,
    pub column: Option<i64>,
    /// "12:3", "12", or none without a line.
    pub place: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Problems {
    pub error: Option<FirstError>,
    pub warnings: usize,
}

/// `checkProblems`: the first error (with where it is) and how many warnings, for the card and the dialog.
pub fn check_problems(report: Option<&Value>) -> Problems {
    let Some(report) = report else { return Problems::default() };
    let list = |k: &str| report.get(k).and_then(Value::as_array).cloned().unwrap_or_default();
    let error = list("errors").first().map(|first| {
        let int = |k: &str| first.get(k).and_then(Value::as_i64);
        let (line, column) = (int("line"), int("column"));
        let place = line.map(|l| column.map_or_else(|| l.to_string(), |c| format!("{l}:{c}")));
        FirstError { message: first.get("message").and_then(Value::as_str).unwrap_or_default().to_string(), line, column, place }
    });
    Problems { error, warnings: list("warnings").len() }
}

/// The warnings' messages, joined as the dialog says them.
pub fn warning_words(report: &Value) -> String {
    let all = report.get("warnings").and_then(Value::as_array).cloned().unwrap_or_default();
    all.iter().map(|w| w.get("message").and_then(Value::as_str).unwrap_or_default()).collect::<Vec<_>>().join("; ")
}
