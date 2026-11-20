//! What the editor offers as you type (`lib/code-editor/completion.ts`, row 13.11 of docs/parity/11-files.md): the words already in
//! the file (CodeMirror's `completeAnyWord`) and import paths (`from "./`, `import("../`, `require("@/`) completed one folder at a
//! time. Plain Rust with no state: the keys and the popup are `fleet::completion` and `view::completion`.
//!
//! Not here: the language's own keywords and snippets, which CodeMirror's language packages bring and Rune's highlighter doesn't
//! have (a gap in the checklist).
use regex::Regex;
use std::sync::OnceLock;

/// What a suggestion is, for its icon and its sort (`type` in CodeMirror).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Word,
    File,
    Folder,
}

/// One suggestion: the text it puts in, and a note at its right (`detail`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub label: String,
    pub detail: Option<String>,
    pub kind: Kind,
    /// `boost`: folders sort above files.
    pub boost: i32,
}

/// A suggestion that matches what is typed, with the characters that matched (marked in `--accent`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ranked {
    pub item: Item,
    /// Char indexes of the label that matched what was typed.
    pub matched: Vec<usize>,
}

/// Extensions a JS/TS import usually leaves out (`IMPLIED_EXTENSION`).
fn implied_extension(name: &str) -> Option<usize> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"(?:\.d)?\.[cm]?[jt]sx?$").expect("a fixed pattern"));
    re.find(name).map(|m| m.start())
}

/// The folder part of a path: up to and including the last "/", else nothing (`directoryOf`).
fn directory_of(path: &str) -> &str {
    path.rfind('/').map_or("", |i| &path[..=i])
}

/// Resolves "./", "../" and "a/b" against a folder, without leaving the repository (`joinFolder`).
fn join_folder(base: &str, relative: &str) -> Option<String> {
    let mut parts: Vec<&str> = base.split('/').filter(|p| !p.is_empty()).collect();
    for part in relative.split('/').filter(|p| !p.is_empty()) {
        match part {
            "." => {}
            ".." => {
                parts.pop()?;
            }
            other => parts.push(other),
        }
    }
    Some(if parts.is_empty() { String::new() } else { format!("{}/", parts.join("/")) })
}

/// The `src/` folder `@/` points at: the nearest one above the file, else the repository's (`aliasRoot`).
fn alias_root(current: &str) -> &str {
    match current.rfind("/src/") {
        Some(i) => &current[..i + "/src/".len()],
        None => "src/",
    }
}

/// The repository folder an import specifier's folder part points at, or `None` for a package import (`importFolder`).
pub fn import_folder(current: &str, typed_folder: &str) -> Option<String> {
    if typed_folder.starts_with("./") || typed_folder.starts_with("../") {
        join_folder(directory_of(current), typed_folder)
    } else if let Some(rest) = typed_folder.strip_prefix("@/") {
        join_folder(alias_root(current), rest)
    } else {
        None
    }
}

/// Where an import path is being typed: the folder part to list and how many bytes of the last segment are typed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportAt {
    pub folder: String,
    pub segment: usize,
}

