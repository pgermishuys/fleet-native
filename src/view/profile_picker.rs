//! Ports `NSD/ProfilePicker.vue`: the chip that names the harness profile a session starts with, and the menu above it (rows
//! B6.1-B6.12). The chip shows only when the harness has profiles; `fleet::new_session::harness` decides that.
use crate::fleet::new_session::harness::{NO_PROFILE, PROFILE_CHIP, profile_summary};
use crate::icons::*;
use crate::theme::Theme;
use crate::ui::chip::{Chip, Option_, option, pop, pop_label, pop_note, pop_separator};
use crate::{State, Tx};
use rune::{Align, El, Side};

const WIDTH: f32 = 320.0;

fn row_key(id: &str) -> String {
    format!("{PROFILE_CHIP}-{id}")
}

/// B6.2: a layers mark, the profile's name, a chevron.
pub fn chip(s: &State, t: &Theme) -> El {
    let disabled = s.new_draft.draft().is_some_and(|d| d.is_starting);
    Chip::new(PROFILE_CHIP, s.profile_chip_label()).glyph(&LAYERS).open(s.profile_menu_open()).disabled(disabled).view(
        t,
        |s: &mut State, tx: &mut Tx| {
            s.open_profile_menu();
            tx.focus(row_key(&s.selected_profile_id()), false);
        },
    )
}

/// B6.3-B6.9: the heading, No profile, each profile with its summary, a rule, the note, Manage profiles.
pub fn overlay(s: &State, t: &Theme) -> Vec<El> {
    let selected = s.selected_profile_id();
    let mut body = vec![
        pop_label(t, "Profile"),
        option(
            t,
            Option_ {
                detail: Some("Your opencode.json as it is".into()),
                checked: selected == NO_PROFILE,
                no_mark: true,
                ..Option_::new(row_key(NO_PROFILE), "No profile")
            },
            None,
            |s, _| s.choose_profile(NO_PROFILE),
        ),
    ];
    for p in s.page_profiles() {
        let id = p.id.clone();
        let spec = Option_ {
            detail: Some(profile_summary(&p.content)),
            checked: p.id == selected,
            tag: p.is_default.then(|| " default".to_string()),
            no_mark: true,
            ..Option_::new(row_key(&p.id), p.name.clone())
        };
        body.push(option(t, spec, None, move |s, _| s.choose_profile(&id)));
    }
    body.push(pop_separator(t));
    body.push(pop_note(t, "The session keeps this profile for as long as it lives."));
    body.push(option(t, Option_ { icon: Some(&LAYERS), ..Option_::new(row_key("manage"), "Manage profiles") }, None, |s, tx| {
        s.close_chip_menu();
        s.toast("Profiles are managed in Settings, which isn't in the native app yet (component 16).".into(), None, 4.0, tx);
    }));
    let panel = pop(t, "new-session-profile-pop", WIDTH, body)
        .focus_return(PROFILE_CHIP.to_string())
        .modal(true, |s: &mut State| s.close_chip_menu());
    vec![
        crate::ui::overlay::scrim("new-session-profile-scrim", |s: &mut State| s.close_chip_menu()),
        crate::ui::overlay::anchored(panel, PROFILE_CHIP, Side::Top, Align::Start, 6.0),
    ]
}
