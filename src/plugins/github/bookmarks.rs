//! The repositories you follow (`composables/use-github-bookmarks.ts`): kept by the Fleet server, one shared list sorted by full
//! name, loaded once when first used (one request in flight). Add, remove and replace each wait for the server before the list
//! changes. Fleet's one-time move of bookmarks out of the browser's `localStorage` has no counterpart here: a native app never had any.
use crate::State;
use crate::fleet::http;
use crate::fleet::settings::HOME_MACHINE_KEY;
use serde_json::{Value, json};

const API: &str = "/api/integrations/github/bookmarks";

/// `BookmarkedRepo`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bookmark {
    pub full_name: String,
    pub owner: String,
    pub name: String,
}

impl Bookmark {
    /// A bookmark from `owner/name`; `None` when either half is missing.
    pub fn of(full_name: &str) -> Option<Bookmark> {
        let (owner, rest) = full_name.split_once('/')?;
        (!owner.is_empty() && !rest.is_empty()).then(|| Bookmark { full_name: full_name.into(), owner: owner.into(), name: rest.into() })
    }
}

#[derive(Debug, Default)]
pub struct Bookmarks {
    pub list: Vec<Bookmark>,
    pub loading: bool,
    /// Why the last load failed.
    pub error: Option<String>,
    pub has_loaded: bool,
    /// Why the last add, remove or replace failed (Fleet throws it to whoever asked; the page that asked shows it).
    pub action_error: Option<String>,
}

impl Bookmarks {
    pub fn has(&self, full_name: &str) -> bool {
        self.list.iter().any(|b| b.full_name == full_name)
    }
}

/// `sortByName`: by full name, letters of either case together.
fn sorted(mut list: Vec<Bookmark>) -> Vec<Bookmark> {
    list.sort_by(|a, b| a.full_name.to_lowercase().cmp(&b.full_name.to_lowercase()).then_with(|| a.full_name.cmp(&b.full_name)));
    list
}

/// `normalizeBookmarksResponse`: a list, or `{bookmarks: [...]}`, else nothing.
pub fn read_bookmarks(v: &Value) -> Vec<Bookmark> {
    let list = v.as_array().or_else(|| v.get("bookmarks").and_then(Value::as_array));
    list.map(|a| {
        a.iter()
            .filter_map(|b| {
                let text = |k: &str| b.get(k).and_then(Value::as_str).map(str::to_string);
                let full_name = text("fullName")?;
                Some(Bookmark { owner: text("owner").unwrap_or_default(), name: text("name").unwrap_or_default(), full_name })
            })
            .collect()
    })
    .unwrap_or_default()
}

/// `readErrorMessage`: the server's `error`, else its `message`, else `fallback`.
fn failure(answer: Result<(u16, String), String>, fallback: &str) -> Option<String> {
    match answer {
        Ok((status, _)) if (200..300).contains(&status) => None,
        Ok((_, body)) => Some(super::auth::words(&body, fallback)),
        Err(_) => Some(fallback.to_string()),
    }
}

impl State {
    /// `onMounted`: loads the list the first time something shows it.
    pub fn github_bookmarks_use(&mut self) {
        if !super::state(&mut self.places).bookmarks.has_loaded {
            self.github_bookmarks_load();
        }
    }

    /// `loadBookmarks` (also `refresh`): one request in flight, whoever asks.
    pub fn github_bookmarks_load(&mut self) {
        let b = &mut super::state(&mut self.places).bookmarks;
        if b.loading {
            return;
        }
        b.loading = true;
        const FALLBACK: &str = "Failed to fetch bookmarks from server.";
        let sent = self.feed_call(
            HOME_MACHINE_KEY,
            |base| http::text_with("GET", base, API, None),
            |s, _, answer| {
                let b = &mut super::state(&mut s.places).bookmarks;
                b.loading = false;
                match answer {
                    Ok((status, body)) if (200..300).contains(&status) => {
                        b.list = sorted(read_bookmarks(&serde_json::from_str(&body).unwrap_or_default()));
                        b.error = None;
                        b.has_loaded = true;
                    }
                    other => b.error = failure(other, FALLBACK),
                }
            },
        );
        if !sent {
            let b = &mut super::state(&mut self.places).bookmarks;
            b.loading = false;
            b.error = Some(FALLBACK.into());
        }
    }

    /// `addBookmark`: nothing to do when it is there already.
    pub fn github_bookmark_add(&mut self, full_name: &str) {
        let Some(repo) = Bookmark::of(full_name) else { return };
        if super::state(&mut self.places).bookmarks.has(full_name) {
            return;
        }
        let body = json!({ "repo": full_name });
        self.bookmark_call("POST", API.into(), Some(body), "Failed to add bookmark.", move |list| {
            list.push(repo);
        });
    }

    /// `removeBookmark`: `owner/name` goes into the address as it is; a name without both halves is refused before asking.
    pub fn github_bookmark_remove(&mut self, full_name: &str) {
        let Some(mark) = Bookmark::of(full_name) else {
            super::state(&mut self.places).bookmarks.action_error = Some("Invalid bookmark name.".into());
            return;
        };
        let gone = full_name.to_string();
        self.bookmark_call("DELETE", format!("{API}/{}/{}", mark.owner, mark.name), None, "Failed to remove bookmark.", move |list| {
            list.retain(|b| b.full_name != gone)
        });
    }

    /// `replaceBookmarks`: the whole list, sorted.
    pub fn github_bookmarks_replace(&mut self, repos: Vec<Bookmark>) {
        let next = sorted(repos);
        let body = json!({ "bookmarks": next.iter().map(|b| json!({"fullName": b.full_name, "owner": b.owner, "name": b.name})).collect::<Vec<_>>() });
        self.bookmark_call("PUT", API.into(), Some(body), "Failed to sync bookmarks.", move |list| *list = next);
    }

    /// One change: asked of the server, applied to the list when it said yes, the reason kept when it didn't.
    fn bookmark_call(
        &mut self,
        method: &'static str,
        path: String,
        body: Option<Value>,
        fallback: &'static str,
        apply: impl FnOnce(&mut Vec<Bookmark>) + Send + 'static,
    ) {
        let sent = self.feed_call(
            HOME_MACHINE_KEY,
            move |base| http::text_with(method, base, &path, body.as_ref()),
            move |s, _, answer| {
                let b = &mut super::state(&mut s.places).bookmarks;
                b.action_error = failure(answer, fallback);
                if b.action_error.is_none() {
                    apply(&mut b.list);
                    b.list = sorted(std::mem::take(&mut b.list));
                    b.error = None;
                }
            },
        );
        if !sent {
            super::state(&mut self.places).bookmarks.action_error = Some(fallback.into());
        }
    }
}
