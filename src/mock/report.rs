//! The Report a problem scenes (pieces Q and R): Describe with a screenshot and without one, the second as the app shows it on
//! every machine today, since Rune can't hand the app its own frame (`docs/RUNE.md`). The picture is the invented shop page
//! the image scenes use; the description is invented.
use crate::State;
use crate::fleet::report::{Prepared, Replacement, Source, Stage};
use crate::fleet::report_form::{ReviewFile, Shot};
use rune::TextEdit;

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Case {
    /// Just opened: nothing typed, the screenshot taken.
    Fresh,
    /// The same with words in the three fields.
    Filled,
    /// Opened with no screenshot, the way it opens today.
    NoShot,
    /// Review of a prepared report.
    Review,
    /// Review with "Show what was removed" on.
    Removed,
    /// Review of the log file.
    Log,
    /// Review of the screenshot.
    ShotView,
    /// Review of a Fleet that has no inbox, and a line left out.
    NoInbox,
    /// A send that failed.
    SendFailed,
    /// Sent.
    Sent,
}

pub fn shot() -> Shot {
    let (width, height) = png_size(super::images::PAGE);
    Shot { png: super::images::PAGE.to_vec(), width, height, taken: "02:03:10 PM".into() }
}

fn png_size(png: &[u8]) -> (u32, u32) {
    let at = |i: usize| u32::from_be_bytes([png[i], png[i + 1], png[i + 2], png[i + 3]]);
    (at(16), at(20))
}

/// What a Fleet answers `/prepare` with for the filled-in description, invented: the same shape and wording as its own.
pub fn prepared() -> Prepared {
    let rep = |label: &str, kind: &str, shown: &str| Replacement { label: label.into(), kind: kind.into(), shown: shown.into() };
    Prepared {
        title: "The list stopped updating after I archived two sessions".into(),
        body: "## What happened\nThe list stopped updating after I archived two sessions in ‹folder-1›. I had to restart to see new ones.\n\n## What I expected\nNew sessions to keep showing up.\n\n## About\n- Session: ‹session-1›\n- Contact: ‹email-1›\n\n## Fleet and system\n- Fleet 0.72.0\n- Desktop app 0.1.0 · linux\n- Screen: Sessions\n- Live updates: connected\n".into(),
        log: Some("2026-11-16 14:02:03.911 [WRN] [SessionHub] subscription for ‹session-1› closed\n2026-11-16 14:02:04.120 [ERR] [Archive] could not read ‹folder-1›/notes.md\n".into()),
        log_entries: 2,
        labels: vec!["opencode2".into(), "desktop".into()],
        replacements: vec![
            rep("‹folder-1›", "folder", "/work/acme-payments"),
            rep("‹session-1›", "session", "Fix invoice rounding"),
            rep("‹email-1›", "email", "sam@example.com"),
        ],
        problems: Vec::new(),
        can_send: true,
    }
}

impl State {
    pub fn show_report(&mut self, case: Case) {
        self.set_open((0, 0));
        // The first invented session as a session Fleet knows, so the footer says what the report is about.
        self.projects[0].sessions[0].id = "s1".into();
        self.route = crate::fleet::store::Route::Session("s1".into());
        self.report_open(Source::Help, None, "");
        self.report_set_screenshot((case != Case::NoShot).then(shot));
        if case != Case::Fresh && case != Case::NoShot {
            let f = &mut self.report_form;
            f.description = TextEdit::new("The list stopped updating after I archived two sessions. I had to restart to see new ones.");
            f.expected = TextEdit::new("New sessions to keep showing up.");
            f.contact = TextEdit::new("@sam-example");
            if case == Case::Filled {
                f.toggle(crate::fleet::report_form::Item::Log);
            }
        }
        if case as u8 >= Case::Review as u8 {
            let mut p = prepared();
            if case == Case::NoInbox {
                p.can_send = false;
                p.problems = vec![("the screenshot".into(), "It was too large to attach.".into())];
            }
            self.report_reviewing(&p.title);
            self.report.prepared = Some(p);
            self.report.stage = Stage::Review;
            let f = &mut self.report_form;
            f.show_removed = case == Case::Removed;
            f.file = match case {
                Case::Log => ReviewFile::Log,
                Case::ShotView => ReviewFile::Screenshot,
                _ => ReviewFile::Report,
            };
            match case {
                Case::SendFailed => self.report.error = Some("Too many reports from this address. Try again in a few minutes.".into()),
                Case::Sent => {
                    self.report.stage = Stage::Sent;
                    self.report.sent_id = Some("r_abc234".into());
                }
                _ => {}
            }
        }
    }
}
