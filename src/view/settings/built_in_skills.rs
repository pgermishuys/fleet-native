//! The Built-in skills plugin's page body (`skills/BuiltInSkillsTab.vue`): the intro and a card for each of Fleet's own skills,
//! with its switch, which copy sessions get, and when Fleet changed its version since the user made theirs, what changed. The
//! page frame (title, breadcrumb) is `plugin_page`'s. The Editor and History dialogs this page opens are piece AL's.
use super::kit::{self, SwitchState};
use super::skills_tools::{error_box, loading};
use crate::State;
use crate::fleet::settings::built_in_skills::{BuiltInPage, PLUGIN_ID};
use crate::fleet::skills::{BuiltInSkill, version_label};
use crate::icons::SPARKLES;
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, mono, sans};
use crate::ui::confirm::spinner;
use crate::ui::rem;
use rune::{Cursor, El, Span, col, icon, rich, row, text};

const LINK: f32 = 13.0;

fn intro(t: &Theme) -> El {
    let (head, code, tail) = (
        "Skills that come with Fleet. Each one is off until you turn it on, and sessions you start afterwards get it. Their names start with ",
        "fleet-",
        ", so they never take the place of a skill of your own. Edit one to make it work your way: your version replaces Fleet's in new sessions, and Fleet's updates never overwrite it.",
    );
    let start = head.len();
    rich(format!("{head}{code}{tail}"), vec![Span::new(start, start + code.len()).mono().size(rem(0.75)).color(t.text)])
        .key("built-in-skills-intro")
        .font(sans(rem(0.875), 400.0, 1.43))
        .color(t.muted)
        .wrap()
        .w_full()
}

/// `.skill-pill`: 11 px, weight 600, `0 7px`, fully round.
fn pill(key: String, words: &str, ink: rune::Color, fill: Option<rune::Color>, outline: Option<rune::Color>) -> El {
    let mut p = row()
        .key(key)
        .label(words.to_string())
        .px(7.0)
        .radius(1000.0)
        .child(text(words.to_string()).font(sans(11.0, 600.0, 1.5)).color(ink));
    if let Some(f) = fill {
        p = p.fill(f);
    }
    if let Some(o) = outline {
        p = p.border(1.0, o).border_layout();
    }
    p
}

/// `.skill-links button`: 13 px, weight 500, the accent text; half-faded while a request runs.
fn link(t: &Theme, key: String, words: &str, off: bool, press: impl Fn(&mut State) + 'static) -> El {
    let l = row()
        .key(key)
        .role("button")
        .label(words.to_string())
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 0.0, t.accent)
        .child(text(words.to_string()).font(sans(LINK, 500.0, 1.5)).color(t.accent_text));
    if off { l.opacity(0.5).disabled(true).cursor(Cursor::Default) } else { l.on_press(press) }
}

fn title(t: &Theme, skill: &BuiltInSkill) -> El {
    let name = &skill.name;
    let version = if skill.version.is_some_and(|v| v > 0) {
        pill(format!("built-in-skill-version-{name}"), &version_label(skill.version), t.accent_text, Some(t.accent_dim), None)
    } else {
        pill(format!("built-in-skill-version-{name}"), &version_label(skill.version), t.muted, None, Some(t.border))
    };
    let mut r = row()
        .w_full()
        .flex_wrap()
        .items_center()
        .gap(7.0)
        .child(icon(&SPARKLES, 16.0).color(t.muted))
        .child(text(name.clone()).font(mono(rem(0.875), 600.0, 1.43)).color(t.text).truncate().shrink().min_w(0.0))
        .child(version);
    if skill.fleet_changed {
        let waiting = t.status_waiting();
        r = r.child(pill(
            format!("built-in-skill-fleet-changed-pill-{name}"),
            "Fleet changed it",
            t.status_waiting_text,
            Some(waiting.alpha(0.14)),
            None,
        ));
    }
    r
}

/// The right side: this one's spinner while it saves, then a switch, or for a skill that comes with Mods a line saying so.
fn side(s: &State, t: &Theme, skill: &BuiltInSkill) -> El {
    let page = &s.settings.built_in;
    let name = skill.name.clone();
    if skill.with_mods {
        let words = if skill.enabled { "On with Mods" } else { "Off \u{b7} turns on with Mods" };
        return row()
            .gap(7.0)
            .items_center()
            .child(text(words).key(format!("built-in-skill-mods-{name}")).label(words).font(sans(rem(0.75), 400.0, 1.33)).color(t.muted));
    }
    let state = SwitchState { saving: page.saving.as_deref() == Some(name.as_str()), disabled: page.saving.is_some(), pending: false };
    let (key, label, on) = (format!("built-in-skill-switch-{name}"), format!("Use {name} in new sessions"), skill.enabled);
    kit::setting_switch(t, &key, &label, on, state, move |s: &mut State| s.built_in_set_enabled(&name, !on))
}

