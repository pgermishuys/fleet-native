//! Settings → Harnesses → Profiles (`HarnessProfilesPanel.vue`): the head with "New profile", the notice, the list with its
//! default, and a row's editor slot (piece I draws the editor; the slot is empty until then). Sizes are Fleet's Tailwind
//! classes at the 14px root.
use super::kit;
use crate::State;
use crate::fleet::new_session::feeds::Profile;
use crate::fleet::settings::harness_profiles::{Editing, Load, profile_summary, usage};
use crate::icons::*;
use crate::theme::{RADIUS_CARD, Theme, sans};
use crate::ui::button::{self, Size, Variant};
use crate::ui::{confirm::spinner, rem};
use rune::{Align, El, Span, col, icon, rich, row, text};

fn small(t: &Theme, words: &str) -> El {
    text(words.to_string()).font(sans(rem(0.75), 400.0, 1.33)).color(t.muted).wrap().shrink().min_w(0.0)
}

/// `rounded-full bg-accent/10 px-2 py-0.5 text-[10px] font-medium text-accent-text`, a 10px star first.
fn default_pill(t: &Theme, key: &str) -> El {
    row()
        .key(key.to_string())
        .center_y()
        .gap(3.5)
        .px(7.0)
        .py(1.75)
        .radius(999.0)
        .fill(t.accent.alpha(0.1))
        .child(icon(&STAR, 10.0).color(t.accent_text))
        .child(text("Default").font(sans(10.0, 500.0, 1.5)).color(t.accent_text))
}

/// The row's name line: `text-sm font-medium` and, when it is the default, the pill.
fn name_line(t: &Theme, name: &str, pill: Option<El>) -> El {
    let mut line = row().flex_wrap().center_y().gap(7.0).child(text(name.to_string()).font(sans(rem(0.875), 500.0, 1.43)).color(t.text));
    if let Some(pill) = pill {
        line = line.child(pill);
    }
    line
}

fn layers(t: &Theme) -> El {
    icon(&LAYERS, 14.0).color(t.muted).mt(1.75)
}

fn head(t: &Theme, kind: &str, name: &str, editing_new: bool) -> El {
    let font = sans(rem(0.75), 400.0, 1.33);
    let words = format!(
        "Extra {name} config that sits on top of your own opencode.json. You pick one when you start a session, and the session keeps it."
    );
    let at = words.find("opencode.json").unwrap_or(0);
    let spans = vec![Span::new(at, at + "opencode.json".len()).mono().color(t.text)];
    let kind = kind.to_string();
    let new = button::button(t, Variant::Outline, Size::Sm, Some(&PLUS), "New profile").key("harness-profile-new");
    let new = if editing_new {
        button::disabled(new)
    } else {
        new.on_press(move |s: &mut State| s.open_profile_editor(&kind, Editing::New(None)))
    };
    row()
        .w_full()
        .flex_wrap()
        .items_start()
        .justify_between()
        .gap(10.5)
        .child(
            col()
                .shrink()
                .min_w(0.0)
                .child(text("Profiles").font(sans(rem(0.875), 500.0, 1.43)).color(t.text))
                .child(row().mt(3.5).max_w(672.0).child(rich(words, spans).font(font).color(t.muted).wrap().shrink().min_w(0.0))),
        )
        .child(new)
}

/// `Spinner lg` centred in `p-6`, its label beside nothing: the words are the accessible label.
fn loading(t: &Theme, name: &str) -> El {
    let words = format!("Loading {name}'s profiles\u{2026}");
    row().key("harness-profiles-loading").w_full().justify_center().px(21.0).py(21.0).label(words).child(spinner(t.muted))
}

/// `ErrorNote` with "Try again".
fn load_error(t: &Theme, kind: &str, name: &str) -> El {
    let kind = kind.to_string();
    let retry = button::button(t, Variant::Outline, Size::Sm, None, "Try again")
        .key("harness-profiles-retry")
        .on_press(move |s: &mut State| {
            s.load_profiles(&kind);
        })
        .mt(7.0);
    let font = sans(rem(0.875), 400.0, 1.43);
    row()
        .key("harness-profiles-load-error")
        .role("alert")
        .w_full()
        .gap(7.0)
        .items_start()
        .px(10.5)
        .py(7.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.error.alpha(0.3))
        .border_layout()
        .fill(t.error.alpha(0.1))
        .child(icon(&CIRCLE_ALERT, 16.0).color(t.error_text).mt(1.0))
        .child(
            col()
                .shrink()
                .min_w(0.0)
                .child(text(format!("Couldn't load {name}'s profiles.")).font(font).color(t.error_text).wrap())
                .child(row().child(retry)),
        )
}

