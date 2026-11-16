//! The notice card's scenes (piece 9L): the four looks of `NoticeCard.vue`, each posted and opened the way the store
//! would at a quiet moment. Invented text throughout.
use crate::State;
use crate::fleet::notices::{Action, ActionTone, IconRef, Item, ItemKind, Link, Notice, Tone};
use crate::icons::{LIGHTBULB, ROTATE_CW, UNDO_2};

#[derive(Clone, Copy)]
pub enum Case {
    /// A title, a line and a link.
    Plain,
    /// A list to read, with a "more" line and buttons.
    Items,
    /// The amber tint, for a card that asks before something stops work.
    Warn,
    /// A card with an Undo that only lasts the hold, and its bar draining.
    Countdown,
}

fn action(label: &str, tone: ActionTone, run: &str) -> Action {
    Action { label: label.into(), icon: None, tone, run: run.into() }
}

fn item(label: Option<&str>, text: &str, kind: ItemKind) -> Item {
    Item { text: text.into(), label: label.map(Into::into), kind: Some(kind) }
}

pub fn notice(case: Case) -> Notice {
    match case {
        Case::Plain => Notice {
            body: Some("Installs when you restart.".into()),
            link: Some(Link { label: "What's new".into(), href: "https://example.test/notes".into(), run: None }),
            ..Notice::new("scene:plain", "Fleet 0.37.0 is ready").chip("Fleet 0.37.0 ready")
        },
        Case::Items => Notice {
            items: vec![
                item(Some("Sessions"), "Pick up where a restart cut a session off.", ItemKind::New),
                item(Some("Composer"), "Drafts are kept for every session.", ItemKind::Highlight),
                item(None, "The list no longer jumps while a session is archived.", ItemKind::Fixed),
            ],
            more: Some("+2 more fixes".into()),
            actions: vec![action("Later", ActionTone::Quiet, "later"), action("Restart now", ActionTone::Primary, "restart")],
            ..Notice::new("scene:items", "Fleet 0.37.0 is ready").chip("Fleet 0.37.0 ready")
        },
        Case::Warn => Notice {
            icon: Some(IconRef(&ROTATE_CW)),
            tone: Tone::Warn,
            body: Some("3 sessions were cut off.".into()),
            actions: vec![action("Continue all", ActionTone::Primary, "continue"), action("Dismiss all", ActionTone::Quiet, "dismiss")],
            ..Notice::new("scene:warn", "Fleet restarted").chip("3 sessions cut off")
        },
        Case::Countdown => Notice {
            icon: Some(IconRef(&LIGHTBULB)),
            body: Some("Run the tests before you push.".into()),
            countdown: true,
            hold_secs: Some(6.0),
            actions: vec![Action { icon: Some(IconRef(&UNDO_2)), ..action("Undo", ActionTone::Quiet, "undo") }],
            ..Notice::new("scene:countdown", "Remembered for this repository")
        },
    }
}

/// Posts the case's notice and opens its card now.
pub fn show(s: &mut State, case: Case) {
    s.notices.post(notice(case), s.clock);
    s.notices.open_next();
}
