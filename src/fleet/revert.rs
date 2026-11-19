//! Revert in the Changes review (`ChangesReview.vue` "Revert", rows 10.1 to 10.8 of docs/parity/12-changes-and-review.md): puts a file
//! or one hunk back as it was before the agent changed it. It asks first (`Confirm::revert_file`, `Confirm::revert_hunk`), then reads
//! the file as it is on disk, refuses to act from a picture that has gone stale, and writes or restores with the hash it read, so
//! an edit made in between is never lost. `run` makes the requests and is blocking: `State::start_revert` runs it on a thread.
//! This only ever acts on a session's own folder, through the server's guarded write and restore.
use super::diffs::{FileDiff, FileStatus};
use super::http;
use crate::State;
use crate::conv::diff::parse_diff_lines;
use crate::conv::review::text_with_range_reverted;
use crate::fleet::actions::Call;
use crate::ui::confirm::Confirm;
use rune::Tx;
use serde_json::Value;

/// What a revert came to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Written or restored.
    Reverted,
    /// The server refused because the file isn't what was read (a 409): nothing was changed, and the words say so.
    Kept(String),
    /// The file moved on since the diff was drawn: its diff and the list are asked again, and the words say so.
    Stale(String),
    /// A request failed: the words are "Could not revert …".
    Failed(String),
}

pub fn stale_words(path: &str) -> String {
    format!("{path} changed while you were reviewing it. Its diff is refreshed; revert again if you still want to.")
}

pub(crate) fn reason_of(status: u16) -> String {
    let phrase = ureq::http::StatusCode::from_u16(status).ok().and_then(|s| s.canonical_reason()).unwrap_or("");
    format!("{status} {phrase}").trim_end().to_string()
}

/// The server's `error` when it gave one, else `<what failed>: <status>`.
pub(crate) fn refusal(status: u16, body: &str, failed: &str) -> String {
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v.get("error").and_then(Value::as_str).filter(|e| !e.is_empty()).map(str::to_string))
        .unwrap_or_else(|| format!("{failed}: {}", reason_of(status)))
}

/// What a file read says: its text (empty when it has none) and the hash to write against.
fn read(base: &str, session: &str, path: &str) -> Result<(String, String), String> {
    let (status, body) = http::file_content(base, session, path)?;
    if !(200..300).contains(&status) {
        return Err(format!("Failed to read file: {}", reason_of(status)));
    }
    let v: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
    let text = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    Ok((text("content"), text("hash")))
}

/// A save or restore: `true` when it took, `false` on a 409 (the file isn't what was read).
fn accepted(answer: (u16, String), failed: &str) -> Result<bool, String> {
    let (status, body) = answer;
    match status {
        200..=299 => Ok(true),
        409 if !body.is_empty() => Ok(false),
        _ => Err(refusal(status, &body, failed)),
    }
}

/// The revert, blocking. `range` is the hunk's first and last line in the file's diff, or `None` for the whole file.
pub fn run(base: &str, session: &str, item: &FileDiff, range: Option<(usize, usize)>) -> Outcome {
    let path = item.file.as_str();
    let go = || -> Result<Outcome, String> {
        if item.status == FileStatus::Deleted && range.is_none() {
            // Gone from disk: put it back as the session found it. Refused if something new sits at that path.
            let put = accepted(http::restore_file(base, session, path, Some(&item.before), None)?, "Failed to restore file")?;
            return Ok(if put { Outcome::Reverted } else { Outcome::Kept(format!("{path} exists again, so nothing was reverted.")) });
        }
        let (content, hash) = read(base, session, path)?;
        // The file moved on since the diff was drawn: reverting from a stale picture would lose the newer edit.
        if content != item.after {
            return Ok(Outcome::Stale(stale_words(path)));
        }
        if item.status == FileStatus::Added && range.is_none() {
            // The session created it: reverting removes it, now that it is still what the diff showed.
            let gone = accepted(http::restore_file(base, session, path, None, Some(&hash))?, "Failed to restore file")?;
            return Ok(if gone { Outcome::Reverted } else { Outcome::Kept(format!("{path} changed on disk, so nothing was removed.")) });
        }
        let text = match range {
            Some((from, to)) => {
                let lines = parse_diff_lines(&item.before, &item.after);
                text_with_range_reverted(&lines, from, to, item.after.ends_with('\n'), Some((&item.before, &item.after)))
            }
            None => item.before.clone(),
        };
        let saved = accepted(http::write_file(base, session, path, &text, &hash)?, "Failed to save file")?;
        Ok(if saved { Outcome::Reverted } else { Outcome::Kept(format!("{path} changed on disk, so nothing was reverted.")) })
    };
    go().unwrap_or_else(|why| Outcome::Failed(format!("Could not revert {path}: {why}")))
}

