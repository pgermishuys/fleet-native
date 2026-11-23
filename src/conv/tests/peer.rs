//! `lib/__tests__/session-messages.test.ts`, under the same names.
use crate::conv::peer::{Peer, PeerOutcome, PeerSender, parse_peer_message, parse_peer_update};

fn sender(id: &str, title: &str, machine: Option<(&str, &str)>) -> PeerSender {
    PeerSender {
        session_id: id.into(),
        title: title.into(),
        machine_id: machine.map(|m| m.0.into()),
        machine_name: machine.map(|m| m.1.into()),
    }
}

fn peer(sender: PeerSender, outcome: Option<PeerOutcome>, text: &str) -> Option<Peer> {
    Some(Peer { sender, outcome, text: text.into() })
}

mod parse_peer_message_cases {
    use super::*;

    #[test]
    fn returns_the_sender_and_the_text_of_a_message_another_session_sent() {
        let body = "<fleet-session-message from=\"ses_1\" title=\"Fix authentication flow\">\nUpdate the docs.\n</fleet-session-message>";
        assert_eq!(parse_peer_message(body), peer(sender("ses_1", "Fix authentication flow", None), None, "Update the docs."));
    }

    #[test]
    fn keeps_the_texts_own_lines_and_decodes_the_title_fleet_encoded() {
        let body =
            "<fleet-session-message from=\"ses_1\" title=\"Say &quot;hi&quot; &amp; &lt;go&gt;\">\nOne.\n\nTwo.\n</fleet-session-message>";
        assert_eq!(parse_peer_message(body), peer(sender("ses_1", "Say \"hi\" & <go>", None), None, "One.\n\nTwo."));
    }

    #[test]
    fn names_the_senders_machine_when_its_on_another_one() {
        let body = "<fleet-session-message from=\"ses_1\" title=\"Fix login flake\" machine=\"m-atlas\" machine-name=\"atlas &amp; co\">\nCheck the installer.\n</fleet-session-message>";
        assert_eq!(
            parse_peer_message(body),
            peer(sender("ses_1", "Fix login flake", Some(("m-atlas", "atlas & co"))), None, "Check the installer.")
        );
    }

    #[test]
    fn ignores_a_prompt_that_only_mentions_the_tag() {
        assert_eq!(parse_peer_message("Why does <fleet-session-message> show up here?"), None);
        assert_eq!(parse_peer_message("Look: <fleet-session-message from=\"a\" title=\"b\">\nx\n</fleet-session-message>"), None);
    }
}

mod parse_peer_update_cases {
    use super::*;

    #[test]
    fn returns_the_session_how_its_turn_ended_and_its_reply() {
        let body = "<fleet-session-update session=\"ses_2\" title=\"Update documentation\" outcome=\"finished\">\nDocs updated.\n</fleet-session-update>";
        assert_eq!(
            parse_peer_update(body),
            peer(sender("ses_2", "Update documentation", None), Some(PeerOutcome::Finished), "Docs updated.")
        );
    }

    #[test]
    fn names_the_machine_of_a_session_on_another_one() {
        let body = "<fleet-session-update session=\"ses_2\" title=\"Run the tests\" outcome=\"finished\" machine=\"m-mini\" machine-name=\"mini\">\n2 failed.\n</fleet-session-update>";
        assert_eq!(
            parse_peer_update(body),
            peer(sender("ses_2", "Run the tests", Some(("m-mini", "mini"))), Some(PeerOutcome::Finished), "2 failed.")
        );
    }

    #[test]
    fn reads_a_failure_and_decodes_the_title() {
        let body = "<fleet-session-update session=\"ses_2\" title=\"Docs &amp; tests\" outcome=\"failed\">\nRate limited.\n</fleet-session-update>";
        assert_eq!(parse_peer_update(body), peer(sender("ses_2", "Docs & tests", None), Some(PeerOutcome::Failed), "Rate limited."));
    }

    #[test]
    fn is_not_a_message_and_a_message_is_not_an_update() {
        let message = "<fleet-session-message from=\"ses_1\" title=\"Fix authentication flow\">\nHi.\n</fleet-session-message>";
        let update = "<fleet-session-update session=\"ses_2\" title=\"Docs\" outcome=\"finished\">\nDone.\n</fleet-session-update>";
        assert_eq!(parse_peer_update(message), None);
        assert_eq!(parse_peer_message(update), None);
        assert_eq!(
            parse_peer_update("<fleet-session-update session=\"a\" title=\"b\" outcome=\"maybe\">\nx\n</fleet-session-update>"),
            None
        );
    }
}

mod terminal_codes_in_a_message_from_another_session {
    use super::*;

    #[test]
    fn drops_them_from_the_senders_title_and_the_text() {
        let body = "<fleet-session-message from=\"ses_1\" title=\"\u{1b}[31mFix\u{1b}[0m login\">\n\u{1b}[32mDone\u{1b}[0m, see above.\n</fleet-session-message>";
        assert_eq!(parse_peer_message(body), peer(sender("ses_1", "Fix login", None), None, "Done, see above."));
    }

    #[test]
    fn drops_them_from_an_updates_title_and_text() {
        let body = "<fleet-session-update session=\"ses_1\" title=\"\u{1b}[1mDocs\u{1b}[0m\" outcome=\"finished\">\n\u{1b}[32mAll green\u{1b}[0m\n</fleet-session-update>";
        assert_eq!(parse_peer_update(body), peer(sender("ses_1", "Docs", None), Some(PeerOutcome::Finished), "All green"));
    }
}
