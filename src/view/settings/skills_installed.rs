//! Settings → Skills & tools → Skills → Installed (`skills/InstalledSkillsTab.vue`): a card per install with its badges, where
//! it came from, the folders Fleet wrote it to, and Check for updates, Update and Remove (which asks first).
use super::skills_tools::{empty_state, error_box, loading};
use crate::State;
use crate::fleet::settings::skills::{InstalledSkill, SkillsPage, Source};
use crate::icons::{BRAIN, REFRESH_CW, TRASH_2, TRIANGLE_ALERT};
use crate::theme::{RADIUS_CARD, Theme, mono, sans};
use crate::ui::button::{Size, Variant, button, button_lead, disabled};
use crate::ui::confirm::spinner;
use crate::ui::controls::{BadgeVariant, badge};
use crate::ui::rem;
use rune::{El, col, icon, row, text};

pub(super) fn small_mono(t: &Theme, words: &str) -> El {
    text(words.to_string()).font(mono(rem(0.75), 400.0, 1.33)).color(t.muted).wrap().shrink().min_w(0.0)
}

/// The title row: the name and its badges (target, "Update Available", "Bundled").
fn title(t: &Theme, skill: &InstalledSkill, id: &str) -> El {
    let mut r = row()
        .gap(7.0)
        .center_y()
        .child(icon(&BRAIN, 16.0).color(t.muted))
        .child(text(skill.name.clone()).font(sans(rem(0.875), 600.0, 1.43)).color(t.text).truncate().shrink().min_w(0.0))
        // The tooltip of Fleet's badge (the repository's path, or "Every session") is its label here: Rune has no tooltips.
        .child(badge(t, BadgeVariant::Outline, &skill.target.label()).key(format!("skill-target-{id}")).label(skill.target.tooltip()));
    if skill.update_available {
        r = r.child(badge(t, BadgeVariant::Default, "Update Available").key(format!("skill-update-badge-{id}")).ml(7.0));
    }
    if skill.source == Source::Bundled {
        r = r.child(badge(t, BadgeVariant::Secondary, "Bundled"));
    }
    r
}

/// `border-coral/30 bg-coral/10`: a skill no harness reads.
pub(super) fn unreachable_note(t: &Theme, key: String, words: &str) -> El {
    row()
        .key(key)
        .mt(7.0)
        .gap(7.0)
        .items_start()
        .px(7.0)
        .py(3.5)
        .radius(RADIUS_CARD)
        .border(1.0, t.coral.alpha(0.3))
        .border_layout()
        .fill(t.coral.alpha(0.1))
        .child(icon(&TRIANGLE_ALERT, 12.0).color(t.coral_text).mt(2.0))
        .child(text(words.to_string()).font(sans(rem(0.75), 400.0, 1.33)).color(t.coral_text).wrap().shrink().min_w(0.0))
}

/// A button whose label and spinner change while its work runs.
pub(super) fn working(t: &Theme, v: Variant, glyph: &'static rune::Icon, ink: rune::Color, busy: bool, idle: &str, doing: &str) -> El {
    let lead = if busy { spinner(ink) } else { icon(glyph, 16.0).color(ink) };
    button_lead(t, v, Size::Sm, Some(lead), if busy { doing } else { idle })
}

