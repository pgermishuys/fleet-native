//! The editor spike's scenes (component 11, piece P): `src/paginate.py` typed into both ways, an input method composing, a wrapped
//! line, and an invented file of twenty thousand lines (about 400 KB, under the 512 KB Fleet opens) edited far down.
use super::file_tab::{Case, answer};
use crate::State;
use crate::fleet::canvases::{FileView, OpenFile};
use crate::fleet::code_edit::{Move, Pos};
use crate::view::code_view::CODE;

/// The long file: generated, so the repository doesn't carry half a megabyte.
pub const BIG_PATH: &str = "src/ledger_rows.py";
/// How many lines it has.
pub const BIG_LINES: usize = 20_000;

/// The long file's text: ten-line functions over and over, each numbered, so every line is different and coloured.
pub fn big_source(lines: usize) -> String {
    let mut out = String::with_capacity(lines * 25);
    let mut n = 0;
    while n * 10 < lines {
        n += 1;
        out.push_str(&format!(
            "def rows_{n:04}(ledger, page=1):\n    \"\"\"Rows of page {n}.\"\"\"\n    start = (page - 1) * 25\n    items = ledger[start:start + 25]\n    if not items:\n        return []\n    return [i for i in items if i]\n\n\n# {n}\n"
        ));
    }
    let cut = out.match_indices('\n').nth(lines - 1).map_or(out.len(), |(i, _)| i + 1);
    out.truncate(cut);
    out
}

/// Which spike scene.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Spike {
    /// `src/paginate.py` in the code view, a comment typed on a new line and its last two words selected by Shift+Ctrl+Left.
    Typed,
    /// The same, with an input method composing at the caret.
    Ime,
    /// The long file, a word typed at line 12,345, the view there.
    Big,
    /// The long-lined file wrapped, the caret on the second row of a wrapped line.
    Wrap,
    /// `Typed` in Rune's `text_input` (the other way).
    Input,
    /// The long-lined file in the `text_input`: it wraps, and the numbers beside it don't.
    InputWrap,
    /// `Big` in the `text_input`.
    InputBig,
}

impl State {
    pub fn show_editor_spike(&mut self, spike: Spike, dark: bool) {
        let case = if matches!(spike, Spike::Wrap | Spike::InputWrap) { Case::Wrap } else { Case::Code };
        self.show_file(case, dark);
        self.code_edit.on = true;
        if matches!(spike, Spike::Big | Spike::InputBig) {
            self.open_big_file();
        }
        if matches!(spike, Spike::Input | Spike::InputWrap | Spike::InputBig) {
            let Some(file) = self.code_edit_target() else { return };
            let lines = self.file_buffers.buffer(&file.0, &file.1, &file.2).map(|b| b.lines().join("\n")).unwrap_or_default();
            let mut edit = rune::TextEdit::new(lines);
            if spike == Spike::Input {
                let at = edit.line_range(6).end;
                edit.set_caret(at);
                edit.insert("\n    # the first page starts at row 0");
                let end = edit.range().1;
                edit.set_selection(end, end - "row 0".len());
            }
            if spike == Spike::InputBig {
                let at = edit.line_range(12_344).end;
                edit.set_caret(at);
                edit.insert("  # checked by hand");
            }
            self.code_edit.input = Some((file, edit));
            self.code_edit.take_focus = Some(crate::view::code_input_spike::FIELD);
            return;
        }
        self.with_editor(|e, l| {
            match spike {
                Spike::Typed | Spike::Ime => {
                    e.set_caret(Pos::new(6, l[6].len()), false);
                    e.newline(l, 0.0);
                    e.insert(l, "# the first page starts at row 0", 0.1);
                    if spike == Spike::Typed {
                        e.motion(l, Move::WordLeft, true);
                        e.motion(l, Move::WordLeft, true);
                    } else {
                        e.insert(l, " ", 0.2);
                        e.preedit = Some(("ページ".to_string(), None));
                    }
                }
                Spike::Big => {
                    e.set_caret(Pos::new(12_344, l[12_344].len()), false);
                    e.insert(l, "  # checked by hand", 0.0);
                }
                Spike::Wrap => {
                    // Line 17's docstring wraps; the caret goes into its second row.
                    let at = l[16].find("whatever").map_or(l[16].len(), |i| i + 2);
                    e.set_caret(Pos::new(16, at), false);
                }
                Spike::Input | Spike::InputWrap | Spike::InputBig => {}
            }
            ((), true)
        });
        self.code_edit.take_focus = Some(CODE);
    }

    /// Opens the long file in a tab of its own, read and unwrapped.
    fn open_big_file(&mut self) {
        let id = self.canvas_session();
        let machine = self.machine_key_of(&id).to_string();
        self.sidebar.save_file_wrap(BIG_PATH, false);
        self.edit_canvases(|c, id| c.open_file(id, BIG_PATH, OpenFile { keep: true, view: Some(FileView::Edit) }, &|_| false));
        if let Some(request) = self.file_buffers.open(&machine, &id, BIG_PATH) {
            self.file_buffers.finish(&request, Ok(answer(&big_source(BIG_LINES))));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_long_file_has_its_lines_and_fits_what_fleet_opens() {
        let text = big_source(BIG_LINES);
        assert_eq!(text.matches('\n').count(), BIG_LINES);
        assert!(text.len() < 512 * 1024, "{} bytes", text.len());
        assert!(text.len() > 350 * 1024, "{} bytes", text.len());
    }
}