/// `EmptyState bordered`, whose dashed border is solid here (Rune has no dashed line).
fn empty(t: &Theme, name: &str) -> El {
    col()
        .key("harness-profiles-empty")
        .w_full()
        .items_center()
        .gap(3.5)
        .px(21.0)
        .py(21.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .child(icon(&LAYERS, 28.0).color(t.muted).mb(7.0))
        .child(text("No profiles yet").font(sans(rem(1.0), 600.0, 1.5)).color(t.text))
        .child(
            row().max_w(rem(40.0) * 1.0).child(
                text(format!(
                    "Every {name} session uses your opencode.json as it is, and the new-session box has no profile chip. Make a profile when some sessions need other providers, models or MCP servers."
                ))
                .font(sans(rem(0.875), 400.0, 1.43))
                .color(t.muted)
                .wrap()
                .align(Align::Center)
                .shrink()
                .min_w(0.0),
            ),
        )
}

/// What piece I fills: the editor. Until it is built there is a way out of the empty slot.
fn editor_slot(t: &Theme, kind: &str, key: &str) -> El {
    let kind = kind.to_string();
    row().key(key.to_string()).w_full().child(
        button::button(t, Variant::Ghost, Size::Sm, None, "Close")
            .key(format!("{key}-close"))
            .on_press(move |s: &mut State| s.close_profile_editor(&kind, None)),
    )
}

fn row_frame(t: &Theme, first: bool) -> El {
    let r = col().w_full();
    if first { r } else { r.border_sides([1.0, 0.0, 0.0, 0.0], t.border) }
}

fn none_row(t: &Theme, kind: &str, has_default: bool) -> El {
    let kind = kind.to_string();
    let mut r = row().key("harness-profile-row-none").w_full().items_start().gap(10.5).px(14.0).py(10.5).child(layers(t)).child(
        col()
            .grow(1.0)
            .shrink()
            .min_w(0.0)
            .child(name_line(t, "No profile", (!has_default).then(|| default_pill(t, "harness-profile-default-none"))))
            .child(row().mt(1.75).child(small(t, "Your opencode.json as it is"))),
    );
    if has_default {
        r = r.child(
            button::button(t, Variant::Ghost, Size::Sm, Some(&STAR), "Set default")
                .key("harness-profile-default-clear")
                .on_press(move |s: &mut State| s.set_profile_default(&kind, None)),
        );
    }
    r
}

fn profile_row(t: &Theme, kind: &str, p: &Profile, editing: bool) -> El {
    let (set_kind, edit_kind, id) = (kind.to_string(), kind.to_string(), p.id.clone());
    let mut buttons = row().center_y().gap(3.5);
    if !p.is_default {
        let id = id.clone();
        buttons = buttons.child(
            button::button(t, Variant::Ghost, Size::Sm, Some(&STAR), "Set default")
                .key(format!("harness-profile-default-{}", p.id))
                .on_press(move |s: &mut State| s.set_profile_default(&set_kind, Some(&id))),
        );
    }
    let edit_id = id.clone();
    buttons = buttons.child(
        button::button(t, Variant::Outline, Size::Sm, Some(&PENCIL), "Edit")
            .key(format!("harness-profile-edit-{}", p.id))
            .on_press(move |s: &mut State| s.open_profile_editor(&edit_kind, Editing::Edit(edit_id.clone()))),
    );
    let top = row().w_full().items_start().gap(10.5).px(14.0).py(10.5).child(layers(t)).child(
        col()
            .grow(1.0)
            .shrink()
            .min_w(0.0)
            .child(name_line(t, &p.name, p.is_default.then(|| default_pill(t, &format!("harness-profile-default-pill-{}", p.id)))))
            .child(row().mt(1.75).child(small(t, &profile_summary(&p.content)).key(format!("harness-profile-summary-{}", p.id))))
            .child(row().mt(1.75).child(
                text(usage(p.open_sessions)).key(format!("harness-profile-usage-{}", p.id)).font(sans(11.0, 400.0, 1.4)).color(t.muted),
            )),
    );
    let top = if editing { top } else { top.child(buttons) };
    let mut r = row_frame(t, false).key(format!("harness-profile-row-{}", p.id)).child(top);
    if editing {
        r = r.child(
            col().w_full().border_sides([1.0, 0.0, 0.0, 0.0], t.border).fill(t.card_bg).px(14.0).pb(14.0).pt(10.5).child(editor_slot(
                t,
                kind,
                "harness-profile-editor",
            )),
        );
    }
    r
}

fn new_row(t: &Theme, kind: &str, first: bool) -> El {
    let r = if first { col().w_full() } else { col().w_full().border_sides([1.0, 0.0, 0.0, 0.0], t.border) };
    r.key("harness-profile-row-new")
        .fill(t.card_bg)
        .px(14.0)
        .pb(14.0)
        .pt(10.5)
        .child(row().mb(10.5).child(text("New profile").font(sans(rem(0.875), 500.0, 1.43)).color(t.text)))
        .child(editor_slot(t, kind, "harness-profile-editor-new"))
}

/// The Profiles group of a harness that supports them (`data-setting-group="profiles"`).
pub fn section(s: &State, t: &Theme, kind: &str, name: &str) -> El {
    let profiles = s.harness_profiles(kind);
    let (editing, notice, error) = s.settings.harnesses.profiles.of(kind);
    let has_default = profiles.iter().any(|p| p.is_default);
    let mut group =
        col().key("settings-group-profiles").w_full().gap(10.5).child(head(t, kind, name, matches!(editing, Some(Editing::New(_)))));
    if let Some(words) = notice {
        group = group.child(
            text(words.to_string()).key("harness-profile-notice").role("status").font(sans(rem(0.75), 400.0, 1.33)).color(t.running_text),
        );
    }
    if let Some(words) = error {
        group = group.child(kit::error_note(t, "harness-profile-error", words).mt(0.0));
    }
    if profiles.is_empty() && editing.is_none() {
        return group.child(match s.profiles_load(kind) {
            Load::Loading => loading(t, name),
            Load::Failed => load_error(t, kind, name),
            Load::Loaded => empty(t, name),
        });
    }
    let mut list = col()
        .key("harness-profiles-list")
        .w_full()
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.main_bg)
        .clip_rounded(RADIUS_CARD);
    if !profiles.is_empty() {
        list = list.child(none_row(t, kind, has_default));
    }
    for p in &profiles {
        list = list.child(profile_row(t, kind, p, editing == Some(&Editing::Edit(p.id.clone()))));
    }
    if matches!(editing, Some(Editing::New(_))) {
        list = list.child(new_row(t, kind, profiles.is_empty()));
    }
    group.child(list)
}
