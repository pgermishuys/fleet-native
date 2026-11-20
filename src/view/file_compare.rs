//! The notice under a file tab's bar while the file is in conflict, and while it is in Compare (`FileCanvas.vue` 478-525, rows 13.6
//! and 13.7 of docs/parity/11-files.md). The conflict says what happened and has Compare, Keep mine and Use the agent's (Keep mine alone
//! when what is on disk is no longer text); Compare says which colour is whose and has Done. The agent's against yours itself is
//! `file_diff::compare_view`.
use crate::State;
use crate::fleet::file_buffers::Conflict;
use crate::theme::{Theme, sans};
use rune::{Cursor, El, Span, el, rich, row};

/// `.file-canvas__notice-actions button`: 24 high, padding `0 9`, 1 px `--border`, radius 7, `--card-bg`, 12 px 500; the primary one
/// (Done) has the accent for its border and fill. Hover turns the border to the accent: a 1 px ring round the button does that here
/// (Rune has no hover border, docs/RUNE.md).
fn action(t: &Theme, key: &str, words: &str, primary: bool) -> El {
    let (ring, fill, ink) = if primary { (t.accent, t.accent, t.primary_fg) } else { (t.border, t.card_bg, t.text) };
    let inner = row()
        .h(22.0)
        .px(8.0)
        .items_center()
        .radius(6.0)
        .fill(fill)
        .child(rune::text(words.to_string()).font(sans(12.0, 500.0, 1.4)).color(ink).single_line());
    row()
        .key(key)
        .role("button")
        .label(words)
        .padding(1.0)
        .radius(7.0)
        .fill(ring)
        .hover(t.accent)
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 1.0, t.accent)
        .child(inner)
}

/// A sentence whose first part is bold (`<b>`, 600).
fn sentence(s: &State, t: &Theme, bold: &str, rest: &str) -> El {
    let words = format!("{bold} {rest}");
    rich(words, vec![Span::new(0, bold.len()).weight(600.0)])
        .font(sans(12.5, 400.0, 1.45))
        .color(t.text)
        .wrap()
        .max_w((s.right_shown_w() - 24.0).max(0.0))
}

fn notice(t: &Theme, key: &str, fill: rune::Color, words: El, actions: Vec<El>) -> El {
    row()
        .key(key)
        .role("alert")
        .w_full()
        .px(12.0)
        .py(8.0)
        .flex_wrap()
        .gaps(6.0, 12.0)
        .items_center()
        .fill(fill)
        .border_sides([0.0, 0.0, 1.0, 0.0], t.border)
        .child(words)
        // `margin-left: auto`: the buttons sit at the right of their line, on the first line or the one they wrap to.
        .child(el().grow(1.0))
        .child(row().gap(6.0).items_center().children(actions))
}

/// The conflict's notice, or Compare's, for the file `path` (named `name` in the words).
pub fn notice_for(s: &State, t: &Theme, session: &str, path: &str, conflict: &Conflict, comparing: bool) -> El {
    let name = path.rsplit('/').next().unwrap_or(path);
    let go = |f: fn(&mut State, &str, &str)| {
        let (session, path) = (session.to_string(), path.to_string());
        move |s: &mut State| f(s, &session, &path)
    };
    if comparing {
        let done = {
            let (session, path) = (session.to_string(), path.to_string());
            move |s: &mut State, tx: &mut rune::Tx| s.done_comparing(&session, &path, tx)
        };
        let button = action(t, "compare-done", "Done", true).on_press_tx(done);
        return notice(
            t,
            "file-comparing",
            t.accent_dim,
            sentence(s, t, "Red is the agent's version, green is yours.", "Take the agent's lines hunk by hunk, then press Done and save."),
            vec![button],
        );
    }
    if conflict.disk_text.is_some() {
        return notice(
            t,
            "file-conflict",
            t.idle.alpha(0.14),
            sentence(s, t, &format!("The agent changed {name} while you were editing."), "Nothing was overwritten."),
            vec![
                action(t, "conflict-compare", "Compare", false).on_press(go(State::compare_file)),
                action(t, "conflict-mine", "Keep mine", false).on_press(go(State::keep_mine)),
                action(t, "conflict-theirs", "Use the agent's", false).on_press(go(State::use_agents)),
            ],
        );
    }
    notice(
        t,
        "file-conflict",
        t.idle.alpha(0.14),
        sentence(s, t, &format!("{name} changed on disk and can't be shown here any more."), "Nothing was overwritten."),
        vec![action(t, "conflict-mine", "Keep mine", false).on_press(go(State::keep_mine))],
    )
}
