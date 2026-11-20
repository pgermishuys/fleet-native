//! The issue filter's choices and the words for them (`composables/github-types.ts` `IssueFilterState`,
//! `lib/issue-filter-query.ts`, `lib/filter-expression.ts`). Pure: no Rune, no I/O.
//!
//! Three ways to write one filter: the struct the bar edits, the search qualifiers sent to GitHub (`issue_filter_query`), and the
//! expression a person types into the filter field (`parse_filter_expression`, `serialize_filter_expression`).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateChoice {
    Open,
    Closed,
    All,
}

impl StateChoice {
    pub fn word(self) -> &'static str {
        match self {
            StateChoice::Open => "open",
            StateChoice::Closed => "closed",
            StateChoice::All => "all",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortField {
    Created,
    Updated,
    Comments,
}

impl SortField {
    pub fn word(self) -> &'static str {
        match self {
            SortField::Created => "created",
            SortField::Updated => "updated",
            SortField::Comments => "comments",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Asc,
    Desc,
}

impl Direction {
    pub fn word(self) -> &'static str {
        match self {
            Direction::Asc => "asc",
            Direction::Desc => "desc",
        }
    }
}

/// `IssueFilterState`. `assignee` and `type` also hold `none` and `*`, as in Fleet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IssueFilter {
    pub state: StateChoice,
    pub labels: Vec<String>,
    pub milestone: Option<String>,
    pub assignee: Option<String>,
    pub author: Option<String>,
    pub kind: Option<String>,
    pub sort: SortField,
    pub direction: Direction,
    pub search: String,
}

/// `DEFAULT_ISSUE_FILTER`: open ones, most recently updated first.
pub const DEFAULT: IssueFilter = IssueFilter {
    state: StateChoice::Open,
    labels: Vec::new(),
    milestone: None,
    assignee: None,
    author: None,
    kind: None,
    sort: SortField::Updated,
    direction: Direction::Desc,
    search: String::new(),
};

impl Default for IssueFilter {
    fn default() -> Self {
        DEFAULT
    }
}

fn quote(value: &str) -> String {
    if value.chars().any(char::is_whitespace) { format!("\"{value}\"") } else { value.to_string() }
}

/// The choices as GitHub search qualifiers, e.g. `is:open label:bug sort:updated-desc`.
pub fn issue_filter_query(filter: &IssueFilter) -> String {
    let mut parts: Vec<String> = Vec::new();
    if filter.state != StateChoice::All {
        parts.push(format!("is:{}", filter.state.word()));
    }
    for label in &filter.labels {
        parts.push(format!("label:{}", quote(label)));
    }
    if let Some(author) = filter.author.as_deref().filter(|a| !a.is_empty()) {
        parts.push(format!("author:{author}"));
    }
    match filter.assignee.as_deref().filter(|a| !a.is_empty()) {
        Some("none") => parts.push("no:assignee".into()),
        Some("*") | None => {}
        Some(who) => parts.push(format!("assignee:{who}")),
    }
    if let Some(milestone) = filter.milestone.as_deref().filter(|m| !m.is_empty()) {
        parts.push(format!("milestone:{}", quote(milestone)));
    }
    if let Some(kind) = filter.kind.as_deref().filter(|t| !t.is_empty()) {
        parts.push(format!("type:{}", quote(kind)));
    }
    parts.push(format!("sort:{}-{}", filter.sort.word(), filter.direction.word()));
    if !filter.search.trim().is_empty() {
        parts.push(filter.search.trim().to_string());
    }
    parts.join(" ")
}

fn is_blank(b: u8) -> bool {
    b == b' ' || b == b'\t'
}

fn is_key(text: &str) -> bool {
    let mut chars = text.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic()) && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Splits on spaces and tabs, keeping a quoted value of a qualifier whole; an unclosed quote takes the rest.
fn tokenise(expr: &str) -> Vec<&str> {
    let bytes = expr.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if is_blank(bytes[i]) {
            i += 1;
            continue;
        }
        if let Some(colon) = expr[i..].find(':').map(|at| at + i)
            && is_key(&expr[i..colon])
        {
            let after = colon + 1;
            if bytes.get(after) == Some(&b'"') {
                match expr[after + 1..].find('"') {
                    Some(close) => {
                        let end = after + 1 + close + 1;
                        tokens.push(&expr[i..end]);
                        i = end;
                    }
                    None => {
                        tokens.push(&expr[i..]);
                        break;
                    }
                }
            } else {
                let mut end = after;
                while end < bytes.len() && !is_blank(bytes[end]) {
                    end += 1;
                }
                tokens.push(&expr[i..end]);
                i = end;
            }
            continue;
        }
        let mut end = i;
        while end < bytes.len() && !is_blank(bytes[end]) {
            end += 1;
        }
        tokens.push(&expr[i..end]);
        i = end;
    }
    tokens
}

