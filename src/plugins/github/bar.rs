//! The issue filter bar (`components/IssueFilterBar.vue`, `FilterExpressionField.vue` and `filters/*`, rows 9.x of
//! docs/parity/21-github.md): what the bar's field holds while it is being edited, which of its menus is open, and what each choice
//! does to the filter the Issues tab searches with. The bar itself is `view::filters`. Pure but for `keep_github_bar`, which runs
//! from the repository page's `keep`.
use super::filter::{DEFAULT, Direction, IssueFilter, SortField, StateChoice, parse_filter_expression, serialize_filter_expression};
use super::{Where, state, state_ref};
use crate::{State, Tx};
use rune::TextEdit;

/// The expression field's key.
pub const EXPR_INPUT: &str = "github-issues-expr";
/// The search box of the open menu.
pub const MENU_INPUT: &str = "github-issues-menu-search";

/// The bar's five menus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Menu {
    Label,
    Author,
    Milestone,
    Assignee,
    Sort,
}

impl Menu {
    /// The key of the button that opens it (the menu anchors to it).
    pub fn trigger(self) -> &'static str {
        match self {
            Menu::Label => "github-issues-label",
            Menu::Author => "github-issues-author",
            Menu::Milestone => "github-issues-milestone",
            Menu::Assignee => "github-issues-assignee",
            Menu::Sort => "github-issues-sort",
        }
    }

    /// The popover's width (the sort menu's is what Fleet's draws, 189 where its CSS says 192). Fleet's CSS says 256 for labels and 224 for the rest, but `PopoverContent`'s own `w-72` (252) wins in the
    /// real client, and so does its `p-4` over the `padding: 0` the components ask for.
    pub fn width(self) -> f32 {
        if self == Menu::Sort { 189.0 } else { 252.0 }
    }

    /// The box's placeholder.
    pub fn placeholder(self) -> &'static str {
        match self {
            Menu::Label => "Filter labels\u{2026}",
            Menu::Author => "Filter authors\u{2026}",
            Menu::Milestone => "Filter milestones\u{2026}",
            Menu::Assignee => "Filter assignees\u{2026}",
            Menu::Sort => "",
        }
    }

    /// `CommandEmpty`'s words.
    pub fn empty_words(self) -> &'static str {
        match self {
            Menu::Label => "No labels found.",
            Menu::Author => "No users found.",
            Menu::Milestone => "No milestones found.",
            Menu::Assignee => "No assignees found.",
            Menu::Sort => "",
        }
    }
}

/// `SORT_OPTIONS`, in the order the menu lists them: the words, the field and the direction.
pub const SORTS: [(&str, SortField, Direction); 5] = [
    ("Recently updated", SortField::Updated, Direction::Desc),
    ("Least recently updated", SortField::Updated, Direction::Asc),
    ("Newest", SortField::Created, Direction::Desc),
    ("Oldest", SortField::Created, Direction::Asc),
    ("Most commented", SortField::Comments, Direction::Desc),
];

/// The current sort's words (`currentLabel`), "Sort" for one that isn't in the list.
pub fn sort_words(f: &IssueFilter) -> &'static str {
    SORTS.iter().find(|(_, field, dir)| *field == f.sort && *dir == f.direction).map_or("Sort", |(words, _, _)| words)
}

/// One row of a menu (`CommandItem`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// cmdk's `value`: what the search matches, and what a press picks.
    pub value: String,
    pub title: String,
    pub look: Look,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Look {
    /// "Clear selection", muted, first while something is chosen.
    Clear,
    /// A label: the checkbox, the dot in its colour (`RRGGBB`).
    Label { color: String, checked: bool },
    /// A person: the check (shown on the chosen one), a 16 px avatar, the login.
    User { checked: bool },
    /// "No milestone" and "Unassigned".
    Plain { checked: bool },
    /// A milestone: the check, the title and "{open} open · {closed} closed" under it.
    Milestone { checked: bool, meta: String },
}

#[derive(Debug, Default)]
pub struct Bar {
    pub menu: Option<Menu>,
    /// The open menu's search box.
    pub query: TextEdit,
    /// The highlighted row of the open menu; past the end while none is (the pointer left the list).
    pub hl: usize,
    /// What the expression field holds: the serialised filter, or what is being typed while it has focus (`editingValue`).
    pub expr: TextEdit,
    pub editing: bool,
}

impl Bar {
    /// Closes the open menu and forgets what was typed in it (`PopoverContent` unmounts).
    pub fn close(&mut self) {
        self.menu = None;
        self.query = TextEdit::default();
        self.hl = 0;
    }
}

/// cmdk matches what is typed against a row's `value`, anywhere in it, whatever the case.
fn matches(value: &str, typed: &str) -> bool {
    typed.trim().is_empty() || value.to_lowercase().contains(&typed.trim().to_lowercase())
}

