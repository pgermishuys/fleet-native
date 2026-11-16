//! A page of every primitive, for the `settings-kit` scenes and the tests: Settings' own `SettingRow` with More open, a Labs
//! row with its Adds line, a switch saving, segments and an error note. Nothing in the app opens it.
use super::kit::{self, Row, SwitchState};
use crate::State;
use crate::fleet::settings_nav::Page;
use crate::theme::Theme;
use crate::ui::controls::{BadgeVariant, badge};
use rune::{El, col};
use std::rc::Rc;

/// What the demo's segments hold, and whether its switch is on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Demo {
    pub choice: String,
    pub starters: bool,
    pub opened: u32,
}

impl Default for Demo {
    fn default() -> Self {
        Demo { choice: "ask".into(), starters: true, opened: 0 }
    }
}

pub fn content(s: &State, t: &Theme) -> El {
    let demo = &s.settings.demo;
    let starters = Row::new("starters", "Show starters")
        .summary("A few sentences to start from on the new-session page.")
        .more(
            "On the new-session page, offer a few sentences to start from, such as \"Explore the code\". A starter fills the box; \
             nothing is sent until you press Enter.",
        )
        .control(kit::setting_switch(t, "kit-starters", "Show starters", demo.starters, SwitchState::default(), |s: &mut State| {
            s.settings.demo.starters = !s.settings.demo.starters
        }))
        .build(s, t);
    let jobs = Row::new("jobs", "Jobs")
        .summary("One queue; Fleet starts each job wherever a seat is free.")
        .adds("Jobs in the rail, seats on each machine", Some(|s: &mut State| s.settings.demo.opened += 1))
        .control(kit::setting_switch(t, "kit-jobs", "Jobs", true, SwitchState::default(), |_: &mut State| {}))
        .build(s, t);
    let quiet = Row::new("quiet", "Mission control")
        .badge(badge(t, BadgeVariant::Secondary, "Labs"))
        .summary("One screen for every running agent. It becomes the first screen.")
        .adds("Fleet in the rail, the first screen", None::<fn(&mut State)>)
        .control(kit::setting_switch(
            t,
            "kit-quiet",
            "Mission control",
            true,
            SwitchState { saving: true, disabled: true, pending: false },
            |_: &mut State| {},
        ))
        .build(s, t);
    let choice = Row::new("choice", "Run commands")
        .summary("Whether agents may run commands without asking.")
        .control(kit::segments(
            t,
            "kit-choice",
            "Run commands",
            &[("ask", "Ask"), ("allow", "Allow"), ("deny", "Deny")],
            &demo.choice,
            false,
            Rc::new(|s: &mut State, v: &str| s.settings.demo.choice = v.to_string()),
        ))
        .below(kit::error_note(t, "kit-error", "Couldn't save that. Try again."))
        .build(s, t);
    col()
        .key("settings-kit-page")
        .w_full()
        .gap(21.0)
        .child(super::super::settings::page_header(t, &Page::General))
        .child(kit::group(t, "new-sessions", "New sessions", None, vec![starters]))
        .child(kit::group(t, "experiments", "Labs", Some("Each is off until you turn it on."), vec![jobs, quiet, choice]))
}
