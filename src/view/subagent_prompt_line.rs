//! The note that stands where the composer would be in a sub-agent's session its harness can't prompt. Ports
//! `components/session/SubagentPromptLine.vue` (docs/parity/03-conversation-parts.md row 4.31): the work strip, then one
//! centred line pointing at the parent. Its work belongs to the parent's turn, so a message here would wait for good.
use super::conversation::COLUMN;
use crate::State;
use crate::theme::{Theme, sans};
use rune::{Cursor, El, col, row, text};

/// What the line says, by whether the parent's title is known.
pub fn words(parent_title: Option<&str>) -> (&'static str, Option<&str>, &'static str) {
    match parent_title {
        Some(title) => ("This is a subagent of ", Some(title), ". Ask there."),
        None => ("This is a subagent's session. Ask the session that started it.", None, ""),
    }
}

impl State {
    /// `isReadOnlySubagent`: a delegated session that isn't archived and whose harness says it takes no prompt. Returns the
    /// parent the line points at (the one Open came from, else the session's own).
    pub fn read_only_subagent(&self) -> Option<String> {
        let row = &self.session().row;
        if row.archived || row.can_prompt != Some(false) {
            return None;
        }
        let (parent, kind) = crate::fleet::groups::lineage_of(&self.session().listing)?;
        (kind == crate::fleet::groups::LineageKind::Subagent).then_some(self.work.opened_from.clone().unwrap_or(parent))
    }
}

/// `SubagentPromptLine`: padding `4px 24px 18px`, the strip, then the note (`max-width: 760px`, padding `10px 4px`, 13 px).
pub fn line(s: &State, t: &Theme, parent: &str) -> El {
    let reason = s.session().row.prompt_disabled_reason.clone();
    let title = s.parent_title(parent).filter(|p| !p.is_empty());
    let (before, link, after) = words(title.as_deref());
    let font = sans(13.0, 400.0, 1.5);
    let mut body = row().key("subagent-prompt-line").label("Subagent note").w_full().max_w(COLUMN).px(4.0).py(10.0).justify_center().wrap();
    body = body.child(text(before).font(font).color(t.muted));
    if let Some(title) = link {
        let to = parent.to_string();
        body = body
            .child(
                text(title)
                    .key("subagent-prompt-parent")
                    .font(sans(13.0, 500.0, 1.5))
                    .color(t.text)
                    .cursor(Cursor::Pointer)
                    .on_press_tx(move |s: &mut State, tx: &mut rune::Tx| s.open_parent(&to, tx)),
            )
            .child(text(after).font(font).color(t.muted));
    }
    if let Some(reason) = reason {
        body = body.help(reason);
    }
    let mut outer = col().key("subagent-prompt-section").label("Subagent prompt line").w_full().pt(4.0).px(24.0).pb(18.0).items_center();
    if let Some(strip) = super::work::strip(s, t) {
        outer = outer.child(strip);
    }
    outer.child(body)
}

impl State {
    /// A scene: a sub-agent's session whose harness can't take a prompt (Claude Code), so the line stands for the composer.
    pub fn show_subagent_prompt_line(&mut self, dark: bool) {
        self.show_header(crate::mock::header::Case::Subagent, dark, true);
        if let Some(open) = self.session_mut() {
            open.row.can_prompt = Some(false);
            open.row.prompt_disabled_reason =
                Some("Claude Code can't prompt a subagent on its own. Ask the session that started it.".into());
        }
    }
}