/// `select` as `AuthorFilter` has it: the chosen one again clears, another one replaces it.
fn pick(current: &Option<String>, value: &str) -> Option<String> {
    if current.as_deref() == Some(value) { None } else { Some(value.to_string()) }
}

impl State {
    fn bar(&mut self) -> &mut Bar {
        &mut state(&mut self.places).repo.bar
    }

    /// The filter a control changes. A field still being edited is committed first: leaving it is what commits it in Fleet, and a
    /// press on a control leaves it.
    fn issues(&mut self) -> &mut IssueFilter {
        if state_ref(&self.places).repo.bar.editing {
            self.github_bar_commit();
        }
        &mut state(&mut self.places).repo.issues
    }

    /// The rows of the open menu, with what is typed applied; loading and the empty words are the view's.
    pub fn github_bar_rows(&self) -> Vec<Row> {
        let g = state_ref(&self.places);
        let (Some(menu), Some((owner, repo))) = (g.repo.bar.menu, at_repo(&g.at)) else {
            return Vec::new();
        };
        let f = &g.repo.issues;
        let typed = g.repo.bar.query.text.as_str();
        let mut rows: Vec<Row> = Vec::new();
        let has = |chosen: &Option<String>| chosen.as_deref().is_some_and(|c| !c.is_empty());
        let clear = |rows: &mut Vec<Row>| rows.push(Row { value: "__clear__".into(), title: "Clear selection".into(), look: Look::Clear });
        let people = |rows: &mut Vec<Row>, chosen: &Option<String>| {
            for a in g.metadata.assignees.get(&owner, &repo).map(|e| e.data.as_slice()).unwrap_or_default() {
                rows.push(Row {
                    value: a.login.clone(),
                    title: a.login.clone(),
                    look: Look::User { checked: chosen.as_deref() == Some(a.login.as_str()) },
                });
            }
        };
        match menu {
            Menu::Label => {
                for l in g.metadata.labels.get(&owner, &repo).map(|e| e.data.as_slice()).unwrap_or_default() {
                    rows.push(Row {
                        value: l.name.clone(),
                        title: l.name.clone(),
                        look: Look::Label { color: l.color.clone(), checked: f.labels.contains(&l.name) },
                    });
                }
            }
            Menu::Author => {
                if has(&f.author) {
                    clear(&mut rows);
                }
                people(&mut rows, &f.author);
            }
            Menu::Milestone => {
                if has(&f.milestone) {
                    clear(&mut rows);
                }
                rows.push(Row {
                    value: "__none__".into(),
                    title: "No milestone".into(),
                    look: Look::Plain { checked: f.milestone.as_deref() == Some("none") },
                });
                for m in g.metadata.milestones.get(&owner, &repo).map(|e| e.data.as_slice()).unwrap_or_default() {
                    rows.push(Row {
                        value: m.title.clone(),
                        title: m.title.clone(),
                        look: Look::Milestone {
                            checked: f.milestone.as_deref() == Some(m.title.as_str()),
                            meta: format!("{} open \u{b7} {} closed", m.open_issues, m.closed_issues),
                        },
                    });
                }
            }
            Menu::Assignee => {
                if has(&f.assignee) {
                    clear(&mut rows);
                }
                rows.push(Row {
                    value: "__none__".into(),
                    title: "Unassigned".into(),
                    look: Look::Plain { checked: f.assignee.as_deref() == Some("none") },
                });
                people(&mut rows, &f.assignee);
            }
            Menu::Sort => return Vec::new(),
        }
        rows.retain(|r| matches(&r.value, typed));
        rows
    }

    /// Whether the open menu's list is still being read with nothing yet (`isLoading && length === 0`) and whether it has nothing to
    /// show (`!isLoading && length === 0`): the first is the spinner, the second the empty words.
    pub fn github_bar_list_state(&self) -> (bool, bool) {
        let g = state_ref(&self.places);
        let (Some(menu), Some((owner, repo))) = (g.repo.bar.menu, at_repo(&g.at)) else {
            return (false, false);
        };
        let (loading, count) = match menu {
            Menu::Label => g.metadata.labels.get(&owner, &repo).map_or((false, 0), |e| (e.loading, e.data.len())),
            Menu::Milestone => g.metadata.milestones.get(&owner, &repo).map_or((false, 0), |e| (e.loading, e.data.len())),
            Menu::Author | Menu::Assignee => g.metadata.assignees.get(&owner, &repo).map_or((false, 0), |e| (e.loading, e.data.len())),
            Menu::Sort => return (false, false),
        };
        (loading && count == 0, !loading && count == 0)
    }

    /// A press on button `menu`: opens it, or closes it when it is the open one. Its box starts empty.
    pub fn github_bar_toggle(&mut self, menu: Menu) {
        let bar = self.bar();
        let was = bar.menu;
        bar.close();
        if was != Some(menu) {
            bar.menu = Some(menu);
        }
    }