fn buttons(s: &State, t: &Theme, skill: &InstalledSkill, id: &str) -> El {
    let page = &s.settings.skills;
    let key = skill.key();
    let mine = |slot: &Option<String>| slot.as_deref() == Some(key.as_str());
    let (checking, updating, removing) = (mine(&page.checking), mine(&page.updating), mine(&page.removing));
    let mut r = row().gap(7.0).wrap();
    if skill.source == Source::GitHub {
        let b = working(t, Variant::Outline, &REFRESH_CW, t.text, checking, "Check for updates", "Checking\u{2026}")
            .key(format!("skill-check-{id}"));
        r = r.child(if checking || updating {
            disabled(b)
        } else {
            let key = key.clone();
            b.on_press(move |s: &mut State| s.check_skill_update(&key))
        });
    }
    if skill.update_available {
        let b = working(t, Variant::Default, &REFRESH_CW, t.primary_fg, updating, "Update", "Updating\u{2026}")
            .key(format!("skill-update-{id}"));
        r = r.child(if updating {
            disabled(b)
        } else {
            let key = key.clone();
            b.on_press(move |s: &mut State| s.update_skill(&key))
        });
    }
    if skill.source != Source::Bundled {
        let confirming = mine(&page.confirm_remove);
        let idle = if confirming { "Confirm remove" } else { "Remove" };
        let b = working(t, Variant::Destructive, &TRASH_2, rune::Color::WHITE, removing, idle, "Removing\u{2026}")
            .key(format!("skill-remove-{id}"));
        r = r.child(if removing {
            disabled(b)
        } else {
            let key = key.clone();
            b.on_press(move |s: &mut State| s.press_remove_skill(&key))
        });
        if confirming && !removing {
            r = r.child(
                button(t, Variant::Outline, Size::Sm, None, "Cancel")
                    .key(format!("skill-cancel-{id}"))
                    .on_press(|s: &mut State| s.cancel_remove_skill()),
            );
        }
    }
    r
}

/// One install: `rounded-card border bg-main-bg p-4`, the words on the left and the buttons on the right.
fn card(s: &State, t: &Theme, skill: &InstalledSkill) -> El {
    let id = skill.key();
    let mut words = col().w(0.0).grow(1.0).min_w(0.0).child(title(t, skill, &id));
    if let Some(description) = &skill.description {
        words = words.child(
            row().mt(7.0).child(text(description.clone()).font(sans(rem(0.875), 400.0, 1.43)).color(t.muted).wrap().shrink().min_w(0.0)),
        );
    }
    if let Some(path) = skill.path_line() {
        words = words.child(row().key(format!("skill-path-{id}")).mt(10.5).child(small_mono(t, path)));
    }
    for installed in &skill.installed_paths {
        words = words.child(row().mt(3.5).child(small_mono(t, &format!("\u{2192} {installed}"))));
    }
    if skill.installed_paths.is_empty() {
        words = words.child(unreachable_note(
            t,
            format!("skill-no-folder-{id}"),
            "Not in any folder a harness reads. Remove it and install it again.",
        ));
    }
    if let Some(e) = &skill.update_check_error {
        words = words.child(col().mt(7.0).child(error_box(t, &format!("skill-check-error-{id}"), e)));
    }
    row()
        .key(format!("skill-{id}"))
        .w_full()
        .gap(14.0)
        .items_start()
        .justify_between()
        .padding(14.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.main_bg)
        .child(words)
        .child(buttons(s, t, skill, &id))
}

fn empty(t: &Theme) -> El {
    empty_state(t, "skills-empty", &BRAIN, "No skills installed", "Install a skill from the Catalog or Custom tab.")
}

/// The tab: a loading line, the error alone when there is nothing to list, the empty state, or the cards with the error under.
pub fn tab(s: &State, t: &Theme) -> El {
    let page: &SkillsPage = &s.settings.skills;
    let has = !page.skills.is_empty();
    let mut body = col().key("skills-installed").w_full().gap(14.0);
    if page.loading {
        body = body.child(loading(t, "skills-loading", "Loading skills\u{2026}"));
    } else if let (Some(e), false) = (&page.error, has) {
        body = body.child(error_box(t, "skills-error", e));
    } else if !has {
        body = body.child(empty(t));
    } else {
        let mut list = col().key("skills-list").w_full().gap(10.5);
        for skill in &page.skills {
            list = list.child(card(s, t, skill));
        }
        body = body.child(list);
    }
    if let (Some(e), true) = (&page.error, has) {
        body = body.child(error_box(t, "skills-error", e));
    }
    body
}
