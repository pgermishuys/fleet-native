//! `composer-references.test.ts` and the `$` half of `skill-mentions.test.ts`, under Fleet's names.
use super::draftrefs::*;
use super::refs::SkillDelivery::{Attached, Note};

fn plain(text: &str) -> Segment {
    Segment { text: text.into(), reference: None, picked: false }
}
fn reference(text: &str, kind: RefKind) -> Segment {
    Segment { text: text.into(), reference: Some(kind), picked: false }
}
fn picked(text: &str, kind: RefKind) -> Segment {
    Segment { text: text.into(), reference: Some(kind), picked: true }
}
fn split(text: &str, caret: Option<usize>) -> Vec<Segment> {
    split_draft_references(text, caret, &Known::default())
}

mod split_draft_references_tests {
    use super::*;
    use RefKind::*;

    #[test]
    fn marks_files_and_folders_picked_with_at() {
        assert_eq!(
            split("look at @src/app.ts and @src/components/ please", None),
            vec![
                plain("look at "),
                reference("@src/app.ts", File),
                plain(" and "),
                reference("@src/components/", Folder),
                plain(" please")
            ]
        );
    }

    #[test]
    fn leaves_emails_a_lone_at_and_double_at_alone() {
        assert_eq!(split("mail me@example.com @ now @@x", None), vec![plain("mail me@example.com @ now @@x")]);
    }

    #[test]
    fn keeps_the_reference_the_caret_is_still_typing_plain() {
        let text = "fix @src/ap";
        assert_eq!(split(text, Some(text.len())), vec![plain(text)]);
        assert_eq!(split(&format!("{text} "), Some(text.len() + 1)), vec![plain("fix "), reference("@src/ap", File), plain(" ")]);
    }

    #[test]
    fn treats_a_reference_at_the_very_start_and_one_without_a_caret_as_done() {
        assert_eq!(split("@README.md", None), vec![reference("@README.md", File)]);
    }

    #[test]
    fn drops_sentence_punctuation_after_a_reference() {
        assert_eq!(
            split("see @docs/guide.md, then (@src/)", None),
            vec![plain("see "), reference("@docs/guide.md", File), plain(", then (@src/)")]
        );
    }

    #[test]
    fn finds_references_on_later_lines() {
        assert_eq!(split("first\n@src/app.ts", None), vec![plain("first\n"), reference("@src/app.ts", File)]);
    }
}

#[test]
fn marks_a_token_picked_as_a_session_from_the_at_list_as_a_session() {
    let known = Known { sessions: &["@t3code-notes"], ..Known::default() };
    assert_eq!(
        split_draft_references("from @t3code-notes and @src/a.ts ", None, &known),
        vec![plain("from "), picked("@t3code-notes", RefKind::Session), plain(" and "), reference("@src/a.ts", RefKind::File), plain(" ")]
    );
}

mod picked_from_the_at_list {
    use super::*;
    const TEXT: &str = "look at @src/app.ts and @t3code-notes now";
    const SESSIONS: &[&str] = &["@t3code-notes"];
    const FILES: &[&str] = &["@src/app.ts"];

    #[test]
    fn stay_references_with_the_caret_at_their_end_where_a_typed_one_is_still_being_typed() {
        let caret = Some("look at @src/app.ts".len());
        let known = Known { sessions: SESSIONS, files: FILES, ..Known::default() };
        assert_eq!(split_draft_references(TEXT, caret, &known)[1], picked("@src/app.ts", RefKind::File));
        let sessions_only = Known { sessions: SESSIONS, ..Known::default() };
        assert_eq!(split_draft_references(TEXT, caret, &sessions_only)[0], plain("look at @src/app.ts and "));
    }

    #[test]
    fn stay_plain_once_their_folder_doesnt_have_them_until_picked_again() {
        let gone = Known { sessions: SESSIONS, plain: FILES, ..Known::default() };
        assert_eq!(split_draft_references(TEXT, None, &gone)[0], plain("look at @src/app.ts and "));
        let again = Known { sessions: SESSIONS, files: FILES, plain: FILES, ..Known::default() };
        assert_eq!(split_draft_references(TEXT, None, &again)[1], picked("@src/app.ts", RefKind::File));
    }

    #[test]
    fn are_found_where_they_are_in_the_text() {
        assert_eq!(picked_reference_ranges(TEXT, SESSIONS, FILES), vec![8..19, 24..37]);
        assert!(picked_reference_ranges("look at @src/app.ts", &[], &[]).is_empty());
    }

    #[test]
    fn go_in_one_piece_on_backspace_after_them_or_delete_before_them_leaving_the_delete_to_the_field() {
        let ranges = picked_reference_ranges(TEXT, SESSIONS, FILES);
        let whole = Some(KeyAction { selection: (8, 19), prevent_default: false });
        assert_eq!(reference_key_action(RefKey::Backspace, 19, &ranges), whole);
        assert_eq!(reference_key_action(RefKey::Delete, 8, &ranges), whole);
        assert_eq!(reference_key_action(RefKey::Backspace, 8, &ranges), None);
        assert_eq!(reference_key_action(RefKey::Delete, 19, &ranges), None);
        assert_eq!(reference_key_action(RefKey::Backspace, 20, &ranges), None);
    }

    #[test]
    fn are_stepped_over_by_the_arrows() {
        let ranges = picked_reference_ranges(TEXT, SESSIONS, FILES);
        assert_eq!(reference_key_action(RefKey::Left, 19, &ranges), Some(KeyAction { selection: (8, 8), prevent_default: true }));
        assert_eq!(reference_key_action(RefKey::Right, 8, &ranges), Some(KeyAction { selection: (19, 19), prevent_default: true }));
        assert_eq!(reference_key_action(RefKey::Left, 8, &ranges), None);
    }

    #[test]
    fn never_keep_the_caret_inside_it_moves_to_the_nearer_edge() {
        let ranges = picked_reference_ranges(TEXT, SESSIONS, FILES);
        assert_eq!(caret_outside_references(10, &ranges), 8);
        assert_eq!(caret_outside_references(17, &ranges), 19);
        assert_eq!(caret_outside_references(21, &ranges), 21);
    }
}

#[test]
fn tints_known_skills_by_how_theyre_sent_and_leaves_other_dollar_text_plain() {
    let skills = [("fleet-debug", Attached), ("fleet-simplify", Note)];
    let known = Known { skills: &skills, ..Known::default() };
    assert_eq!(
        split_draft_references("Use $fleet-debug, then $fleet-simplify on $HOME", None, &known),
        vec![
            plain("Use "),
            reference("$fleet-debug", RefKind::Skill { note: false }),
            plain(", then "),
            reference("$fleet-simplify", RefKind::Skill { note: true }),
            plain(" on $HOME")
        ]
    );
}

#[test]
fn keeps_a_skill_the_caret_is_still_typing_plain() {
    let skills = [("fleet-debug", Attached)];
    let known = Known { skills: &skills, ..Known::default() };
    assert_eq!(split_draft_references("Use $fleet-debug", Some(16), &known), vec![plain("Use $fleet-debug")]);
}

#[test]
fn a_skill_in_backticks_is_code_not_a_reference() {
    let skills = [("fleet-debug", Attached)];
    let known = Known { skills: &skills, ..Known::default() };
    assert_eq!(split_draft_references("`$fleet-debug`", None, &known), vec![plain("`$fleet-debug`")]);
}