/// The import path being typed at the end of `before` (the line up to the caret), as `importPathSource` reads it: after `from "`,
/// `import "`, `import("` or `require("`. Paths complete from the first "/", so `"./` does and `"vu` doesn't.
pub fn import_at(current: &str, before: &str) -> Option<ImportAt> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re =
        RE.get_or_init(|| Regex::new(r#"(?:\bfrom\s*|\bimport\s*\(?\s*|\brequire\s*\(\s*)(["'])([^"'\n]*)$"#).expect("a fixed pattern"));
    let typed = re.captures(before)?.get(2)?.as_str();
    let typed_folder = directory_of(typed);
    if typed_folder.is_empty() {
        return None;
    }
    Some(ImportAt { folder: import_folder(current, typed_folder)?, segment: typed.len() - typed_folder.len() })
}

/// The suggestions for a folder's listing (`entries`: repository paths, folders ending in "/"), the file itself left out and the
/// JS/TS extensions of a file's label left off.
pub fn import_items(current: &str, folder: &str, entries: &[String]) -> Vec<Item> {
    let mut out = Vec::new();
    for entry in entries {
        let is_folder = entry.ends_with('/');
        let name = entry.strip_prefix(folder).unwrap_or(entry);
        if name.is_empty() || entry == current {
            continue;
        }
        let label = if is_folder { name.to_string() } else { name[..implied_extension(name).unwrap_or(name.len())].to_string() };
        out.push(Item {
            label,
            detail: Some(if is_folder { "folder".to_string() } else { name.to_string() }),
            kind: if is_folder { Kind::Folder } else { Kind::File },
            boost: i32::from(is_folder),
        });
    }
    out
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// The word being typed that ends at byte `at` of `line`: where it starts.
pub fn word_start(line: &str, at: usize) -> usize {
    let mut start = at;
    for (i, c) in line[..at].char_indices().rev() {
        if !is_word(c) {
            break;
        }
        start = i;
    }
    start
}

/// How far around the caret the words are collected from (`completeAnyWord`'s `range`).
const WORD_RANGE: usize = 50_000;

/// Words of the file that begin with, or contain the letters of, `typed` in order, found near line `line` (outwards from it, so a
/// 20,000-line file costs a window of text, not all of it). The word being typed itself is left out.
pub fn words(lines: &[String], line: usize, typed: &str) -> Vec<Item> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    let mut spent = 0usize;
    let mut take = |text: &str, out: &mut Vec<Item>, spent: &mut usize| {
        *spent += text.len() + 1;
        let mut from = None;
        for (i, c) in text.char_indices().chain([(text.len(), ' ')]) {
            match (is_word(c), from) {
                (true, None) => from = Some(i),
                (false, Some(f)) => {
                    let w = &text[f..i];
                    from = None;
                    if w.chars().count() >= 2
                        && w != typed
                        && w.chars().next().is_some_and(|c| !c.is_ascii_digit())
                        && seen.insert(w.to_string())
                    {
                        out.push(Item { label: w.to_string(), detail: None, kind: Kind::Word, boost: 0 });
                    }
                }
                _ => {}
            }
        }
    };
    // Outwards from the caret's line: that line (up to the caret is the caller's `typed`, so the line is taken whole), then the line
    // above and below it in turn.
    let mut step = 0usize;
    while spent < WORD_RANGE {
        let up = line.checked_sub(step);
        let down = line + step;
        let mut any = false;
        if let Some(i) = up
            && let Some(l) = lines.get(i)
        {
            take(l, &mut out, &mut spent);
            any = true;
        }
        if step > 0
            && let Some(l) = lines.get(down)
        {
            take(l, &mut out, &mut spent);
            any = true;
        }
        if !any {
            break;
        }
        step += 1;
    }
    out
}

/// The characters of `label` that spell `typed` in order, ignoring case, taking a prefix first: `None` when they don't.
fn fuzzy(label: &str, typed: &str) -> Option<(u8, Vec<usize>)> {
    if typed.is_empty() {
        return Some((0, Vec::new()));
    }
    let lower: Vec<char> = label.chars().flat_map(char::to_lowercase).collect();
    let want: Vec<char> = typed.chars().flat_map(char::to_lowercase).collect();
    if lower.len() != label.chars().count() {
        // A character whose lowercase is longer would shift the marks: match as plain text.
        return label.to_lowercase().starts_with(&typed.to_lowercase()).then(|| (0, (0..typed.chars().count()).collect()));
    }
    if lower.starts_with(&want) {
        return Some((0, (0..want.len()).collect()));
    }
    // One typed letter only matches a word that starts with it (CodeMirror's `FuzzyMatcher`).
    if want.len() == 1 {
        return None;
    }
    let mut at = 0;
    let mut matched = Vec::with_capacity(want.len());
    for (i, c) in lower.iter().enumerate() {
        if at < want.len() && *c == want[at] {
            matched.push(i);
            at += 1;
        }
    }
    (at == want.len()).then_some((1, matched))
}

/// The suggestions that match `typed`, best first: a prefix before a looser match, then the higher boost (folders), then the
/// alphabetical. A word the same as `typed` is not offered.
pub fn rank(items: Vec<Item>, typed: &str) -> Vec<Ranked> {
    let mut scored: Vec<(u8, Ranked)> = items
        .into_iter()
        .filter(|i| i.label != typed || i.kind != Kind::Word)
        .filter_map(|item| fuzzy(&item.label, typed).map(|(score, matched)| (score, Ranked { item, matched })))
        .collect();
    scored
        .sort_by(|(sa, a), (sb, b)| sa.cmp(sb).then_with(|| b.item.boost.cmp(&a.item.boost)).then_with(|| a.item.label.cmp(&b.item.label)));
    scored.into_iter().map(|(_, r)| r).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(items: &[Item]) -> Vec<&str> {
        items.iter().map(|i| i.label.as_str()).collect()
    }

    // completion.test.ts, "importFolder".
    #[test]
    fn import_folder_resolves_dot_and_dot_dot_against_the_files_folder() {
        assert_eq!(import_folder("src/components/Foo.vue", "./").as_deref(), Some("src/components/"));
        assert_eq!(import_folder("src/components/Foo.vue", "../lib/").as_deref(), Some("src/lib/"));
        assert_eq!(import_folder("app.ts", "./").as_deref(), Some(""));
    }

    #[test]
    fn import_folder_maps_at_to_the_src_folder_the_file_is_in() {
        assert_eq!(import_folder("client/src/components/Foo.vue", "@/lib/").as_deref(), Some("client/src/lib/"));
        assert_eq!(import_folder("src/main.ts", "@/").as_deref(), Some("src/"));
    }

    #[test]
    fn import_folder_does_not_leave_the_repo_or_complete_packages() {
        assert_eq!(import_folder("app.ts", "../"), None);
        assert_eq!(import_folder("src/app.ts", "vue/"), None);
    }

    // completion.test.ts, "importPathSource". Fleet's source returns the items and the `from`; here `import_at` and `import_items`.
    fn complete(doc: &str, listing: &[&str]) -> Option<(ImportAt, Vec<Item>)> {
        let at = import_at("src/components/Foo.ts", doc)?;
        let entries: Vec<String> = listing.iter().map(|s| s.to_string()).collect();
        let items = import_items("src/components/Foo.ts", &at.folder, &entries);
        Some((at, items))
    }

    #[test]
    fn import_path_lists_the_typed_folder_and_replaces_only_the_last_segment() {
        let doc = "import { x } from \"../lib/fo";
        let (at, items) = complete(doc, &["src/lib/format.ts", "src/lib/types.d.ts", "src/lib/nested/"]).expect("an import");
        assert_eq!(at.folder, "src/lib/");
        assert_eq!(at.segment, 2, "only `fo` is replaced");
        assert_eq!(labels(&items), ["format", "types", "nested/"]);
    }

    #[test]
    fn import_path_also_works_in_import_and_require() {
        let (_, a) = complete("const m = await import(\"./", &["src/components/Bar.ts"]).expect("import()");
        assert_eq!(a[0].label, "Bar");
        let (_, b) = complete("const m = require('./", &["src/components/Bar.js"]).expect("require()");
        assert_eq!(b[0].label, "Bar");
    }

    #[test]
    fn import_path_leaves_out_the_file_itself() {
        let (_, items) = complete("import a from \"./", &["src/components/Foo.ts", "src/components/Bar.ts"]).expect("an import");
        assert_eq!(labels(&items), ["Bar"]);
    }

    #[test]
    fn import_path_stays_out_of_the_way_outside_an_import_and_before_the_first_slash() {
        assert!(complete("const s = \"./", &[]).is_none());
        assert!(complete("import a from \".", &[]).is_none());
        assert!(complete("import a from \"vu", &[]).is_none());
    }

    #[test]
    fn a_folder_is_marked_and_sorts_above_the_files() {
        let (_, items) = complete("import a from \"./", &["src/components/Bar.ts", "src/components/ui/"]).expect("an import");
        let ranked = rank(items, "");
        assert_eq!(ranked[0].item.label, "ui/");
        assert_eq!(ranked[0].item.detail.as_deref(), Some("folder"));
        assert_eq!(ranked[1].item.detail.as_deref(), Some("Bar.ts"));
    }

    // Ours: the words of the file.
    #[test]
    fn words_come_from_the_file_nearest_the_caret_first_and_leave_out_the_word_typed() {
        let lines: Vec<String> =
            ["def total(rows):", "    page_size = 10", "    return page_s", "", "def other(): pass"].map(String::from).to_vec();
        let found = words(&lines, 2, "page_s");
        let ranked = rank(found, "page_s");
        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].item.label, "page_size");
        assert_eq!(ranked[0].matched, vec![0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn a_looser_match_comes_after_a_prefix_and_marks_the_letters_it_found() {
        let items =
            ["total_pages", "page_total", "pages"].map(|l| Item { label: l.into(), detail: None, kind: Kind::Word, boost: 0 }).to_vec();
        let ranked = rank(items, "pg");
        assert_eq!(ranked.iter().map(|r| r.item.label.as_str()).collect::<Vec<_>>(), ["page_total", "pages", "total_pages"]);
        assert_eq!(ranked[1].matched, vec![0, 2]);
        let none = rank(vec![Item { label: "zzz".into(), detail: None, kind: Kind::Word, boost: 0 }], "pg");
        assert!(none.is_empty());
    }

    #[test]
    fn collecting_words_in_a_huge_file_looks_only_at_a_window() {
        let lines: Vec<String> = (0..20_000).map(|i| format!("let value_{i} = {i};")).collect();
        let found = words(&lines, 10_000, "val");
        assert!(found.len() < 6_000, "a window, not all 20,000 lines: {}", found.len());
        assert!(found.iter().any(|w| w.label == "value_10001"));
    }

    #[test]
    fn the_word_before_the_caret_starts_where_its_letters_start() {
        assert_eq!(word_start("  return page_s", 15), 9);
        assert_eq!(word_start("a.b", 3), 2);
        assert_eq!(word_start("  ", 2), 2);
    }
}