    pub fn github_bar_close(&mut self) {
        self.bar().close();
    }

    /// The list under a new search: the first row is the highlighted one again.
    pub fn github_bar_typed(&mut self) {
        self.bar().hl = 0;
    }

    /// Up and Down in the menu's box (cmdk wraps); the new highlighted row.
    pub fn github_bar_step(&mut self, by: i32) -> Option<usize> {
        let n = self.github_bar_rows().len();
        if n == 0 {
            return None;
        }
        let bar = self.bar();
        // With no row highlighted (the pointer left the list) Down goes to the first and Up to the last.
        bar.hl = if bar.hl >= n { if by > 0 { 0 } else { n - 1 } } else { (bar.hl as i64 + by as i64).rem_euclid(n as i64) as usize };
        Some(bar.hl)
    }

    /// Enter in the menu's box.
    pub fn github_bar_pick_highlighted(&mut self) {
        let at = self.bar().hl;
        self.github_bar_pick(at);
    }

    /// A press on row `at` of the open menu (`@select`).
    pub fn github_bar_pick(&mut self, at: usize) {
        let Some(row) = self.github_bar_rows().into_iter().nth(at) else { return };
        let Some(menu) = self.bar().menu else { return };
        let clear = row.look == Look::Clear;
        match menu {
            Menu::Label => {
                let f = self.issues();
                if let Some(i) = f.labels.iter().position(|l| *l == row.value) {
                    f.labels.remove(i);
                } else {
                    f.labels.push(row.value);
                }
                self.github_bar_sync();
                return;
            }
            Menu::Author => {
                let f = self.issues();
                f.author = if clear { None } else { pick(&f.author, &row.value) };
            }
            Menu::Milestone => {
                let f = self.issues();
                let value = if row.value == "__none__" { "none".to_string() } else { row.value };
                f.milestone = if clear { None } else { pick(&f.milestone, &value) };
            }
            Menu::Assignee => {
                let f = self.issues();
                let value = if row.value == "__none__" { "none".to_string() } else { row.value };
                f.assignee = if clear { None } else { pick(&f.assignee, &value) };
            }
            Menu::Sort => return,
        }
        self.bar().close();
        self.github_bar_sync();
    }

    /// A press on a sort option (`handleSortChange`).
    pub fn github_bar_sort(&mut self, field: SortField, direction: Direction) {
        let f = self.issues();
        f.sort = field;
        f.direction = direction;
        self.bar().close();
        self.github_bar_sync();
    }

    /// Open or Closed (`setFilter({ state })`).
    pub fn github_bar_state(&mut self, choice: StateChoice) {
        self.issues().state = choice;
        self.github_bar_sync();
    }

    /// `handleClear`: the defaults.
    pub fn github_bar_clear(&mut self) {
        *self.issues() = DEFAULT;
        let bar = self.bar();
        bar.editing = false;
        bar.expr.replace_text("");
    }

    /// `commit`: what the field holds, parsed, becomes the filter.
    pub fn github_bar_commit(&mut self) {
        let parsed = parse_filter_expression(&self.bar().expr.text.clone());
        state(&mut self.places).repo.issues = parsed;
        self.bar().editing = false;
    }

    /// Escape: the edit is dropped, the field shows the filter again.
    pub fn github_bar_drop_edit(&mut self) {
        let shown = serialize_filter_expression(&state_ref(&self.places).repo.issues);
        let bar = self.bar();
        bar.editing = false;
        bar.expr.replace_text(&shown);
    }

    /// With no edit under way the field shows the serialised filter.
    fn github_bar_sync(&mut self) -> bool {
        if state_ref(&self.places).repo.bar.editing {
            return false;
        }
        let shown = serialize_filter_expression(&state_ref(&self.places).repo.issues);
        if self.bar().expr.text == shown {
            return false;
        }
        self.bar().expr.replace_text(&shown);
        true
    }

    /// `serialized.length > 0`: the Clear button shows.
    pub fn github_bar_has_filters(&self) -> bool {
        !serialize_filter_expression(&state_ref(&self.places).repo.issues).is_empty()
    }

    /// What the bar does while the Issues tab is on screen: focusing the field starts an edit; leaving it commits; with no edit the
    /// field shows the serialised filter. True when something drawn changed.
    pub(super) fn keep_github_bar(&mut self, tx: &Tx) -> bool {
        let focused = tx.focused() == Some(EXPR_INPUT);
        let editing = state_ref(&self.places).repo.bar.editing;
        if focused {
            if !editing {
                self.bar().editing = true;
            }
            return false;
        }
        let mut changed = false;
        if editing {
            self.github_bar_commit();
            changed = true;
        }
        changed | self.github_bar_sync()
    }
}

/// The repository the page is on.
fn at_repo(at: &Where) -> Option<(String, String)> {
    match at {
        Where::Repo { owner, repo } => Some((owner.clone(), repo.clone())),
        _ => None,
    }
}
