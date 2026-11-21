//! Items by number and one item (`lib/github-items.ts`: `fetchGitHubItemsByNumber`, `fetchGitHubItem`). Nothing is kept: the pages
//! and the `#` picker ask when they need and hold the answer themselves. The request goes on a thread; `done` runs on the UI thread.
use super::items::{ItemDetail, ItemSummary, error_words, read_items};
use crate::fleet::http;
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::{State, Tx};
use serde_json::Value;

use crate::conv::export_history::encode_component as encode;

/// What asking came to before any answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Asked {
    /// Nothing was asked: no numbers (Fleet answers an empty list without a request).
    Nothing,
    Sent,
    /// The home machine can't be reached from here; `done` will not run.
    Unreachable,
}

const FAILED: &str = "GitHub didn't answer.";

/// `…/items?numbers=a,b`; none when there is no number.
pub fn by_number_path(owner: &str, repo: &str, numbers: &[u64]) -> Option<String> {
    if numbers.is_empty() {
        return None;
    }
    let list: Vec<String> = numbers.iter().map(u64::to_string).collect();
    Some(format!("/api/integrations/github/repos/{}/{}/items?numbers={}", encode(owner), encode(repo), list.join(",")))
}

pub fn item_path(owner: &str, repo: &str, number: u64) -> String {
    format!("/api/integrations/github/repos/{}/{}/items/{number}", encode(owner), encode(repo))
}

fn read<T>(answer: Result<(u16, String), String>, of: impl FnOnce(&Value) -> T) -> Result<T, String> {
    match answer {
        Ok((status, body)) if (200..300).contains(&status) => Ok(of(&serde_json::from_str(&body).unwrap_or_default())),
        Ok((status, body)) => Err(error_words(status, &body)),
        Err(_) => Err(FAILED.into()),
    }
}

impl State {
    /// `fetchGitHubItemsByNumber`: unknown numbers are left out by the server.
    pub fn github_items_by_number(
        &self,
        owner: &str,
        repo: &str,
        numbers: &[u64],
        done: impl FnOnce(&mut State, &mut Tx, Result<Vec<ItemSummary>, String>) + Send + 'static,
    ) -> Asked {
        let Some(path) = by_number_path(owner, repo, numbers) else { return Asked::Nothing };
        let sent = self.feed_call(
            HOME_MACHINE_KEY,
            move |base| http::text_with("GET", base, &path, None),
            move |s, tx, a| done(s, tx, read(a, read_items)),
        );
        if sent { Asked::Sent } else { Asked::Unreachable }
    }

    /// `fetchGitHubItem`: one item with its body, checks and conversation.
    pub fn github_item(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        done: impl FnOnce(&mut State, &mut Tx, Result<ItemDetail, String>) + Send + 'static,
    ) -> Asked {
        let path = item_path(owner, repo, number);
        let sent = self.feed_call(
            HOME_MACHINE_KEY,
            move |base| http::text_with("GET", base, &path, None),
            move |s, tx, a| done(s, tx, read(a, ItemDetail::read)),
        );
        if sent { Asked::Sent } else { Asked::Unreachable }
    }
}