/// `.skill-changed`: the amber box with its three links, and the diff when it is open.
fn changed_box(s: &State, t: &Theme, skill: &BuiltInSkill) -> El {
    let page = &s.settings.built_in;
    let name = &skill.name;
    let waiting = t.status_waiting();
    let (head, tail) = ("Fleet changed its version", " since you made yours. Sessions still use yours.");
    let sentence = rich(format!("{head}{tail}"), vec![Span::new(0, head.len()).weight(600.0)])
        .font(sans(LINK, 400.0, 1.5))
        .color(t.text)
        .wrap()
        .w_full();
    let open = page.fleet_change.as_ref().filter(|c| &c.name == name);
    let off = page.busy.is_some();
    let (a, b, c) = (name.clone(), name.clone(), name.clone());
    let links = row()
        .flex_wrap()
        .items_center()
        .gaps(4.0, 16.0)
        .child(link(
            t,
            format!("built-in-skill-see-change-{name}"),
            if open.is_some() { "Hide what changed" } else { "See what changed" },
            false,
            move |s: &mut State| s.built_in_toggle_change(&a),
        ))
        .child(link(t, format!("built-in-skill-keep-mine-{name}"), "Keep mine", off, move |s: &mut State| s.built_in_keep_mine(&b)))
        .child(link(t, format!("built-in-skill-fleets-in-box-{name}"), "Use Fleet's", off, move |s: &mut State| s.built_in_use_fleets(&c)));
    let mut b = col()
        .key(format!("built-in-skill-fleet-changed-{name}"))
        .w_full()
        .gap(8.0)
        .px(12.0)
        .py(10.0)
        .radius(RADIUS_BTN)
        .border(1.0, waiting.alpha(0.35))
        .border_layout()
        .fill(waiting.alpha(0.07))
        .child(sentence)
        .child(links);
    if let Some(change) = open {
        b = b.child(crate::view::skill_diff::skill_diff(
            t,
            s,
            &format!("built-in-skill-diff-{name}"),
            &change.before,
            &change.after,
            "What Fleet changed in its version",
        ));
    }
    b
}

fn links(s: &State, t: &Theme, skill: &BuiltInSkill) -> El {
    let page = &s.settings.built_in;
    let name = &skill.name;
    let mut r = row().w_full().flex_wrap().items_center().gaps(4.0, 16.0);
    let n = name.clone();
    r = r.child(link(t, format!("built-in-skill-edit-{name}"), "Edit", false, move |s: &mut State| s.built_in_edit(&n)));
    if skill.version_count > 0 {
        let n = name.clone();
        r = r.child(link(t, format!("built-in-skill-history-{name}"), "History", false, move |s: &mut State| s.built_in_history(&n)));
    }
    if skill.version.is_some_and(|v| v > 0) && !skill.fleet_changed {
        let n = name.clone();
        r = r.child(link(t, format!("built-in-skill-fleets-{name}"), "Use Fleet's", page.busy.is_some(), move |s: &mut State| {
            s.built_in_use_fleets(&n)
        }));
    }
    if page.busy.as_deref() == Some(name.as_str()) {
        r = r.child(spinner(t.muted));
    }
    r
}

fn card(s: &State, t: &Theme, skill: &BuiltInSkill) -> El {
    let top = row().w_full().items_start().justify_between().gap(14.0).child(
        col().grow(1.0).shrink().min_w(0.0).child(title(t, skill)).child(
            row()
                .mt(7.0)
                .child(text(skill.description.clone()).font(sans(rem(0.875), 400.0, 1.43)).color(t.muted).wrap().shrink().min_w(0.0)),
        ),
    );
    let mut c = col()
        .key(format!("built-in-skill-{}", skill.name))
        .w_full()
        .gap(10.5)
        .padding(14.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.main_bg)
        .child(top.child(side(s, t, skill)));
    if skill.fleet_changed {
        c = c.child(changed_box(s, t, skill));
    }
    c.child(links(s, t, skill))
}

/// The body: the intro, then a spinner while the first read runs (only while there is nothing to list) or the cards, then the
/// note. An empty list draws the intro alone: Fleet has no empty state here (8.61).
pub fn body(s: &State, t: &Theme) -> El {
    let page: &BuiltInPage = &s.settings.built_in;
    let mut b = col().key(format!("plugin-config-{PLUGIN_ID}")).w_full().gap(14.0).child(intro(t));
    if page.show_loading() {
        b = b.child(loading(t, "built-in-skills-loading", "Loading built-in skills\u{2026}"));
    } else {
        let mut grid = col().key("built-in-skills-list").w_full().gap(10.5);
        for skill in &page.list {
            grid = grid.child(card(s, t, skill));
        }
        b = b.child(grid);
    }
    if let Some(why) = page.note() {
        b = b.child(error_box(t, "built-in-skills-error", why));
    }
    b
}
