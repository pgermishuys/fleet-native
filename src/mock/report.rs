//! The Report a problem scenes (piece Q): Describe with a screenshot and without one, the second as the app shows it on
//! every machine today, since Rune can't hand the app its own frame (`docs/RUNE.md`). The picture is the invented shop page
//! the image scenes use; the description is invented.
use crate::State;
use crate::fleet::report::Source;
use crate::fleet::report_form::Shot;
use rune::TextEdit;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Case {
    /// Just opened: nothing typed, the screenshot taken.
    Fresh,
    /// The same with words in the three fields.
    Filled,
    /// Opened with no screenshot, the way it opens today.
    NoShot,
}

pub fn shot() -> Shot {
    let (width, height) = png_size(super::images::PAGE);
    Shot { png: super::images::PAGE.to_vec(), width, height, taken: "02:03:10 PM".into() }
}

fn png_size(png: &[u8]) -> (u32, u32) {
    let at = |i: usize| u32::from_be_bytes([png[i], png[i + 1], png[i + 2], png[i + 3]]);
    (at(16), at(20))
}

impl State {
    pub fn show_report(&mut self, case: Case) {
        self.set_open((0, 0));
        // The first invented session as a session Fleet knows, so the footer says what the report is about.
        self.projects[0].sessions[0].id = "s1".into();
        self.route = crate::fleet::store::Route::Session("s1".into());
        self.report_open(Source::Help, None, "");
        self.report_set_screenshot((case != Case::NoShot).then(shot));
        if case == Case::Filled {
            let f = &mut self.report_form;
            f.description = TextEdit::new("The list stopped updating after I archived two sessions. I had to restart to see new ones.");
            f.expected = TextEdit::new("New sessions to keep showing up.");
            f.contact = TextEdit::new("@sam-example");
            f.toggle(crate::fleet::report_form::Item::Log);
        }
    }
}
