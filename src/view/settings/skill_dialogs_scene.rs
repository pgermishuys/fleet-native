//! Scenes of the skill Editor and History dialogs (`settings-skill-*`): over Built-in skills, from an invented skill with two
//! versions of the user's, the second made in a session.
use super::plugins_scene::{BuiltInLook, built_in_scene};
use crate::State;
use crate::fleet::settings::skill_editor::{Detail, Version};
use crate::fleet::settings::skill_history::Compare;
use crate::view::skill_diff::{AFTER, BEFORE};
use rune::TextEdit;

#[derive(Clone, Copy)]
pub enum Look {
    Editor,
    EditorChanges,
    History,
    HistoryCompare,
}

fn detail() -> Detail {
    Detail {
        name: "tidy-docs".into(),
        fleet_content: BEFORE.into(),
        your_content: Some(AFTER.into()),
        version: Some(2),
        versions: vec![
            Version {
                number: 2,
                created_at: "2026-11-12T09:00:00Z".into(),
                note: Some("Also check the examples that use it".into()),
                session_title: Some("Fix pagination".into()),
                active: true,
            },
            Version { number: 1, created_at: "2026-11-05T09:00:00Z".into(), ..Default::default() },
        ],
    }
}

pub fn scene(s: &mut State, dark: bool, look: Look) {
    built_in_scene(s, dark, BuiltInLook::Changed);
    let d = detail();
    match look {
        Look::Editor | Look::EditorChanges => {
            let e = &mut s.settings.editor;
            e.open = true;
            e.name = d.name.clone();
            e.draft = TextEdit::new(format!("{AFTER}Reply in plain words.\n"));
            e.note = TextEdit::new("");
            e.changes = matches!(look, Look::EditorChanges);
            e.detail = Some(d);
        }
        Look::History | Look::HistoryCompare => {
            let h = &mut s.settings.history;
            h.open = true;
            h.name = d.name.clone();
            if matches!(look, Look::HistoryCompare) {
                h.comparing = Some(Compare {
                    number: 1,
                    content: AFTER.replace("check the docs that name it, and the examples that use it.", "check the docs that name it."),
                });
            }
            h.detail = Some(d);
        }
    }
}