fn unquote(value: &str) -> &str {
    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') { &value[1..value.len() - 1] } else { value }
}

/// A typed expression as a filter. A qualifier it doesn't know, or a value it can't use, stays in the search words.
pub fn parse_filter_expression(expr: &str) -> IssueFilter {
    let mut state = DEFAULT;
    if expr.trim().is_empty() {
        return state;
    }
    let mut search: Vec<&str> = Vec::new();
    for token in tokenise(expr) {
        let Some(colon) = token.find(':') else {
            search.push(token);
            continue;
        };
        let key = token[..colon].to_lowercase();
        let value = unquote(&token[colon + 1..]);
        let some = |v: &str| (!v.is_empty()).then(|| v.to_string());
        match key.as_str() {
            "is" => match value {
                "open" => state.state = StateChoice::Open,
                "closed" => state.state = StateChoice::Closed,
                "all" => state.state = StateChoice::All,
                _ => search.push(token),
            },
            "label" => {
                if !value.is_empty() && !state.labels.iter().any(|l| l == value) {
                    state.labels.push(value.to_string());
                }
            }
            "milestone" => state.milestone = some(value),
            "assignee" => state.assignee = some(value),
            "author" => state.author = some(value),
            "type" => state.kind = some(value),
            "sort" => {
                let parsed = value.rfind('-').and_then(|dash| {
                    let field = match &value[..dash] {
                        "created" => SortField::Created,
                        "updated" => SortField::Updated,
                        "comments" => SortField::Comments,
                        _ => return None,
                    };
                    let direction = match &value[dash + 1..] {
                        "asc" => Direction::Asc,
                        "desc" => Direction::Desc,
                        _ => return None,
                    };
                    Some((field, direction))
                });
                match parsed {
                    Some((field, direction)) => {
                        state.sort = field;
                        state.direction = direction;
                    }
                    None => search.push(token),
                }
            }
            _ => search.push(token),
        }
    }
    state.search = search.join(" ");
    state
}

fn quote_if_needed(value: &str) -> String {
    if value.chars().any(|c| c.is_whitespace() || c == '"' || c == ':') {
        format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        value.to_string()
    }
}

/// The filter as the expression a person would type: what is still the default is left out, the search words come last.
pub fn serialize_filter_expression(filter: &IssueFilter) -> String {
    let mut parts: Vec<String> = Vec::new();
    if filter.state != DEFAULT.state {
        parts.push(format!("is:{}", filter.state.word()));
    }
    for label in &filter.labels {
        parts.push(format!("label:{}", quote_if_needed(label)));
    }
    for (key, value) in
        [("milestone", &filter.milestone), ("assignee", &filter.assignee), ("author", &filter.author), ("type", &filter.kind)]
    {
        if let Some(value) = value {
            parts.push(format!("{key}:{}", quote_if_needed(value)));
        }
    }
    if filter.sort != DEFAULT.sort || filter.direction != DEFAULT.direction {
        parts.push(format!("sort:{}-{}", filter.sort.word(), filter.direction.word()));
    }
    if !filter.search.trim().is_empty() {
        parts.push(filter.search.trim().to_string());
    }
    parts.join(" ")
}
