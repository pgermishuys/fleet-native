//! The Showcase under the kept mods (`mods/ModsShowcase.vue`): Fleet's own sample mods, what each does, where to see it, and Add.
use super::kit;
use super::mods_tab::xs_button;
use crate::State;
use crate::fleet::settings::mods_showcase::{GROUPS, Sample};
use crate::theme::{RADIUS_CARD, Theme, mono, sans};
use crate::ui::confirm::spinner;
use crate::ui::controls::separator;
use crate::ui::rem;
use rune::{El, Span, col, rich, row, text};

/// The round "Showcase" tag: a 1px border at 40% accent, 10px uppercase.
fn tag(t: &Theme) -> El {
    row()
        .px(5.25)
        .py(0.5)
        .radius(999.0)
        .border(1.0, t.accent.alpha(0.4))
        .border_layout()
        .child(text("SHOWCASE").font(sans(10.0, 500.0, 1.5)).color(t.accent_text))
}

/// "Added" once the sample is kept and on, else Add (a spinner while it is the one being added; every Add waits meanwhile).
fn action(s: &State, t: &Theme, sample: &Sample) -> El {
    let page = &s.settings.skills.mods;
    if page.is_added(sample.name) {
        return text("Added").key(format!("mods-sample-added-{}", sample.name)).font(sans(rem(0.75), 400.0, 1.33)).color(t.muted).py(7.0);
    }
    let lead = (page.adding.as_deref() == Some(sample.name)).then(|| spinner(t.text));
    let b = xs_button(t, &format!("mods-sample-add-{}", sample.name), "Add", lead).label(format!("Add {}", sample.name));
    if page.adding.is_some() {
        b.opacity(0.6).disabled(true)
    } else {
        let name = sample.name;
        b.on_press(move |s: &mut State| s.add_mod_sample(name))
    }
}

fn sample_row(s: &State, t: &Theme, sample: &Sample) -> El {
    let mut words = col()
        .w(0.0)
        .grow(1.0)
        .min_w(0.0)
        .child(
            row()
                .flex_wrap()
                .items_center()
                .gap(7.0)
                .child(text(sample.name).font(mono(rem(0.875), 600.0, 1.43)).color(t.text).wrap_anywhere())
                .child(tag(t)),
        )
        .child(row().mt(3.5).child(text(sample.description).font(sans(rem(0.875), 400.0, 1.43)).color(t.muted).wrap().shrink().min_w(0.0)))
        .child({
            let all = format!("Try it: {}", sample.try_it);
            row().mt(3.5).child(
                rich(all, vec![Span::new(0, 7).weight(500.0).color(t.text)])
                    .font(sans(rem(0.75), 400.0, 1.33))
                    .color(t.muted)
                    .wrap()
                    .shrink()
                    .min_w(0.0),
            )
        });
    if let Some((name, why)) = &s.settings.skills.mods.sample_error
        && name == sample.name
    {
        words = words.child(kit::error_note(t, &format!("mods-sample-error-{}", sample.name), why));
    }
    row()
        .key(format!("mods-sample-{}", sample.name))
        .w_full()
        .px(14.0)
        .py(10.5)
        .gap(10.5)
        .items_start()
        .justify_between()
        .child(words)
        .child(action(s, t, sample))
}

pub fn showcase(s: &State, t: &Theme) -> El {
    let mut c =
        col().key("mods-showcase").w_full().mt(28.0).child(text("Showcase").font(sans(rem(0.875), 600.0, 1.43)).color(t.text)).child(
            row().mt(3.5).child(
                text(concat!(
                    "Fleet's own sample mods, to see what a mod can do. Add one and it's kept like any mod: on in every session, ",
                    "with its switch in the list above."
                ))
                .font(sans(rem(0.875), 400.0, 1.43))
                .color(t.muted)
                .wrap()
                .shrink()
                .min_w(0.0),
            ),
        );
    for group in GROUPS {
        let mut list = col().w_full().radius(RADIUS_CARD).border(1.0, t.border).border_layout().clip_rounded(RADIUS_CARD);
        for (i, sample) in group.samples.iter().enumerate() {
            if i > 0 {
                list = list.child(separator(t));
            }
            list = list.child(sample_row(s, t, sample));
        }
        c = c.child(
            col().mt(14.0).child(text(group.title.to_uppercase()).font(sans(rem(0.75), 500.0, 1.33)).color(t.muted).mb(5.25)).child(list),
        );
    }
    c
}