impl Confirm {
    /// `askRevertFile`: the body depends on what happened to the file.
    pub fn revert_file(path: &str, status: FileStatus, call: Call) -> Confirm {
        let body = match status {
            FileStatus::Added => "Deletes the file: the session created it. Its contents are lost.",
            FileStatus::Deleted => "Puts the deleted file back as it was.",
            FileStatus::Modified => "Puts the file back as it was before the agent changed it. The agent's edits to it are lost.",
        };
        Confirm::revert("Revert this file?", path, body, "Revert file", call)
    }

    /// `askRevertHunk`.
    pub fn revert_hunk(path: &str, call: Call) -> Confirm {
        Confirm::revert(
            "Revert this change?",
            path,
            "Puts these lines back as they were. The agent's other edits to the file stay.",
            "Revert change",
            call,
        )
    }

    fn revert(title: &str, path: &str, body: &str, confirm_label: &str, call: Call) -> Confirm {
        Confirm {
            title: title.into(),
            subject: path.into(),
            body: body.into(),
            confirm_label: confirm_label.into(),
            busy_label: "Reverting…".into(),
            error_message: "Couldn't revert.".into(),
            destructive: true,
            busy: false,
            error: None,
            call,
        }
    }
}

impl State {
    /// Revert file (`range` is `None`) or Revert hunk: asks first. Nothing happens while another revert runs.
    pub fn ask_revert(&mut self, path: &str, range: Option<(usize, usize)>, tx: &mut Tx) {
        if self.review.reverting {
            return;
        }
        let call = Call::Revert { path: path.to_string(), range };
        let confirm = match range {
            Some(_) => Confirm::revert_hunk(path, call),
            None => {
                let status = self.diffs.by_file(path).map_or(FileStatus::Modified, |f| f.status);
                Confirm::revert_file(path, status, call)
            }
        };
        self.ask(confirm, tx);
    }

    /// The question was answered Yes: reads the file and reverts, on a thread. Does nothing unless the file's contents are loaded.
    pub(crate) fn start_revert(&mut self, path: &str, range: Option<(usize, usize)>) {
        if self.review.reverting {
            return;
        }
        let session = self.diffs.session().to_string();
        let Some(crate::fleet::diff_contents::Entry::Ready(item)) = self.diff_contents.entry(path).cloned() else { return };
        let Some(link) = self.link_for(&session) else { return };
        self.review.reverting = true;
        self.review.notice = None;
        let (id, file) = (session.clone(), path.to_string());
        link.off_thread(move |base| run(base, &id, &item, range), move |s, _, outcome| s.revert_done(&session, &file, outcome));
    }

    /// The answer: the notice if there is one, then the list is asked again (never joined to an answer that may predate the
    /// revert) and, once it has come, the file's diff again, only if the file is still in it (10.7). A failure refreshes nothing.
    fn revert_done(&mut self, session: &str, path: &str, outcome: Outcome) {
        self.review.reverting = false;
        if self.diffs.session() != session {
            return;
        }
        match outcome {
            Outcome::Failed(words) => self.review.notice = Some(words),
            Outcome::Stale(words) => {
                self.review.notice = Some(words);
                if let Some(file) = self.diffs.by_file(path).cloned() {
                    self.reload_diff_file(session, &file);
                }
                self.refetch_diffs();
            }
            Outcome::Kept(words) => {
                self.review.notice = Some(words);
                self.review.reload_after_list = Some(path.to_string());
                self.refetch_diffs();
            }
            Outcome::Reverted => {
                self.review.reload_after_list = Some(path.to_string());
                self.refetch_diffs();
            }
        }
    }

    /// A list answered: the file a revert was waiting on is asked for again if it is still there.
    pub(crate) fn after_revert_list(&mut self) {
        if self.diffs.is_loading() {
            return;
        }
        let Some(path) = self.review.reload_after_list.take() else { return };
        let session = self.diffs.session().to_string();
        if let Some(file) = self.diffs.by_file(&path).cloned() {
            self.reload_diff_file(&session, &file);
        }
    }

    pub fn dismiss_review_notice(&mut self) {
        self.review.notice = None;
    }
}
